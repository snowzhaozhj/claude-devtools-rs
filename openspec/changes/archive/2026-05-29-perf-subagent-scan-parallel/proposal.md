## Why

`get_session_detail` 的 `scan_subagents` 阶段在 31 个 subagent 的会话上耗时 921ms（总耗时 975ms 的 94%），超出 800ms 预算。根因：同一 project 内 subagent 串行处理 + 每个 subagent 双重文件读取（先泛型 `Value` 全扫 metadata，再结构化 parse 完整内容）。

## What Changes

- **合并双重文件读取**：`parse_subagent_candidate` 内从"先 `serde_json::Value` 全行扫 metadata + 再 `parse_file_via_fs` 结构化 parse"改为单次结构化 parse，从 `ParsedMessage` 流中提取 metadata（spawn_ts / end_ts / parent_session_id / description_hint）
- **同项目内并行化**：`scan_subagent_candidates_cross_project` 中同一 project 的 `for sub_entry in sub_entries` 串行循环改为 `join_all` + 内层 `Semaphore(4)` 限流并发
- **保留外层 `Semaphore(METADATA_SCAN_CONCURRENCY=8)` 不变**：跨 project_dir 并发控制维持现状，内层 Sem(4) 只在命中 subagent 的单个 project 内生效

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `ipc-data-api`：纯内部性能优化，不改 spec 行为契约。`get_session_detail` 的 subagent 扫描结果、字段内容、裁剪策略均不变。无需 spec delta。

## Impact

- **代码**：`crates/cdt-api/src/ipc/local.rs` — `parse_subagent_candidate` 函数重写 + `scan_subagent_candidates_cross_project` 内层并发改造
- **性能目标**：cold path scan_subagents_ms 从 ~930ms 降到 < 200ms；user/real ≤ 0.66（Sem(4) 限流）
- **风险**：并发 fd 数 = 4×2 = 8（远低于 ulimit 256）；tokio async I/O 不阻塞 worker；行为结果不变（resolve 逻辑输入相同）
