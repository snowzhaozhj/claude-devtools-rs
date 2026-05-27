## Context

Rust 端口当前只有 `ReadToolViewer` 有 copy 按钮（文本式"复制"按钮在 header 内）。原版 TS 实现中 `CopyButton` 是通用组件（两种模式：overlay hover 出现 + inline header 内），被 ReadToolViewer、WriteToolViewer、MarkdownViewer、AI 输出、用户消息、Thinking 块、代码块等多处复用。

当前缺失：WriteToolViewer / BashToolViewer / AI 文本输出 / 用户消息气泡 / OutputBlock（代码高亮块）无 copy 功能。

## Goals / Non-Goals

**Goals:**
- 提供统一的 `CopyButton.svelte` 通用组件，支持 overlay（hover 可见）和 inline 两种模式
- WriteToolViewer header 加 copy 按钮（复制文件内容）
- BashToolViewer header 加 copy 按钮（复制命令输出）
- OutputBlock（AI 代码块）hover 时右上角出现 overlay copy 按钮
- 统一视觉反馈：Copy 图标 → Check 图标，2s 后恢复

**Non-Goals:**
- 不改 ReadToolViewer（已有 copy 按钮，保持现状）
- 不做 `CopyablePath` 组件（路径点击复制已在右键菜单覆盖，不阻塞本次）
- 不在 AI 文本输出 / 用户消息 / Thinking 块加 copy 按钮（这些区域用右键菜单"复制为 Markdown"已覆盖，体验合理；后续按需追加）
- 不修改后端 / IPC

## Decisions

### D1: CopyButton 组件 vs 各处内联

**选择**：抽取通用 `CopyButton.svelte` 组件。

**候选方案**：
- A. 每个 viewer 内联写 copy 逻辑（现 ReadToolViewer 做法）
- B. 抽通用 CopyButton 组件，各处引用

**取舍**：B 减少重复代码，统一视觉反馈时序（2s）和图标切换行为。ReadToolViewer 后续可迁移到 CopyButton 但不阻塞本 change。

### D2: Overlay 模式 vs 仅 inline

**选择**：CopyButton 支持两种模式 —— inline（header 按钮）+ overlay（hover 出现在右上角）。

**理由**：
- 工具 header 区用 inline 模式（与 view-toggle 等按钮并排）
- OutputBlock 代码块用 overlay 模式（不占常驻空间，hover 才出现）
- 原版 TS 同样是双模式设计

### D3: 图标选择

**选择**：用 lucide 风格 Copy / Check SVG path（已有 `ui/src/lib/icons.ts` 导出图标常量的模式）。

**候选方案**：
- A. 文本"复制"/"已复制"（现 ReadToolViewer 做法）
- B. SVG 图标 + tooltip

**取舍**：B 更紧凑、国际化友好。但 ReadToolViewer 现有文本按钮不在本 change 改动范围内，保持一致性留到后续统一。本 change 新增的 CopyButton 用图标模式。

### D4: BashToolViewer copy 范围

**选择**：复制**命令输出**（output text），不含命令本身。

**理由**：命令已在 BaseItem summary 可见可选；用户最常需要的是"拿输出去别处贴"。原版 TS 的 Bash 输出（`DefaultToolViewer`）反而没有 copy 按钮，我们比原版做得更好一点。

## Risks / Trade-offs

- [风险] `navigator.clipboard.writeText` 在 HTTP non-secure context 可能被浏览器策略拒绝 → Tauri webview 和 localhost dev 都是 secure context，风险低；失败时静默忽略（与 ReadToolViewer 现有行为一致）
- [风险] overlay 模式在触屏设备无 hover 态 → 桌面应用优先，触屏场景用右键菜单兜底
- [取舍] 不统一 ReadToolViewer 的文本按钮为图标 → 保持向后兼容、减小改动面积
