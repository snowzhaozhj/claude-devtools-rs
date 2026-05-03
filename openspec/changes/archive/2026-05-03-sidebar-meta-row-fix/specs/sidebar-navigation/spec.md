## MODIFIED Requirements

### Requirement: 会话项展示

每个会话项 SHALL 显示标题和元数据（消息计数、相对时间、git 分支）。标题 SHALL 优先使用后端提供的 title 字段，无 title 时 fallback 到 sessionId 前缀。

消息计数（`SessionSummary.messageCount`）SHALL 等于该 session 文件中**真实 user-chunk 消息**与配对 assistant 消息的总数——后端 `cdt-api/src/ipc/session_metadata.rs::extract_session_metadata` MUST 用对齐原版 `claude-devtools/src/main/types/messages.ts::isParsedUserChunkMessage` 的过滤函数判定 user 消息：`category != User` 或 `is_meta = true` 或 `MessageContent::Blocks` 不含任何 `Text` / `Image` block（即纯 `tool_result`-only 行）SHALL NOT 计入。配对计数规则保持原状：每个 user-chunk 后，紧接的第一个非 synthetic 非 sidechain 的 assistant 消息计 1（与 `awaitingAIGroup` 状态机一致）。

git 分支（`SessionSummary.gitBranch`）SHALL 在每条会话项第二行 meta 末尾以 `· <GitBranch icon> {branch}` chip 形式渲染；`gitBranch` 为 `null` 时 SHALL NOT 渲染该 chip（不留分隔符 `·`、不留空位）。该 chip MUST 跟随 `session-metadata-update` 事件 patch 的 `gitBranch` 即时更新。

#### Scenario: 有标题的会话
- **WHEN** SessionSummary.title 非空
- **THEN** SHALL 显示 title，文本溢出时截断并显示省略号

#### Scenario: 无标题的会话
- **WHEN** SessionSummary.title 为 null
- **THEN** SHALL 显示 sessionId 前 8 字符 + "…"

#### Scenario: 元数据显示
- **WHEN** 会话项渲染，`gitBranch` 为 null
- **THEN** SHALL 显示消息计数（`<MessageSquare icon> {N}` 格式）和相对时间（"刚刚"/"Nm"/"Nh"/"Nd"/日期），中间用 `·` 分隔

#### Scenario: 元数据含 git 分支
- **WHEN** 会话项渲染，`gitBranch = "feat/x"`
- **THEN** SHALL 在 messageCount + 时间之后追加 `· <GitBranch icon> feat/x`

#### Scenario: 消息计数排除 tool_result-only user 行
- **WHEN** session JSONL 含 1 条真实用户输入（`{role:"user", content:"hi"}`）+ 1 条 assistant tool_use + 1 条 user tool_result（`{role:"user", content: [{type:"tool_result", ...}]}`）+ 1 条 assistant 收尾
- **THEN** `extract_session_metadata` 返回的 `messageCount` SHALL 为 `2`（真实 user + 配对 assistant），**不**计入 tool_result-only 行

#### Scenario: 消息计数包含含 text+tool_result 混合 user 行
- **WHEN** user 消息 `MessageContent::Blocks` 同时含 `Text` block 与 `ToolResult` block
- **THEN** SHALL 计入 messageCount（与原版 `isParsedUserChunkMessage` 行为一致，"Must contain text or image blocks"）

#### Scenario: 消息计数包含 image-only user 行
- **WHEN** user 消息 `MessageContent::Blocks` 只含 `Image` block（用户粘贴截图，无文字）
- **THEN** SHALL 计入 messageCount

#### Scenario: 消息计数排除 is_meta=true 的 user 行
- **WHEN** user 消息 `is_meta = true`
- **THEN** SHALL NOT 计入 messageCount

## REMOVED Requirements

### Requirement: 项目 git 分支只读栏

**Reason**: 该 Requirement 把 `gitBranch` 当作 per-project 属性显示在 SidebarHeader 项目名下方一栏；实际 `gitBranch` 来自 JSONL 每行（每条 message），是 per-session 属性——同一 project 不同 session 可能在不同 branch（用户中途 `git checkout`），单栏 active-fallback 显示在跨 session 切换时跟着变，与"项目所在分支"语义不符，是伪 per-project 视图。

**Migration**: git 分支信息已迁移到每条 SessionItem 行内（详见本 spec `会话项展示` Requirement 的元数据 chip 条款）。SidebarHeader 不再渲染 `branch-row`，且不再接收 `sessions` / `activeSessionId` props。前端 e2e 测试断言从 `.branch-row .branch-name` 选择器迁移到 SessionItem 第二行内的 git chip 元素。
