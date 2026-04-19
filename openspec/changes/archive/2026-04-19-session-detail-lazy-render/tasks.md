# Tasks

## 1. lazy markdown 模块

- [x] 1.1 新增 `ui/src/lib/lazyMarkdown.svelte.ts`：导出 `LAZY_MARKDOWN_ENABLED` 常量、`createLazyMarkdownObserver(rootEl, opts)` 工厂；内部维持单一 `IntersectionObserver`（rootMargin `200px 0px`），暴露 `observe(el, text, onRendered?)` / `disconnect()`
- [x] 1.2 observer fire 时同步执行 `el.innerHTML = renderMarkdown(text)`、设 `data-rendered="1"`、`unobserve(el)`，并触发 `onRendered?.(el)` 回调（用于 mermaid post-process）
- [x] 1.3 `LAZY_MARKDOWN_ENABLED = false` 时 `observe` SHALL 立即同步渲染（保留旧行为）
- [x] 1.4 单纯函数：`estimatePlaceholderHeight(text: string, kind: "user" | "ai" | "system" | "thinking" | "output" | "slash"): number`，按 design.md decision 2 公式实现

## 2. SessionDetailSkeleton 组件

- [x] 2.1 新增 `ui/src/components/SessionDetailSkeleton.svelte`：5 条静态灰色矩形（混合宽高对应 user/AI/system 视觉密度）
- [x] 2.2 卡片 CSS 用 `var(--color-border)` 作背景，无 shimmer/pulse 动画；`min-height` 与真实 chunk 接近（80/200/60/240/100 px）

## 3. SessionDetail 集成

- [x] 3.1 `ui/src/routes/SessionDetail.svelte`：`onMount` 创建 observer（root=`conversationEl`），`onDestroy` `disconnect()`
- [x] 3.2 把 user prose、AI lastOutput、Thinking 子级、Output 子级、Slash instructions 子级、System pre 内的 `{@html renderMarkdown(text)}` 改为占位 `<div class="prose lazy-md" {@attach (el) => observer.observe(el, text, mermaidPost)} style="min-height: {est}px"></div>`
- [x] 3.3 `mermaidPost(el)`: `await processMermaidBlocks(el)`；同步替换原 `$effect` 全树扫描
- [x] 3.4 `{#if loading}` 分支替换为 `<SessionDetailSkeleton />`；保留 `{:else if error}` / `{:else if detail}` 分支
- [x] 3.5 `refreshDetail` 路径**不**显示骨架（已是 silent）；保持反闪烁
- [x] 3.6 file-change 触发刷新替换 detail 后，已 `data-rendered="1"` 的节点不重复渲染（observer 只盯未渲染节点；新 chunk 默认占位）
- [x] 3.7 `lib/searchHighlight.ts` 路径不动；followups 记一条"搜索高亮跨视口未命中未渲染 chunk"，本次不修

## 4. 验证 + 诊断

- [x] 4.1 `npm run check --prefix ui` 通过（5 个既有 warning，无新增 error）
- [x] 4.2 `cargo build --workspace` + `cargo test --workspace` 全绿（92 + 多个 crate 全部通过）
- [ ] 4.3 用 1221 条样本 session（`46a25772-b57c-43bb-9ca6-f0292f9ca912`）在 `cargo tauri dev` 启动的窗口手动打开，对比 console `[perf]` 探针：first-paint 应从 改造前数值 → 改造后 < 200 ms（**留待用户启动 just dev 验证**：探针已就位）
- [ ] 4.4 验证滚动时 lazy chunk 平滑出现，无空白闪烁；mermaid 图表在含 mermaid 代码块的 chunk 进入视口时正确渲染（**留待用户启动 just dev 验证**）
- [ ] 4.5 验证已知功能未回归（**留待用户启动 just dev 验证**）：file-change auto refresh、ongoing banner、interruption block、subagent 卡片展开、ContextPanel、SearchBar Cmd+F（搜索字符串若在未渲染 chunk 内无法定位 = 已知，记 followup）
- [ ] 4.6 暗色主题验证骨架背景色无突兀（**留待用户启动 just dev 验证**）

## 5. spec 与 followups 维护

- [x] 5.1 `openspec validate session-detail-lazy-render --strict` 通过
- [x] 5.2 `openspec/followups.md` 在"性能 / 首次打开大会话卡顿"条目下追加：定位结果 + 本次改动 + 后端 payload 瘦身留作下一轮 follow-up
- [x] 5.3 `openspec/followups.md` 新增条目：搜索高亮跨视口未命中未渲染 chunk（lazy markdown 副作用）
- [x] 5.4 `openspec/followups.md` 新增条目：浏览器原生 Cmd+F find-in-page 不命中未渲染 chunk（同根因）

## 6. archive

- [ ] 6.1 `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings` 通过（archive 前最后一道）
- [ ] 6.2 `just preflight` 全绿（archive 前最后一道）
- [ ] 6.3 `openspec archive session-detail-lazy-render -y`（归档目录会变为 `<日期>-session-detail-lazy-render`）
- [ ] 6.4 同步 `CLAUDE.md` "UI 已知遗留问题" 段落 + 删除 memory `project_perf_large_session.md` 或更新为"已优化，效果观察中"
