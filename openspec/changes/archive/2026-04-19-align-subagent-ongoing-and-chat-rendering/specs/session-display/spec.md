# session-display Spec Delta

## ADDED Requirements

### Requirement: Render task notification cards in user bubble

UserChunk 的 `content` 含一或多个 `<task-notification>...</task-notification>` XML 块时，UI MUST 通过 `parseTaskNotifications(content)` 抽取每个 block 的 `taskId` / `status` / `summary` / `outputFile` 四字段，在 user 气泡内**追加**渲染为独立卡片（move 原版 `UserChatGroup.tsx:484-536` 布局）。`cleanDisplayText` SHALL 继续把 `<task-notification>` 整段 XML 从正文清洗掉；user 气泡的渲染条件 MUST 改为 `text || images.length > 0 || taskNotifications.length > 0`——即使文本被清空、无图片，只要 task-notification 非空气泡仍 MUST 渲染。

#### Scenario: user message 只含 task-notification
- **WHEN** 一条 `user` 消息 content 是完整的 `<task-notification>...</task-notification>` XML，清洗后文本为空
- **THEN** 该 UserChunk MUST 渲染为独立 user 气泡，气泡内 MUST 含至少一张 task-notification 卡片，卡片 MUST 显示 summary 抽出的 cmdName、status 标签、exitCode（若 summary 含 `(exit code N)`）、outputFile basename

#### Scenario: user message 含 task-notification 混合正文
- **WHEN** 一条 `user` 消息 content 含多个 `<task-notification>` 块 + 普通文本
- **THEN** 气泡 MUST 先渲染清洗后的文本（markdown），再渲染每张 task-notification 卡片；卡片顺序 MUST 与 XML 出现顺序一致

#### Scenario: 失败 / 完成状态 UI 区分
- **WHEN** task-notification 的 `status` 为 `"failed"` 或 `"error"`
- **THEN** 卡片 status icon MUST 显示 ✕（红色 `error-highlight-text`）；`"completed"` 显示 ✓（绿色 `badge-success-text`）；其他状态（如 `"running"`）显示空心圆

### Requirement: AI header token summary uses last response usage snapshot

AIChunk 的 header 右侧 token 展示 MUST 取该 chunk 内**最后一条**带 `usage` 的 `AssistantResponse` 的 `usage` 四项之和作为"该 AI turn 结束时的 context window snapshot"，格式为压缩形式（如 `65.5k`）。**禁止**累加 chunk 内多条 responses 的 usage——Anthropic API 的 `cache_read_input_tokens` 每次返回"从 session 开头至当前 call 已缓存的历史"，多次 tool_use turn 中累加会把同一段历史重复计数 N 次，导致 UI 数字远大于真实值。

Header 前缀 MUST 显示 lucide `Info` SVG icon（hover 视觉提示）；hover 时 MUST 在 header 下方弹出 popover 卡片，列出 5 行 breakdown：Total / Input / Output / Cache create / Cache read（每项以 `toLocaleString()` 千分位显示）。`AIChunk.responses` 为空或全部 `usage=null` 时，header MUST 不渲染 token 槽（不显示 0）。

#### Scenario: 多 tool_use turn 取 last usage
- **WHEN** AIChunk 内含 3 条 responses：r1.usage={input=10, output=20, cacheRead=1000, cacheCreation=100} / r2.usage={input=5, output=8, cacheRead=1100, cacheCreation=50} / r3.usage={input=3, output=12, cacheRead=1200, cacheCreation=30}
- **THEN** header token MUST 显示 `fk(3+12+1200+30)` = `1.2k`（取 r3），**不是** `fk((10+20+1000+100)+(5+8+1100+50)+(3+12+1200+30))` = `3.5k`

#### Scenario: last usage 跳过 null
- **WHEN** AIChunk 末尾 response.usage 为 null，但前一条 response.usage 非 null
- **THEN** MUST 取"最后一条 usage 非 null"的 response 的 usage 计算

#### Scenario: hover 展示 breakdown
- **WHEN** 用户 hover Info icon 或 token 数字
- **THEN** 气泡下方 MUST 立即（<200ms，无原生 title 延迟）弹出自定义 popover 卡片，显示 Total / Input / Output / Cache create / Cache read 5 行；popover 不得依赖 `title=` HTML 原生 tooltip

### Requirement: Tool row displays approximate token count

ExecutionTrace 与 AI chunk 内联工具渲染中，每个工具 row 的 `BaseItem` MUST 通过 `getToolContextTokens(exec)` 估算 token 总和并以 `~{formatTokens(N)} tokens` 文案显示，与原版 `BaseItem.tsx:150` 格式一致。估算算法 MUST 为：

- input 部分：`estimateContentTokens(exec.input)`——对象/数组先 `JSON.stringify` 后按 ~4 字符/token 启发式计算
- output 部分：按 `ToolOutput.kind` 分支——`text` 取 `text` 字段走 `estimateTokens`；`structured` 取 `value` 走 `estimateContentTokens`；`missing` 贡献 0

`~N` 数字槽 SHALL 在 status 圆点之前渲染；工具 row 同时 MUST 在 status 圆点之后显示 `durationMs`（如 `25ms`）。当 `getToolContextTokens` 返回 0（空 input + missing output）时 row SHALL 不显示 token 槽。

#### Scenario: Bash 工具 row 显示 token 与 duration
- **WHEN** 一条 Bash tool row 的 `input={command: "ls -la"}` + `output.kind="text"` + `output.text="foo.rs\nbar.rs\n..."` 约 200 字符，duration 25ms
- **THEN** row MUST 显示 `~50 tokens` 槽（`ceil((len(JSON.stringify(input)) + 200) / 4)`）+ status 圆点 + `25ms`

#### Scenario: missing output 工具仍显示 input token
- **WHEN** 工具 `output.kind="missing"`（IPC 懒裁剪前的初始状态），`input={file_path: "/tmp/x.txt"}` JSON 约 40 字符
- **THEN** row MUST 显示 `~10 tokens`（仅 input 部分，output 贡献 0）
