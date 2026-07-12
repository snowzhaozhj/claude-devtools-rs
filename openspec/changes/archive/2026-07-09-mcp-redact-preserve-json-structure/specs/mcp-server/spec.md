## MODIFIED Requirements

### Requirement: Secret redaction

MCP server SHALL 默认对 tool 返回内容进行 secret pattern 脱敏——匹配的 secret 替换为 `[REDACTED]`，返回体附加 `redacted: true` 和 `redactedCount: N`。

支持 `--allow-sensitive` 启动参数跳过 redaction。

脱敏 SHALL 作用于响应的结构化表示（对 JSON 字符串叶子值与对象 key 内的 secret 做替换），SHALL NOT 破坏响应的 JSON 结构：脱敏后返回体始终是合法 JSON，未命中 secret 的字段完整保留，仅命中的 secret 子串被替换为 `[REDACTED]`。

#### Scenario: API key 被自动脱敏

- **GIVEN** session 内容包含 `sk-ant-api03-xxxxxxxxxxxx`
- **WHEN** AI 调用 `get_session_chunks` 获取该段内容
- **THEN** 返回中该 key SHALL 被替换为 `[REDACTED]`
- **AND** 返回体 SHALL 含 `redacted: true`

#### Scenario: allow-sensitive 跳过脱敏

- **GIVEN** MCP server 以 `--allow-sensitive` 启动
- **WHEN** AI 调用任意 tool
- **THEN** 返回内容 SHALL 不做脱敏处理

#### Scenario: 脱敏不破坏响应 JSON 结构

- **GIVEN** 某字符串字段的值包含 secret（如 `password=hunter2`），且其后紧邻其他字段
- **WHEN** AI 调用返回该内容的任意 tool（默认脱敏开启）
- **THEN** 返回体（含 `{data, redacted: true, redactedCount: N}` 包裹）SHALL 是合法 JSON
- **AND** 命中的 secret 子串 SHALL 被替换为 `[REDACTED]`
- **AND** 同一响应中其余未命中 secret 的字段 SHALL 完整保留、值不被截断
