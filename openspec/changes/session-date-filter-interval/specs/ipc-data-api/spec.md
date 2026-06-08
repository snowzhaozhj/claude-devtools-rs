## MODIFIED Requirements

### Requirement: SessionSummary 携带 created 字段

`list_sessions` 返回的每个 `SessionSummary` MUST 额外携带 `created` 字段（epoch ms，IPC 序列化为 camelCase `created`）。该字段来源为文件 birthtime（`FsMetadata.created_ms()`），代表 session 的近似创建时间。

`timestamp` 字段语义不变——仍为文件 mtime，用于排序和展示。`created` 仅用于时间范围过滤。

`created` SHALL 使用 `#[serde(default)]` 注解，确保向后兼容（旧版前端忽略此字段不报错）。

#### Scenario: SessionSummary 含 created 字段

- **WHEN** 调用 `list_sessions` 获取 session 列表
- **THEN** 每个 `SessionSummary` SHALL 携带 `created` 字段
- **AND** `created` <= `timestamp`（创建时间不晚于最后修改时间）
- **AND** `timestamp` 仍用于列表排序（最近修改在前）
