## ADDED Requirements

### Requirement: Runtime project list refresh

项目列表消费者 SHALL 能在收到项目刷新信号后重新扫描 `~/.claude/projects/` 并暴露新增项目。新增项目的显示名、路径、session 数与启动时全量扫描结果 MUST 使用同一 `project-discovery` 规则计算。

#### Scenario: Newly added project appears after rescan

- **WHEN** 应用启动后 `~/.claude/projects/` 下新增一个包含 `.jsonl` 会话的 project 目录
- **AND** 项目列表消费者触发重新扫描
- **THEN** 返回的 project 列表 SHALL 包含该新增 project
- **AND** 该 project 的 displayName、path、sessionCount SHALL 与冷启动扫描结果一致

#### Scenario: Rescan preserves existing project metadata

- **WHEN** 项目列表刷新前已有 N 个 project
- **AND** 新增一个 project 后触发重新扫描
- **THEN** 刷新后的列表 SHALL 包含原 N 个 project 与新增 project
- **AND** 原有 project 的 id SHALL 保持稳定
