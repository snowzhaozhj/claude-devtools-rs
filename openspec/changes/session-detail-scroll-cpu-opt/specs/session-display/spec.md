## ADDED Requirements

### Requirement: SessionDetail 滚动路径渲染隔离

SessionDetail SHALL 在对话流的 chunk / message 级稳定块容器上使用浏览器原生渲染隔离策略，降低视口外 DOM 对滚动过程 layout / paint 的影响。该策略 MUST NOT 改变 chunk 的 DOM 顺序、展开状态、搜索语义、lazy markdown 触发时机、Mermaid 渲染结果或 header popover 可见性；不支持相关 CSS 属性的平台 SHALL 退化为旧渲染行为。

#### Scenario: 离屏 chunk 使用内容可见性优化
- **WHEN** SessionDetail 渲染包含大量 `UserChunk` / `AIChunk` / `SystemChunk` / `CompactChunk` 的会话
- **THEN** 每个可见 chunk 外层容器 SHALL 带有用于 `content-visibility: auto` 的样式类或等价样式
- **AND** 该容器 SHALL 提供 `contain-intrinsic-size` 估算高度，减少离屏内容首次进入视口时的滚动跳变

#### Scenario: 渲染隔离不改变 lazy markdown
- **WHEN** 一个 markdown 占位尚未进入视口
- **THEN** 渲染隔离策略 SHALL NOT 强制调用 `renderMarkdown(text)`
- **AND** markdown 仍 SHALL 仅在 lazy markdown 控制器观察到进入视口或调用 `flushAll()` 时渲染

#### Scenario: 搜索强制渲染后仍能命中文本
- **WHEN** 用户在 SessionDetail 打开搜索并输入只存在于离屏 chunk 中的文本
- **THEN** 搜索流程 SHALL 继续通过 lazy markdown 控制器 `flushAll()` 渲染 pending 内容
- **AND** 渲染隔离策略 SHALL NOT 阻止浏览器在 DOM 中找到该文本

#### Scenario: Mermaid 首次可见后仍渲染
- **WHEN** 一个含 ```mermaid 代码块的 markdown 区域因滚动进入视口而触发 lazy markdown 渲染
- **THEN** `processMermaidBlocks(el)` SHALL 仍在该 markdown 区真实渲染后执行
- **AND** 渲染隔离策略 SHALL NOT 让 Mermaid 图表停留在纯代码视图

#### Scenario: Header popover 不被容器裁剪
- **WHEN** 用户 hover AI header 的 token summary
- **THEN** 自定义 token breakdown popover SHALL 能溢出 chunk 容器显示
- **AND** 渲染隔离策略 SHALL NOT 通过 `contain: paint` 或等价裁剪边界遮挡 popover

### Requirement: 无语言代码块高亮自动检测限制

Markdown 代码块高亮 SHALL 避免对未声明语言的大块内容执行同步 `highlightAuto` 语言检测。声明语言且 highlight.js 支持时 MUST 继续使用指定语言高亮；未声明语言或超过自动检测阈值的代码块 SHALL 按 plaintext 安全渲染，仍经过 Markdown 渲染与 DOMPurify 清洗链路。

#### Scenario: 声明语言代码块保持高亮
- **WHEN** Markdown 内容包含 ```rust 或其他 highlight.js 支持的声明语言代码块
- **THEN** renderer SHALL 使用对应语言调用 highlight.js 高亮
- **AND** 输出 SHALL 保留 `hljs` token class 以应用 Soft Charcoal 主题颜色

#### Scenario: 未声明语言代码块按 plaintext 渲染
- **WHEN** Markdown 内容包含未声明语言的 fenced code block
- **THEN** renderer SHALL NOT 对该代码块调用不受限的 `highlightAuto`
- **AND** 输出 SHALL 保留代码文本内容并按 plaintext 安全渲染

#### Scenario: 大块代码不自动检测语言
- **WHEN** Markdown 内容包含字符数超过自动检测阈值的未声明语言代码块
- **THEN** renderer SHALL NOT 调用 `highlightAuto`
- **AND** 首次进入视口时 SHALL 避免因语言猜测造成主线程长任务
