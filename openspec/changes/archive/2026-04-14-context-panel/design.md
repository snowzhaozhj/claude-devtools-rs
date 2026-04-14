## Context

原版 Context Panel（`SessionContextPanel/index.tsx`）是 320px 右侧边栏，展示 6 类上下文注入，支持 Category/Ranked 两种视图模式。数据从 `allContextInjections` 前端提取，不走后端 API。

Rust 版的 `SessionDetail.chunks` 已包含所有原始数据：
- `SystemChunk.contentText`：CLAUDE.md 内容、system-reminder 等
- `AIChunk.toolExecutions`：工具执行的 input/output
- `AIChunk.semanticSteps`：thinking text、text content
- `UserChunk.content`：用户消息

## Goals / Non-Goals

**Goals:**
- 右侧边栏展示 session 上下文，按类别分组折叠
- top-bar Context badge 可点击 toggle 面板
- 对齐原版的核心分类和交互

**Non-Goals:**
- 不实现 Ranked 视图模式（Category 模式优先，Ranked 后续补充）
- 不实现 Phase selector（当前 chunks 无 phase 信息）
- 不实现点击跳转到对应 turn（后续补充）
- 不改后端 API

## Decisions

### D1: 数据提取 — 前端 contextExtractor

新建 `contextExtractor.ts`，从 chunks 提取 4 类上下文（对齐原版但简化）：

| 类别 | 数据来源 | 展示内容 |
|------|---------|---------|
| System | SystemChunk.contentText | CLAUDE.md、system-reminder 等 |
| Tools | AIChunk.toolExecutions | 工具名 + input 摘要 + output 长度 |
| Thinking | AIChunk.semanticSteps (kind=thinking) | thinking 文本预览 |
| User | UserChunk.content | 用户消息预览 |

每个 entry 包含：category、label、preview（截断文本）、estimatedTokens（粗略按 4 chars/token 估算）。

### D2: 布局 — flex 三栏

App.svelte 的 main-content 内，SessionDetail 通过 flex 横向排列：左侧 conversation 区域（flex: 1）+ 右侧 ContextPanel（固定 320px，条件渲染）。ContextPanel 在 SessionDetail 内部管理，不需要改 App.svelte 布局。

### D3: ContextPanel 组件

- 固定 320px 宽度，`border-left` 分隔
- Header：标题 + 总 token 估算 + 关闭按钮
- Body：按类别分组，每组 collapsible（复用 CSS 模式，不复用 BaseItem——Context Panel 条目更紧凑）
- 每个条目：label + token 估算 badge + 点击展开查看完整内容

### D4: Toggle 交互

top-bar 的 `Context (N)` badge 改为可点击按钮，click 切换 `contextPanelVisible` 状态。

## Risks / Trade-offs

- [token 估算不精确] → 4 chars/token 是粗略估算，原版也是估算值。后续可接后端精确计算
- [数据量大时性能] → chunks 遍历是 O(n)，session 数据量不足以造成性能问题
