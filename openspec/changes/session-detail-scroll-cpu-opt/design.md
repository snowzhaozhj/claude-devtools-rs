## Context

SessionDetail 已有 lazy markdown 管线，能避免视口外 markdown 首屏进入 `marked` / `highlight.js` / `DOMPurify`，但大型会话滚动时仍保留大量 chunk / message DOM。浏览器在滚动过程中仍可能为离屏节点付出 layout / paint 成本；同时已进入视口的无语言代码块若走 `highlightAuto`，会在主线程同步做语言猜测，首次可见时造成滚动卡顿。

本 change 聚焦低风险前端优化：利用浏览器原生 CSS containment 减少离屏 DOM 成本，并收敛高亮自动检测范围。它不改变 chunk 数据、滚动状态持久化、搜索语义、Mermaid 渲染时机或工具展开行为。

## Goals / Non-Goals

**Goals:**

- 降低 SessionDetail 长会话滚动时离屏 chunk / message 对 layout / paint 的 CPU 影响。
- 避免大块或未声明语言代码块触发 `highlightAuto` 的同步语言检测。
- 保持 lazy markdown、Mermaid、搜索、工具展开、自动刷新与贴底滚动语义不变。
- 提供可通过 CSS / 小函数回退的低风险实现路径。

**Non-Goals:**

- 不实现完整 virtual list，也不改变 chunk mount / unmount 生命周期。
- 不修改 Rust 后端、Tauri IPC、session detail payload 或 lazy markdown 数据模型。
- 不改变已声明语言代码块的高亮结果。
- 不引入新的前端依赖。

## Decisions

### D1: 使用 chunk / message 容器级 CSS containment，而非 virtual list

采用 `content-visibility: auto`、`contain-intrinsic-size` 与 `contain` 标记 SessionDetail 对话流中的稳定块级容器。候选方案是完整 virtual list，但 virtual list 会改变 DOM 存在性、搜索 / `flushAll()` / Mermaid / 展开状态与贴底滚动的交互，行为风险远高于本次 CPU 优化目标。

选择 CSS containment 的原因：浏览器仍保留 DOM 语义，搜索前的 `flushAll()`、IntersectionObserver lazy markdown、Mermaid 的局部扫描和现有滚动状态都可继续工作；优化主要交给渲染引擎处理，改动面小且容易回滚。

### D2: containment 只挂在自然块边界，不挂在 markdown 内部节点

容器级 class 应挂在 `UserChunk`、`AIChunk`、`SystemChunk`、`CompactChunk` 或消息卡片外层等自然块边界。候选方案是在 markdown 输出内部或每个 tool row 上细粒度挂 containment，但这会增加调试复杂度，并可能影响 sticky / popover / Mermaid SVG 尺寸计算。

选择块边界的原因：chunk 高度较稳定，能覆盖主要长 DOM 成本，同时避免破坏局部组件布局。

### D2b: 不使用 `contain: paint`

实现阶段验证发现 `contain: paint` 可能裁剪 AI header 的 token popover 等溢出层。容器隔离 SHALL 使用 `content-visibility: auto` + `contain-intrinsic-size` + `contain: layout style`；paint 跳过交给 `content-visibility` 处理，不用 paint containment 强行建立裁剪边界。

### D3: 无语言代码块默认 plaintext，大块内容禁止 `highlightAuto`

`renderMarkdown` 的代码高亮策略改为：声明语言且 highlight.js 支持时继续用指定语言；无语言代码块默认按 plaintext escape；若保留自动检测，也必须受明确字符数阈值约束。候选方案是继续对所有无语言 code block 调 `highlightAuto`，但这正是首次可见同步 CPU 峰值来源之一。

选择 plaintext 默认的原因：未声明语言没有契约要求必须猜测语言；plaintext 保留内容、XSS 清洗链路与 `<pre><code>` 结构，牺牲少量自动着色换取稳定滚动性能。

### D4: 用现有 mock UI / Playwright 验证行为，不新增性能基准

本 change 的目标是降低用户滚动过程 CPU，但自动化环境难以稳定断言 CPU 百分比。候选方案是新增浏览器 perf trace gate，但噪声和维护成本较高，不适合作为低风险 CSS / highlighter 策略 PR 的阻塞项。

选择现有验证路径：用 unit 测覆盖高亮策略，用 mock UI 或 Playwright 手动/自动覆盖滚动、搜索、lazy markdown 和 Mermaid，PR 描述中记录浏览器验证结果。

## Risks / Trade-offs

- `content-visibility: auto` 可能导致浏览器用估算高度替代离屏真实高度，滚动条在首次进入复杂 chunk 时轻微校正 → 通过合理 `contain-intrinsic-size` 与只挂 chunk 外层缓解。
- `contain` 可能影响依赖外部尺寸的子元素 → 避免使用过强的 `contain: strict`，只使用 layout / paint / style 级隔离，并通过 Mermaid / 搜索验证。
- 无语言代码块不再自动着色，视觉上可能比过去朴素 → 未声明语言本身没有准确性保证；用户需要高亮时可在 fenced code block 指定语言。
- 浏览器兼容性：`content-visibility` 在现代 Chromium WebView 支持；若某平台忽略该属性，行为退化为旧布局，不影响正确性。
