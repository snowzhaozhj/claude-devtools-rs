# notification-triggers Specification Changes

## MODIFIED Requirements

### Requirement: Notifier 按 `FileSignature` 缓存以避免重复 parse

`NotificationPipeline` SHALL 维护一个内部缓存，以 `(project_id, session_id)` 为 key，记录上一次成功处理的 JSONL 文件的 `FileSignature`。`FileSignature` MUST 至少包含：

- `mtime`：文件最后修改时间
- `size`：文件字节数
- `identity`：文件身份 —— Unix 上是 `(dev, ino)` 元组；Windows 与其它平台允许退化为空（best-effort）

**等价性是 best-effort**：在常规 append-only 写入路径下，`FileSignature` 字段 byte-equal 即视为文件未变。inode reuse + mtime/size 三维同时撞车（极罕见）等极端场景可能假命中，由后续任何文件变化的 `[[push-events::file-change]]` 自然恢复（Claude Code 持续 append 让 size 单调增加 → 必然 cache miss → 重 parse）。

处理 file-change 事件时 SHALL 在 `parse_file` 之前先 stat 目标文件，若 stat 拿到的 `FileSignature` 字段 byte-equal 等于缓存中该 key 的记录 THEN MUST 跳过 `parse_file` 与 `detect_errors` 整段流程；否则正常 parse + detect 并把新的 `FileSignature` 写回缓存。

缓存 SHALL 在以下任一条件下走 cache miss（即正常 parse 路径）：

- 缓存中无该 key
- 缓存中该 key 的 `mtime` / `size` / `identity` 任一字段与 stat 结果不同（含 truncate 导致 size 变小、文件被 rename 替换导致 inode/file_index 变化等）
- stat 调用失败（文件被删 / 权限变化等）

#### Scenario: 同一 session 文件 `FileSignature` 未变时跳过 parse

- **WHEN** notification pipeline 收到 file-change 事件且目标 JSONL 的 `FileSignature` 字段 byte-equal 等于缓存中该 `(project_id, session_id)` 的记录
- **THEN** pipeline MUST 跳过 parse 与 detect 整段流程，无新通知输出

#### Scenario: mtime 变化时重新 parse

- **WHEN** notification pipeline 收到 file-change 事件且目标 JSONL 的 `mtime` 与缓存记录不同
- **THEN** pipeline MUST 调用 parse 重新解析全文件，跑 detect，按确定性 id 去重后通过广播推送新增错误，并把新的 `FileSignature` 写回缓存

#### Scenario: size 缩小时走 cache miss

- **WHEN** notification pipeline 收到 file-change 事件且目标 JSONL 的 `size` 比缓存记录小
- **THEN** pipeline MUST 走 cache miss 分支，重新 parse 与 detect，并以新 `FileSignature` 覆盖缓存

#### Scenario: identity 变化时走 cache miss

- **WHEN** notification pipeline 收到 file-change 事件且目标 JSONL 的 `identity`（Unix `(dev, ino)`）与缓存记录不同 —— 即便 mtime 与 size 巧合相同
- **THEN** pipeline MUST 走 cache miss 分支重新 parse，并以新 `FileSignature` 覆盖缓存

#### Scenario: stat 失败时走 cache miss

- **WHEN** notification pipeline 收到 file-change 事件但目标 JSONL 的 stat 调用失败（例如文件已被删除、权限错误）
- **THEN** pipeline SHALL 走 cache miss 分支（尝试 parse — 若文件不存在则自然跳过不产出通知）
