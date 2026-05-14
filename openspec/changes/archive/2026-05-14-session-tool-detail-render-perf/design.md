## Context

会话详情页当前在工具项展开时同步挂载完整详情 DOM。Read/Write 查看器会把内容拆成行，并对每行调用 `highlightCode`；该函数内部使用 `highlight.js` 并经 `DOMPurify.sanitize` 后返回 HTML。Edit 查看器走 `DiffViewer`，先执行 LCS diff，再对 diff 行逐行高亮。几百行文件在低端机器或长会话 DOM 常驻时会形成明显主线程阻塞。

原版 React 实现中，`CodeBlockViewer` 使用轻量自写行级 highlighter，且通过 `useMemo` 缓存拆行；`DiffViewer` 保留 diff 行背景与行号，但不对每行再做语法高亮。Svelte 端需要对齐这种“工具详情优先保持可点、可滚、可读”的策略，同时保留已有 XSS 防护边界。

## Goals / Non-Goals

**Goals:**

- 降低 Read/Write/Edit 工具详情首次展开的同步主线程工作量。
- 保持现有视觉语义：路径、行号、增删背景、diff header 与 Soft Charcoal 代码 token 风格尽量一致。
- 保持安全边界：未经清洗的用户/文件内容不得通过 `{@html}` 注入。
- 为大文本工具详情增加可验证测试或性能回归入口。

**Non-Goals:**

- 不修改 IPC 数据结构、`OMIT_TOOL_OUTPUT` 策略或 `get_tool_output` 后端实现。
- 不实现整页对话流虚拟化；该问题范围更大，可另开 change。
- 不引入新的代码编辑器组件或大型依赖。

## Decisions

### D1: Read/Write 使用轻量转义优先，必要时再做受控高亮

Read/Write 工具查看器默认 SHALL 避免对所有行同步执行 `highlight.js + DOMPurify`。实现优先采用本地 `escapeHtml` + 轻量 token 渲染或分批高亮，让展开动作先完成。对需要 `{@html}` 的行内容，输出 MUST 来自内部高亮器生成的受控标签，或在注入前经过清洗。

候选方案：
- 继续用 `highlight.js` 但加缓存：命中重复内容时有效，首次展开仍阻塞。
- Web Worker 高亮：隔离主线程更彻底，但增加打包、语言加载和取消逻辑复杂度。
- 轻量/分批渲染：与原版更接近，改动范围小，适合当前问题。

选择轻量/分批渲染，因为目标是修复几百行文件展开卡顿，而不是重建完整编辑器级高亮。

### D2: DiffViewer 不对 diff 行做重型语法高亮

Edit diff 的主要语义是增删/上下文与行号。DiffViewer SHALL 保留统一 diff 的结构、背景、前缀与行号，但避免对每个 diff 行执行 `highlight.js`。行内容使用安全文本渲染或轻量转义输出。

候选方案：
- 保留语法高亮：视觉更丰富，但会把 LCS 后的每行再放大成高亮成本。
- 只高亮小 diff：行为分叉增加复杂度，用户难以预测。
- diff 行统一不做重型高亮：对齐原版，性能收益稳定。

选择统一不做重型高亮。

### D3: 展开状态与输出缓存保持局部更新，不改数据协议

SessionDetail 继续使用现有 `getToolOutput` 懒加载路径，但展开状态、加载态、缓存写入应避免触发不必要的 display item 重建。优先把派生计算限制在稳定输入上，并确保工具详情组件自己的渲染成本可控。

候选方案：
- 后端进一步裁剪 tool output：会改变 IPC/数据 omit 契约，超出本 change。
- 前端局部化渲染：不改协议，风险更小。

选择前端局部化。

### D4: 测试以策略断言为主，手动/性能入口补充体感验证

前端单测 SHALL 覆盖大文本 Read/Write/Edit 内容不会调用重型高亮路径或不会一次性处理所有行。若现有测试环境难以稳定测量耗时，则用结构性断言（如 diff 行不调用 `highlightCode`、大文本分批渲染初始行数）替代脆弱的毫秒阈值。

## Risks / Trade-offs

- [Risk] Read/Write 轻量高亮比 `highlight.js` 少部分语言 token。→ Mitigation: 保留基础代码可读性与主题变量，优先交互性能；复杂 Markdown 代码块仍由既有 Markdown 渲染路径负责。
- [Risk] 分批渲染可能在展开后短时间内逐步补齐行。→ Mitigation: 初始批次足够覆盖首屏，并显示稳定容器，避免布局大跳。
- [Risk] Diff 不做语法高亮导致视觉信息减少。→ Mitigation: diff 的核心语义来自增删背景、前缀和行号；与原版一致。
- [Risk] 结构性测试不能完全代表真实桌面体感。→ Mitigation: 同步跑 `npm run check --prefix ui`、相关 Vitest，并在可行时用 mock 浏览器手动验证大工具展开。
