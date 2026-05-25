## MODIFIED Requirements

### Requirement: hot path 性能契约

启用 telemetry 后的 wall time 增量 SHALL < 0.2%；user time 增量 SHALL < 0.5%；max RSS 增量 SHALL < 1 MB（Registry 静态分配）。系统 SHALL 通过 perf 测试同步守护：以 feature flag 控制 telemetry 启用 / 关闭，运行同一负载（10000 次 `try_lookup_cached_metadata` + 1000 次 `list_sessions(50)`），对比两个分支的 wall time / user time / max RSS 满足上述阈值。

`event!` 宏 SHALL NOT 出现在 hot path（`cdt-api` IPC 入口、`cdt-discover` / `cdt-parse` / `cdt-analyze` 主循环 / 主算法）。CI 检查 SHALL 拦截 hot-path 文件中出现 `event!(` 字面量；命中即 fail PR。

#### Scenario: telemetry 启用后 hot path 无回归

- **WHEN** perf 测试以 `telemetry-enabled` feature 启用 telemetry，运行同一负载
- **THEN** wall time 增量 SHALL < 0.2%（按 baseline `try_lookup_cached_metadata` ~10-50 μs / list_sessions(50) ~95 ms 计）
- **AND** max RSS 增量 SHALL < 1 MB

#### Scenario: hot path 误用 event 宏被 CI 拦截

- **WHEN** PR 在 hot-path 文件（如 `cdt-api` IPC 入口）内加一行 `event!("perf.skeleton.start", ...)` 并 push
- **THEN** CI hot-path event 检查 SHALL fail
- **AND** PR SHALL NOT 通过（pipeline 红）
