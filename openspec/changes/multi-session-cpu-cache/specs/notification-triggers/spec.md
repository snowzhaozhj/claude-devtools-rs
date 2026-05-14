## ADDED Requirements

### Requirement: Notifier 按 `FileSignature` 缓存以避免重复 parse

`NotificationPipeline` SHALL 维护一个内部缓存，以 `(project_id, session_id)` 为 key，记录上一次成功处理的 JSONL 文件的 `FileSignature`。`FileSignature` MUST 至少包含：

- `mtime`：文件最后修改时间
- `size`：文件字节数
- `identity`：文件身份 —— Unix 上是 `(dev, ino)` 元组；Windows 与其它平台允许退化为空（详 design D1f：Windows 上 `std::os::windows::fs::MetadataExt::file_index()` 是 unstable feature `windows_by_handle`，stable Rust 不可用，故退化为仅依赖 mtime+size 的 best-effort 等价）

**等价性是 best-effort**：在常规 append-only 写入路径下，`FileSignature` 字段 byte-equal 即视为文件未变。inode reuse + mtime/size 三维同时撞车（极罕见）等极端场景可能假命中，由后续任何文件变化的 file-change 自然恢复（Claude Code 持续 append 让 size 单调增加 → 必然 cache miss → 重 parse）。

处理 `FileChangeEvent` 时 SHALL 在 `parse_file` 之前先 stat 目标文件，若 stat 拿到的 `FileSignature` 字段 byte-equal 等于缓存中该 key 的记录 THEN MUST 跳过 `parse_file` 与 `detect_errors` 整段流程；否则正常 parse + detect 并 把新的 `FileSignature` 写回缓存。

缓存 SHALL 在以下任一条件下走 cache miss（即正常 parse 路径）：

- 缓存中无该 key
- 缓存中该 key 的 `mtime` / `size` / `identity` 任一字段与 stat 结果不同（含 truncate 导致 size 变小、文件被 rename 替换导致 inode/file_index 变化等）
- stat 调用失败（文件被删 / 权限变化等）

缓存容量 SHALL 上限 200 entries，超过时按 LRU 淘汰最久未访问的条目；命中时 MUST 把命中 key bump 到队首（最新访问），避免冷热顺序混淆。

#### Scenario: 同一 session 文件 `FileSignature` 未变时跳过 parse

- **WHEN** `process_file_change` 收到 `FileChangeEvent` 且目标 JSONL 的 `FileSignature` 字段 byte-equal 等于缓存中该 `(project_id, session_id)` 的记录
- **THEN** notifier MUST 不调用 `parse_file`，不调用 `detect_errors`，不向 `error_tx` 发送任何事件

#### Scenario: 文件 mtime 变化触发重 parse

- **WHEN** `process_file_change` 收到 `FileChangeEvent` 且目标 JSONL 的 `mtime` 与缓存记录不同
- **THEN** notifier MUST 调用 `parse_file` 重新解析全文件，跑 `detect_errors`，按确定性 id 去重后通过 `error_tx` 广播新增 `DetectedError`，并把新的 `FileSignature` 写回缓存

#### Scenario: 文件 size 变小（truncate / rotate）触发重 parse

- **WHEN** `process_file_change` 收到 `FileChangeEvent` 且目标 JSONL 的 `size` 比缓存记录小
- **THEN** notifier MUST 走 cache miss 分支，重新 parse 与 detect，并以新 `FileSignature` 覆盖缓存

#### Scenario: 文件被 rename 替换（inode 变化）触发重 parse（仅 Unix）

- **WHEN** `process_file_change` 收到 `FileChangeEvent` 且目标 JSONL 的 `identity`（Unix `(dev, ino)`）与缓存记录不同 —— 即便 mtime 与 size 巧合相同
- **THEN** notifier MUST 走 cache miss 分支重新 parse，并以新 `FileSignature` 覆盖缓存
- Windows 与其它平台 identity 退化为 `None`，此 Scenario 由 mtime/size 维度兜底（best-effort，详 design D1f）

#### Scenario: stat 失败时走 cache miss

- **WHEN** `process_file_change` 收到 `FileChangeEvent` 但目标 JSONL 的 `tokio::fs::metadata` 调用失败（例如文件已被删除、权限错误）
- **THEN** notifier MUST 不依赖缓存，进入正常 parse 路径（由 parse_file 自身决定如何报错），并 SHALL NOT 把失败结果写入缓存

#### Scenario: 缓存超过容量时按 LRU 淘汰

- **WHEN** notifier 处理一条新的 `(project_id, session_id)` 且缓存已达 200 entries
- **THEN** notifier MUST 淘汰当前最久未访问的条目后再写入新条目，缓存大小始终 ≤ 200

#### Scenario: 缓存命中时把 key bump 到队首

- **WHEN** lookup 在缓存中命中 `(project_id, session_id)`
- **THEN** notifier MUST 把该 key 的 LRU 位置移到队首（最新访问），后续淘汰循环中该 key 不会被冷热顺序错误淘汰
