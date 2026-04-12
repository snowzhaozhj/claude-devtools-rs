## Context

Tauri 2 + Svelte 5 骨架已在 `tauri-svelte-scaffold` 提交中搭好。当前 UI 有两个页面：ProjectList（项目列表）和 SessionList（会话列表），通过 `App.svelte` 中的 `currentView` 状态切换。后端 `get_session_detail` Tauri command 已实现，返回 `SessionDetail`（含 `chunks: serde_json::Value`、`metrics`、`metadata`）。

TS 原版的 session detail 使用 ChatHistory → AIChatGroup → DisplayItemList 多层嵌套组件，结构较重。Rust 版 UI 尚在早期，应保持简洁。

## Goals / Non-Goals

**Goals:**
- 从 SessionList 点击 session card 进入 detail 视图
- 渲染所有 4 种 chunk 类型（User / AI / System / Compact）
- AI chunk 可展开查看 semantic steps 和 tool executions
- 顶部显示 session 级 metrics 汇总（token 用量、工具调用数）
- 保持 Tokyo Night 暗色主题一致性

**Non-Goals:**
- 不实现搜索高亮 / 导航跳转
- 不实现 Context 面板（injection 可视化）
- 不实现实时刷新 / file watching 联动
- 不实现子代理（subagent）展开视图
- 不做代码高亮渲染（tool input/output 用 `<pre>` 原样展示）

## Decisions

### 1. 组件拆分粒度

**选择**：`SessionDetail.svelte` 作为顶层，内联渲染各 chunk 类型（不拆子组件）。

**理由**：4 种 chunk 的渲染逻辑各不超过 30 行模板，拆成独立 `.svelte` 文件反而增加导航成本。等后续需要交互复杂化（搜索、context 面板）时再拆分。

### 2. 路由方式

**选择**：继续用 `App.svelte` 的 `currentView` 状态机（`"projects" | "sessions" | "detail"`），不引入 svelte-routing 等路由库。

**理由**：目前只有三级页面，状态机足够。引入路由库是过早抽象。

### 3. AI chunk 展开/折叠

**选择**：默认折叠，显示摘要行（第一条 text step 的前 100 字符 + 工具调用数）。点击展开显示所有 semantic steps 和 tool executions。

**理由**：session 可能有几十个 AI chunk，全部展开会导致页面过长且性能问题。TS 版也是默认折叠。

### 4. Tool execution output 渲染

**选择**：`ToolOutput::Text` 和 `ToolOutput::Structured` 统一用 `<pre>` 块渲染，截断超过 500 字符的内容并显示"展开全部"按钮。

**理由**：tool output 可能很长（如 `cat` 大文件），不截断会卡 DOM。TS 版也有类似的折叠处理。

### 5. TypeScript 类型

**选择**：在 `api.ts` 中定义与 Rust `serde(rename_all = "camelCase")` 对齐的接口。`SessionDetail.chunks` 在后端是 `serde_json::Value`，前端用 discriminated union `Chunk` 类型断言。

**理由**：Rust 端 `Chunk` 已经 `#[serde(tag = "kind")]`，前端可以直接用 `chunk.kind` 做类型收窄。

## Risks / Trade-offs

- **大 session 性能**：几百个 chunk 的 session 可能导致初始渲染慢 → 先不做虚拟滚动，观察实际性能后再决定
- **`serde_json::Value` 类型安全**：后端 chunks 是 JSON value 不是强类型 → 前端类型断言可能在字段缺失时崩溃 → 对关键字段做可选处理（`?.` 链）
