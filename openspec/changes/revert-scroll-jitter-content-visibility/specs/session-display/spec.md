## MODIFIED Requirements

### Requirement: 按 Chunk 类型渲染对话流

SessionDetail SHALL 按顺序渲染 chunks 数组中的每个 Chunk。不同 kind 的 Chunk SHALL 使用不同的视觉布局。对话流容器及其 chunk / message 级稳定块容器 SHALL NOT 采用"离屏时用估算高度占位、进入视口后以真实高度替换"的容器级渲染优化机制——该模式在离屏内容真实高度与估算占位差异较大时反复改变 conversation 容器 `scrollHeight`，触发用户可感知的滚动跳动。

#### Scenario: UserChunk 渲染
- **WHEN** chunk.kind 为 "user"
- **THEN** SHALL 渲染为右对齐气泡，显示消息文本（Markdown 渲染）、时间戳和 "You" 标签

#### Scenario: AIChunk 渲染
- **WHEN** chunk.kind 为 "ai"
- **THEN** SHALL 渲染为左对齐区块，包含 AI header（头像、模型名、token 统计、时间戳）和 body（文本+思考块）

#### Scenario: SystemChunk 渲染
- **WHEN** chunk.kind 为 "system"
- **THEN** SHALL 渲染为等宽字体预格式化块，带 Terminal 图标和 "System" 标签

#### Scenario: CompactChunk 渲染
- **WHEN** chunk.kind 为 "compact"
- **THEN** SHALL 渲染为居中摘要行，带 "Compact" 标签

#### Scenario: 空内容消息不渲染
- **WHEN** UserChunk 的文本经清洗后为空
- **THEN** 该 chunk SHALL 不产出任何 DOM 元素

#### Scenario: 长会话滚动高度保持稳定
- **WHEN** 用户在长会话上以触控板或鼠标滚轮上下滚动
- **THEN** conversation 容器 `scrollHeight` 在用户视口持续滚动且无新数据写入时 SHALL 保持稳定
- **AND** 当前阅读位置 SHALL NOT 因离屏内容首次进入视口而出现可感知跳动
- **AND** 对话流容器及任一 chunk / message 级稳定块容器 SHALL NOT 采用"离屏时用估算高度占位、进入视口后以真实高度替换"的容器级渲染优化机制

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

#### Scenario: token breakdown popover 不被容器裁剪
- **WHEN** 用户 hover AI header 的 Info icon 或 token 数字触发 popover 显示
- **THEN** 自定义 breakdown popover SHALL 完整显示在 header 下方，5 行 breakdown 内容不被任何祖先容器裁剪
- **AND** popover SHALL NOT 被 AI chunk 容器、message 级稳定块容器或对话流容器的渲染隔离边界遮挡

## REMOVED Requirements

### Requirement: SessionDetail 滚动路径渲染隔离

**Reason**: 该 Requirement 由 archive change `2026-05-16-session-detail-scroll-cpu-opt` 引入，依赖 `content-visibility: auto` + `contain-intrinsic-size: auto 220px` 估算占位机制。现场客观测量表明该机制在长会话滚动时使 conversation `scrollHeight` 反复跳变（10 秒滚动期间变化 11 次 / 总幅度 5291 px / 单次最大 4180 px），触发用户可感知视觉抖动；同 archive change 的"无语言代码块高亮自动检测限制" Requirement 才是 CPU 优化主源，与本 Requirement 独立。整段移除该 Requirement 并把反模式约束沉淀进 `按 Chunk 类型渲染对话流` Requirement，禁止后续以"性能优化"为由再次引入同类机制。

**Migration**: 移除 SessionDetail 模板中所有 `.msg-row-contained` 类应用以及对应 CSS 定义。被删除的 Scenarios 行为契约由以下既有 Requirement 独立覆盖，不留缺口：
- "渲染隔离不改变 lazy markdown" / "搜索强制渲染后仍能命中文本" → 由 `Lazy markdown rendering for first paint performance` Requirement 独立覆盖
- "Mermaid 首次可见后仍渲染" → 由 `Mermaid 图表渲染` Requirement 独立覆盖
- "Header popover 不被容器裁剪" → 本 delta 已把对应 Scenario 补入 `AI header token summary uses last response usage snapshot` Requirement
- "离屏 chunk 使用内容可见性优化" → 整段失效（该 Scenario 的前提就是引入本 Requirement 所反对的机制），不需要其它 Requirement 接手

若未来需要在滚动 CPU 上做新优化，候选方案见 `design.md` D1 替代方案讨论；任何方案 MUST 满足 `按 Chunk 类型渲染对话流` Requirement 中 `长会话滚动高度保持稳定` Scenario 的反模式约束。
