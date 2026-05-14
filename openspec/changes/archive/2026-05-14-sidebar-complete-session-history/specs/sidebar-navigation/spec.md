## ADDED Requirements

### Requirement: 完整加载分页会话历史

Sidebar 与 Command Palette 在消费 `list_sessions` IPC 时 SHALL 加载当前项目的完整会话历史，而不是只展示或搜索默认第一页结果。若 `list_sessions` 响应包含 `nextCursor`，前端 MUST 继续扩大请求或采用等价方式获取所有分页，直到最终响应 `nextCursor = null`。实现 SHALL NOT 使用逐页追加导致同 project 的 `session-metadata-update` 后台扫描只覆盖最后一页。

#### Scenario: Sidebar 显示默认第一页之后的旧会话
- **WHEN** 当前项目有 51 条会话，且 `list_sessions(projectId)` 默认第一页只返回 50 条并带 `nextCursor`
- **THEN** Sidebar SHALL 加载完整 51 条会话，并在会话列表中包含第 51 条旧会话

#### Scenario: Command Palette 搜索覆盖默认第一页之后的旧会话
- **WHEN** 当前项目有 51 条会话，且第 51 条旧会话的 title 匹配 Command Palette 查询文本
- **THEN** Command Palette SHALL 能从本地 session 数据中匹配并打开第 51 条旧会话

#### Scenario: 会话数量变化时继续扩大请求直到完整
- **WHEN** 前端第一次调用 `list_sessions(projectId)` 得到 `total = 51` 与 `nextCursor`
- **AND** 第二次按 `pageSize = 51` 从头请求时项目已新增会话，响应仍包含 `nextCursor`
- **THEN** 前端 SHALL 基于最新响应继续扩大请求，直到收到 `nextCursor = null` 的完整结果
