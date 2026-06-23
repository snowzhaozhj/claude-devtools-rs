## ADDED Requirements

### Requirement: Expose full session detail for export

新 IPC command `get_session_detail_for_export(projectId, sessionId) -> SessionDetailResponse` MUST 返回保留 tool output 与 response content 的 SessionDetail，供导出（Markdown / JSON / HTML）使用。该 command SHALL 复用 `LocalDataApi::get_session_detail`（始终以 `None` fingerprint 调用，强制返回 `Full` variant），返回前应用**导出专用裁剪**（`apply_export_omissions`）：`tool_executions[].output` 与 `responses[].content` SHALL 保留（不裁剪、对应 `*Omitted` 标志不置位），而 image data 与 subagent `messages` SHALL 照常裁剪（设 `dataOmitted` / `messagesOmitted`）以控制 payload。首屏 `get_session_detail` Tauri command 的协议与裁剪行为 SHALL NOT 受影响。

#### Scenario: 导出路径保留 tool output 与 response content

- **WHEN** caller 调用 `get_session_detail_for_export("proj", "sess")`，会话含有 output 的工具调用及 response content
- **THEN** 响应 SHALL 为 `Full` variant
- **AND** `tool_executions[].output` SHALL 含真实内容，`outputOmitted` SHALL NOT 为 true
- **AND** `responses[].content` SHALL 保留原内容，`contentOmitted` SHALL NOT 为 true

#### Scenario: 导出路径仍裁剪图片与 subagent 消息

- **WHEN** caller 调用 `get_session_detail_for_export` 拉取含内联图片与 subagent 的会话
- **THEN** 内联图片数据 SHALL 被裁剪（`dataOmitted = true`）
- **AND** subagent 的 `messages` SHALL 被裁剪（`messagesOmitted = true`）

#### Scenario: 首屏命令不受影响

- **WHEN** 新增导出 command 后调用首屏 `get_session_detail`
- **THEN** 其裁剪行为 SHALL 与既有一致（`OMIT_TOOL_OUTPUT` / `OMIT_RESPONSE_CONTENT` / `OMIT_IMAGE_DATA` / `OMIT_SUBAGENT_MESSAGES` 仍生效，对应 `*Omitted` 标志为 true）

#### Scenario: 浏览器模式复用既有完整路径

- **WHEN** 应用在 HTTP 模式运行，前端触发导出
- **THEN** 前端 SHALL 复用既有 `get_session_detail`（HTTP 路由本就返回完整未裁剪 detail）
- **AND** 浏览器导出与桌面导出在 tool output 与 response content 完整性上 SHALL 一致

#### Scenario: contract 测试覆盖新 command

- **WHEN** 运行 IPC contract 测试
- **THEN** `get_session_detail_for_export` SHALL 出现在 `EXPECTED_TAURI_COMMANDS`
- **AND** contract 测试 SHALL 断言导出路径（`apply_export_omissions`）保留 tool-output + response-content 且裁剪 image + subagent-messages；首屏路径（`apply_omissions`）四项全裁剪
