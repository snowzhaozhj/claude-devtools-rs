## Why

打开 ≥1000 条消息的大 session 时首屏明显延迟（用户报告 976 条 session 卡顿）。已实测定位（`crates/cdt-api/tests/perf_get_session_detail.rs`，release）：

| Session | msgs | chunks | subs | 后端总耗时 | payload |
|---------|------|--------|------|-----------|---------|
| 4cdfdf06 | 172 | 8 | 5 | 22ms | 3.4 MB |
| 7826d1b8 | 250 | 19 | 1 | 15ms | 5.1 MB |
| 46a25772 | 1221 | 96 | 14 | **45ms** | **7.7 MB** |

后端纯计算（parse + chunk-build + context tracking + serde）<50ms，**不是瓶颈**。瓶颈在前端：

1. **96 个 chunk 一次性 mount** —— 每个 AI chunk 的 `lastOutput`、user 气泡、thinking/output/slash instructions 全部在首次 render 时同步跑 `marked.parse + highlight.js + DOMPurify`。N=96 时是几百到上千次同步 marked 调用。
2. **Mermaid 动态 import + 全量扫描** —— 首屏触发 `processMermaidBlocks` 扫所有 `.mermaid-block`。
3. **首屏纯文本 "加载中..."** —— 等 IPC + 全量 mount 期间用户看到的反馈极弱。

后端 payload 7.7 MB 的 JSON 跨 IPC 也有 50-150 ms 量级开销，但本次不动契约——先把前端这个独立可优化的大头先吃掉，验证后再决定要不要改 payload 形态（下一轮 change）。

## What Changes

- **MODIFIED**：`session-display` capability 新增 `Lazy markdown rendering for first paint performance` requirement —— 所有 markdown 渲染（user prose / AI lastOutput / thinking / output / slash instructions / system pre）SHALL 在节点进入视口（含 200 px 余量）后再触发 `renderMarkdown`；视口外保留占位（背景色块 + 估算高度）。
- **MODIFIED**：`session-display` capability 既有 `Markdown 渲染与代码高亮` requirement 微调措辞，明确"渲染时机由 lazy render 控制器决定，但 XSS 防护与代码高亮规则不变"。
- **MODIFIED**：`session-display` capability 既有 `Mermaid 图表渲染` requirement 微调措辞，明确 `processMermaidBlocks` SHALL 在 markdown 实际渲染**之后**再被触发，不在占位阶段扫描。
- **ADDED**：`session-display` capability 新增 `Skeleton placeholder while loading` requirement —— 首屏 IPC 进行中显示 N 条骨架卡片，替代当前纯文本 "加载中..."。

## Capabilities

### New Capabilities

无

### Modified Capabilities

- `session-display`：新增 lazy markdown render + skeleton 两条 requirement；既有 markdown / mermaid 两条措辞微调以容纳新机制。

## Impact

- **代码**
  - `ui/src/lib/lazyMarkdown.svelte.ts`（新文件）：`createLazyMarkdownObserver(rootEl, opts)` 返回 `observe(el, text)`/`disconnect()`；内部维持单一 `IntersectionObserver`，命中时 `renderMarkdown(text)` 并 `el.innerHTML = ...`，加 `data-rendered="1"` 标记防重复。
  - `ui/src/routes/SessionDetail.svelte`：所有 `{@html renderMarkdown(...)}` 调用点改为 `<div class="prose lazy-md" {@attach (el) => observer.observe(el, text)}>占位</div>`；`onMount` 创建 observer 并 root=`conversationEl`；`onDestroy` `disconnect()`。
  - `ui/src/routes/SessionDetail.svelte`：`processMermaidBlocks` 调用从首屏 effect 移到"任意 lazy markdown 区刚渲染完"的回调里（observer fire 后再扫该 block 内的 mermaid）。
  - `ui/src/routes/SessionDetail.svelte`：`{#if loading}` 分支替换 `<div class="state-msg">加载中...</div>` 为 `<SessionDetailSkeleton />` 组件。
  - `ui/src/components/SessionDetailSkeleton.svelte`（新文件）：渲染 5-8 条不同高度的灰色骨架卡片（不动画，避免与浏览器内置 skeleton-shimmer 冲突）。
- **依赖**：零新增——`IntersectionObserver` 是浏览器原生 API；`@attach` 是 Svelte 5 既有特性。
- **后端**：不动。`crates/cdt-api/src/ipc/local.rs::get_session_detail` 的 tracing 计时探针保留（性能回归监测有用）；`crates/cdt-api/tests/perf_get_session_detail.rs` 保留为 `#[ignore]` 基准。
- **测试**：前端没有单测框架（仅 `svelte-check`）；本次新增 lazy 模块尽量保持纯函数 + small surface，依赖 `npm run check` 与人工验证（开 1221 条样本 session 验视觉无回归 + console `[perf]` 探针读数对比）。
- **回滚**：lazy 行为可被一个 build-time const `LAZY_MARKDOWN_ENABLED = false` 切回直接渲染（保留代码路径），用于紧急 disable。
