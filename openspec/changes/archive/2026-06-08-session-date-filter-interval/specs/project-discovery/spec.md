## ADDED Requirements

### Requirement: Session 文件扫描

`Session` struct SHALL 额外携带 `created: i64` 字段（epoch ms）。`ProjectScanner` 扫描 session 文件时 SHALL 从 `FsMetadata.created_ms()` 填充该字段。

排序规则不变——仍按 `last_modified` 倒序。`created` 仅供下游过滤消费。

#### Scenario: Session 携带 created 字段

- **GIVEN** 一个 project 目录下有 session JSONL 文件
- **WHEN** `ProjectScanner::list_sessions` 扫描该目录
- **THEN** 返回的每个 `Session` SHALL 携带 `created` 字段
- **AND** `created` <= `last_modified`（birthtime 不晚于最后修改时间）
- **AND** 排序仍按 `last_modified` 倒序
