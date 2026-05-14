## ADDED Requirements

### Requirement: `extract_session_metadata` 按 `FileSignature` 缓存

`LocalDataApi` SHALL 持有一个内部 LRU 缓存（不使用全局单例），以文件 `PathBuf` 为 key，记录上一次扫描时的 `(FileSignature, title, message_count, messages_ongoing, git_branch)`。`FileSignature` MUST 至少包含：

- `mtime`：文件最后修改时间
- `size`：文件字节数
- `identity`：文件身份 —— Unix `(dev, ino)`；Windows 与其它平台退化为空（详 design D1f）

**等价性是 best-effort**：在常规 append-only 写入路径下，`FileSignature` 字段 byte-equal 即视为文件未变。inode reuse + mtime/size 三维同时撞车的极端场景可能假命中，由后续任何文件变化的 file-change 自然恢复。

再次调用相同 path 时 SHALL 先 stat 目标文件，若 stat 拿到的 `FileSignature` 字段 byte-equal 等于缓存记录 THEN MUST 直接返回基于缓存数据合成的 `SessionMetadata`，**不**再 line-by-line 重读全文件；否则正常扫描并把结果写回缓存。

由于 `is_ongoing` 字段含 `is_file_stale(path)` 时间敏感判定，缓存 MUST 仅缓存"基于消息序列结构"的 `messages_ongoing` 中间值（即 `cdt_analyze::check_messages_ongoing` 的结果），而 `is_ongoing = messages_ongoing && !is_session_stale(signature.mtime, SystemTime::now())` MUST 在每次 lookup 时根据当前 wall clock 实时计算合成——不得直接缓存 `is_ongoing` 终态。

缓存 SHALL 在以下任一条件下走 cache miss：

- 缓存中无该 path
- `mtime` / `size` / `identity` 任一不一致
- stat 失败

缓存容量 SHALL 上限 200 entries，按 LRU 淘汰；命中时 MUST 把命中 key bump 到队首避免冷热混淆。

#### Scenario: 相同文件 `FileSignature` 不变命中缓存

- **WHEN** 调用 metadata 缓存 wrapper 且 stat 拿到的 `FileSignature` 与缓存记录字段 byte-equal 等于缓存记录
- **THEN** MUST 直接返回基于缓存数据合成的 `SessionMetadata`，且 SHALL NOT 再调用 `tokio::io::AsyncBufReadExt::lines` 读全文件

#### Scenario: mtime 不一致触发重扫

- **WHEN** 调用 metadata 缓存 wrapper 且 stat 拿到的 `mtime` 与缓存记录不同
- **THEN** MUST 走原有 line-by-line 全文件扫描路径，并以新 `FileSignature` 与新结果覆盖缓存

#### Scenario: 文件被 rename 替换（inode 变化）触发重扫（仅 Unix）

- **WHEN** 调用 metadata 缓存 wrapper 且 stat 拿到的 `identity`（Unix `(dev, ino)`）与缓存记录不同 —— 即便 mtime 与 size 巧合相同
- **THEN** MUST 走 cache miss 分支重新扫描
- Windows 与其它平台 identity 退化为 `None`，此 Scenario 由 mtime/size 维度兜底（best-effort，详 design D1f）

#### Scenario: 缓存命中后实时重算 stale 状态

- **WHEN** 缓存命中（`FileSignature` 一致），且缓存条目的 `messages_ongoing = true`，且当前 wall clock 距 `mtime` 已超过 `STALE_SESSION_THRESHOLD`（5 分钟）
- **THEN** 返回的 `SessionMetadata.is_ongoing` MUST 为 `false`（`messages_ongoing && !stale = true && !true = false`）；缓存 SHALL NOT 因此被 invalidate（`FileSignature` 仍正确反映文件未变，下次访问还能复用其它字段）

#### Scenario: 文件 size 变小触发重扫

- **WHEN** 调用 metadata 缓存 wrapper 且 stat 拿到的 `size` 比缓存记录小
- **THEN** MUST 走 cache miss 分支重新扫描

#### Scenario: stat 失败时走 cache miss

- **WHEN** 调用 metadata 缓存 wrapper 但 `tokio::fs::metadata(path)` 失败
- **THEN** MUST 走原路径（由 `File::open` 自身决定返回空 `SessionMetadata`），且 SHALL NOT 把空结果写入缓存

#### Scenario: 缓存超过容量按 LRU 淘汰

- **WHEN** 缓存已达 200 entries 时再调用一个新 path
- **THEN** MUST 淘汰当前最久未访问的条目后再写入新条目，缓存大小始终 ≤ 200

#### Scenario: 缓存命中时把 key bump 到队首

- **WHEN** lookup 在缓存中命中 `path`
- **THEN** MUST 把该 path 的 LRU 位置移到队首（最新访问），后续淘汰循环中该 path 不会被冷热顺序错误淘汰

### Requirement: metadata 缓存 ownership 由 `LocalDataApi` 持有

`LocalDataApi` SHALL 通过一个 `Arc<std::sync::Mutex<MetadataCache>>` 字段持有缓存实例。所有构造器（`new` / `new_with_xxx`）MUST 初始化为空 cache。**禁止**用全局 `OnceLock` / `static` 单例 ——多个 `LocalDataApi` 实例（HTTP server + Tauri IPC 各自构造）必须各自独立持有 cache，互相不共享。

`extract_session_metadata` 自身 MUST 保留为纯函数（不持 cache），缓存查询 wrapper（如 `extract_session_metadata_cached(cache, path)`）MUST 作为内部辅助函数，由 `LocalDataApi` 的方法或 `scan_metadata_for_page` 调用。

#### Scenario: 多个 `LocalDataApi` 实例独立持有 cache

- **WHEN** 测试或运行时构造两个 `LocalDataApi` 实例 A 与 B
- **THEN** A 的 `metadata_cache` 与 B 的 `metadata_cache` MUST 是独立 `Arc<Mutex<MetadataCache>>` 实例，A 中的缓存写入 SHALL NOT 影响 B 中的 lookup 结果

#### Scenario: `extract_session_metadata` 保持纯函数签名

- **WHEN** 现有调用方（含单元测试 `extract_*`）直接调用 `extract_session_metadata(path)`
- **THEN** 该函数签名 MUST 保持 `pub async fn extract_session_metadata(path: &Path) -> SessionMetadata`，不接受 cache 参数；行为与本 change 之前完全一致（line-by-line 全文件扫描）
