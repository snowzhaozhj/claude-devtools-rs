## ADDED Requirements

### Requirement: Content mode 视图层

MCP server 的 `get_session_detail` tool 的 `content_mode` 行为 SHALL 保持不变。

视图层实现（`ChunkEnvelope`/`ToolExecEnvelope`/`ResponseEnvelope`/`ContentField`/`ContentMode`/`build_chunk_envelope()`/`summarize_input()`）SHALL 提取到共享模块 `crate::view`，MCP handler 通过引用共享模块使用。

提取后 MCP output 的 JSON 结构 SHALL 与提取前完全一致（字段名、字段顺序、omit/full 行为不变）。

#### Scenario: MCP get_session_detail content_mode=omit 行为不变

- **WHEN** MCP client 调用 `get_session_detail` with `content_mode: "omit"`
- **THEN** 返回的 chunk envelope 结构 SHALL 与重构前一致
- **THEN** tool execution 的 `inputSummary`、`outputChars`、`outputOmitted` 字段 SHALL 保持不变

#### Scenario: MCP get_session_detail content_mode=full 行为不变

- **WHEN** MCP client 调用 `get_session_detail` with `content_mode: "full"`
- **THEN** 返回的 chunk envelope 结构 SHALL 与重构前一致

#### Scenario: grep hit auto-expand 行为不变

- **WHEN** MCP client 调用 `get_session_detail` with `content_mode: "omit"` 和 `grep: "keyword"`
- **THEN** grep 命中 chunk SHALL auto-expand 为 full，context chunk 保持 omit
