## MODIFIED Requirements

### Requirement: Lazy markdown rendering for first paint performance

SessionDetail SHALL 把所有 markdown 内容（user prose / AI lastOutput / Thinking 展开体 / Output 展开体 / Slash instructions 展开体 / System pre 文本）的 `renderMarkdown` 调用延迟到节点进入视口（含 `200 px` rootMargin 余量）后再触发；视口外的对应区域 SHALL 仅渲染高度估算占位（背景色块），不调用 marked / highlight.js / DOMPurify。Mermaid block 的 `processMermaidBlocks` SHALL 在该 markdown 区真正渲染**之后**再被触发，不在占位阶段扫描。lazy markdown 控制器 MUST 对外暴露 `flushAll()` 同步方法，用于全文 DOM 操作场景（搜索 / 打印 / 导出）触发所有 pending 占位的强制渲染。

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

#### Scenario: flushAll 强制渲染所有 pending 占位

- **WHEN** 调用方对 lazy markdown 控制器调用 `flushAll()`
- **THEN** 所有处于 pending 状态（已 `observe` 但未进入视口）的占位元素 SHALL 按 `observe` 注册顺序同步调用 `renderMarkdown(text)` 注入 HTML
- **AND** 每个被 flush 的元素 SHALL 标记 `data-rendered="1"` 防重复
- **AND** 控制器内部的 pending map SHALL 被清空，IntersectionObserver SHALL `unobserve` 这些元素
- **AND** `flushAll` 是幂等的：再次调用时若无 pending 元素 SHALL 立即返回不做任何工作

#### Scenario: flushAll 在回滚开关关闭时为 no-op

- **WHEN** `LAZY_MARKDOWN_ENABLED = false` 时 SessionDetail 创建 lazy markdown 控制器并调用 `flushAll()`
- **THEN** 该方法 SHALL 立即返回不做任何工作（因为该分支下 `observe()` 已在注册时同步渲染，不存在 pending 元素）
- **AND** 接口签名 SHALL 与 enabled 分支一致，调用方无需分支判断
