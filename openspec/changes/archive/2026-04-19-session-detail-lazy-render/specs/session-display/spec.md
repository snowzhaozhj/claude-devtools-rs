# session-display Spec Delta

## ADDED Requirements

### Requirement: Lazy markdown rendering for first paint performance

SessionDetail SHALL 把所有 markdown 内容（user prose / AI lastOutput / Thinking 展开体 / Output 展开体 / Slash instructions 展开体 / System pre 文本）的 `renderMarkdown` 调用延迟到节点进入视口（含 `200 px` rootMargin 余量）后再触发；视口外的对应区域 SHALL 仅渲染高度估算占位（背景色块），不调用 marked / highlight.js / DOMPurify。Mermaid block 的 `processMermaidBlocks` SHALL 在该 markdown 区真正渲染**之后**再被触发，不在占位阶段扫描。

#### Scenario: 视口外 markdown 不渲染

- **WHEN** SessionDetail 首次挂载，detail 含 96 条 chunk，初始视口只覆盖前 5 条
- **THEN** 仅前 5 条 + 200 px rootMargin 内的 chunk 的 markdown 占位 SHALL 被替换为真实 HTML
- **AND** 其余 chunk 的 markdown 占位 SHALL 保持空背景色块，DOM 中 SHALL NOT 出现 `<pre><code class="hljs">` / `marked` 产出节点

#### Scenario: 滚动进入视口后渲染

- **WHEN** 用户向下滚动，未渲染的 markdown 占位首次进入视口（含 rootMargin）
- **THEN** 该占位 SHALL 在同一帧内调用 `renderMarkdown(text)`，把 HTML 注入容器
- **AND** SHALL 标记 `data-rendered="1"` 防重复，从 IntersectionObserver `unobserve` 该节点

#### Scenario: Mermaid 渲染时机

- **WHEN** 一个 markdown 占位首次渲染，且文本含 ```mermaid 代码块
- **THEN** SHALL 在占位渲染**之后**对**该占位元素**调用 `processMermaidBlocks(el)`，不扫整个 conversation 容器
- **AND** 已 `mermaid-done` 标记的 block SHALL 不重复渲染

#### Scenario: 视口外不进入 highlight.js / DOMPurify

- **WHEN** 一个 chunk 的 markdown 占位从未进入视口
- **THEN** 该文本 SHALL NOT 经过 `marked.parse`、`hljs.highlight`、`DOMPurify.sanitize` 任一处理

#### Scenario: 占位高度估算避免 layout 跳

- **WHEN** SessionDetail 首屏 mount 完，未渲染的占位高度按文本长度估算（如 `Math.max(60, text.length / 80 * 22)` px）
- **THEN** 进入视口后真实内容渲染产生的高度差 SHALL 不超过 `200 px` rootMargin（即不会让用户感知"跳一下"导致当前阅读位置丢失）

#### Scenario: file-change 自动刷新不打破 lazy 状态

- **WHEN** 当前 SessionDetail 已 lazy 渲染了部分 chunk，收到 `file-change` 触发 `refreshDetail` 重拉
- **THEN** 新 detail 替换后，已渲染过的 chunk（按 `chunkKey` 一致性）SHALL 保留渲染态；新增 chunk SHALL 默认占位、入视口后再渲染

#### Scenario: 紧急回滚开关

- **WHEN** `lazyMarkdown.svelte.ts` 顶部常量 `LAZY_MARKDOWN_ENABLED = false`
- **THEN** SessionDetail SHALL 退化为首屏直接渲染所有 markdown（旧行为），用于发现严重回归时一行切回

### Requirement: Skeleton placeholder while loading

SessionDetail 在 IPC `getSessionDetail` 进行中（`detail == null && loading == true`）SHALL 渲染骨架卡片占位（5 条不同高度的灰色矩形，对应 user / AI / system 视觉密度），而非纯文本 "加载中..."。骨架仅在初次加载（无缓存）显示；file-change 自动刷新走 `silent` 路径不显示骨架。

#### Scenario: 初次打开 session 显示骨架

- **WHEN** 用户首次点开一个 session tab，无 `tabStore` 缓存
- **THEN** SessionDetail SHALL 立即渲染 `<SessionDetailSkeleton />`（5 条灰色卡片），SHALL NOT 显示纯文本 "加载中..."

#### Scenario: 缓存命中不显示骨架

- **WHEN** 用户切回已打开过的 session tab（`getCachedSession(tabId)` 命中）
- **THEN** SessionDetail SHALL 直接渲染缓存的 detail，SHALL NOT 显示骨架

#### Scenario: file-change 刷新不显示骨架

- **WHEN** 已打开的 SessionDetail 收到 `file-change` 触发 `refreshDetail()`
- **THEN** 旧 detail 视图 SHALL 保留至新数据返回；过程中 SHALL NOT 切到骨架占位（保持反闪烁三原则）

#### Scenario: 骨架卡片无 shimmer 动画

- **WHEN** 骨架占位渲染
- **THEN** 卡片背景 SHALL 为静态 `var(--color-border)`，SHALL NOT 有 shimmer / pulse 动画（避免与 OngoingIndicator 视觉竞争 + 节省 GPU）

## MODIFIED Requirements

### Requirement: Markdown 渲染与代码高亮

文本内容 SHALL 通过 Markdown 渲染器转为 HTML。代码块 SHALL 进行语法高亮。渲染结果 SHALL 经过 XSS 防护处理。**渲染时机由 lazy markdown 控制器决定（详见 `Lazy markdown rendering for first paint performance`）；XSS 防护与代码高亮规则在懒渲染触发时仍 MUST 严格执行。**

#### Scenario: 代码块语法高亮

- **WHEN** Markdown 内容包含围栏代码块且指定了语言
- **THEN** SHALL 使用 highlight.js 进行语法高亮，应用 Soft Charcoal 主题 token 颜色

#### Scenario: XSS 防护

- **WHEN** Markdown 渲染的 HTML 包含潜在 XSS 内容（script 标签等）
- **THEN** SHALL 通过 DOMPurify 清洗后再注入 DOM

### Requirement: Mermaid 图表渲染

Markdown 中的 mermaid 代码块 SHALL 渲染为 SVG 图表。**`processMermaidBlocks` 的触发时机 SHALL 紧跟在该 markdown 区被 lazy 渲染之后（详见 `Lazy markdown rendering for first paint performance`），而非首屏 effect 全树扫描。**

#### Scenario: Mermaid 代码块渲染

- **WHEN** markdown 内容包含 ```mermaid 代码块
- **THEN** SHALL 动态加载 mermaid 库并渲染为 SVG 图表

#### Scenario: Code/Diagram 切换

- **WHEN** mermaid 图表已渲染
- **THEN** SHALL 提供 Code/Diagram 切换按钮，点击在源码和图表间切换

#### Scenario: 渲染失败降级

- **WHEN** mermaid 语法错误导致渲染失败
- **THEN** SHALL 显示错误提示并保留代码视图

#### Scenario: 主题适配

- **WHEN** 应用主题为 dark
- **THEN** mermaid 图表 SHALL 使用 dark 主题渲染
