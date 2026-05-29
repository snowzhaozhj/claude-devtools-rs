# ipc-data-api — delta (perf-subagent-scan-parallel)

## ADDED Requirements

### Requirement: get_session_detail subagent 扫描性能

`scan_subagent_candidates_cross_project` SHALL 对同一 project 下的多个 subagent 文件并行处理（内层并发度 ≤ 4），而非串行逐个解析。每个 subagent 文件 SHALL 仅做单次完整读取（结构化 parse），不再做额外的泛型 JSON 预扫描。

行为契约不变：扫描范围、过滤逻辑、resolve 三阶段 fallback、IPC payload 字段与裁剪策略均维持现状。

#### Scenario: 多 subagent 会话的 scan 阶段性能

- **GIVEN** 一个会话含 ≥ 20 个已结束的 subagent
- **WHEN** 调用 `get_session_detail`
- **THEN** `scan_subagents_ms` SHALL < 300ms（cold path，无缓存）
- **AND** 期间进程 user/real ratio SHALL ≤ 1.0
