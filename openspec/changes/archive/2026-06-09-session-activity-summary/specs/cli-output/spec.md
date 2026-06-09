## MODIFIED Requirements

### Requirement: --json fields 字段选择

`cdt sessions list --json` 无参数时列出的可用字段 SHALL 包含活动摘要字段：`projectId`、`projectName`、`userIntents`、`lastActive`、`durationMs`、`totalCost`、`toolErrorCount`、`filesTouched`、`gitSummary`。

这些字段 SHALL 与 `SessionSummary` serde 序列化的 camelCase 键名一致。

#### Scenario: --json 无参数列出新增字段

- **WHEN** 运行 `cdt sessions list --json`（无参数）
- **THEN** 输出的可用字段列表 SHALL 包含 `userIntents`、`filesTouched`、`gitSummary` 等新增字段

#### Scenario: --json 字段投影包含新字段

- **WHEN** 运行 `cdt sessions list --json=projectName,userIntents,totalCost`
- **THEN** 输出 SHALL 只包含指定的 3 个字段
