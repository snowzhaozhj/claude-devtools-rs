## Context

### 实测数据

`crates/cdt-api/tests/perf_get_session_detail.rs`（release）：

| Session | msgs | chunks | subs | parse | scan_subs | build | serde | TOTAL | payload |
|---------|------|--------|------|-------|-----------|-------|-------|-------|---------|
| 4cdfdf06 | 172 | 8 | 5 | 10 ms | 6 ms | 1 ms | 3 ms | 22 ms | 3.4 MB |
| 7826d1b8 | 250 | 19 | 1 | 9 ms | 1 ms | 0 ms | 3 ms | 15 ms | 5.1 MB |
| 46a25772 | 1221 | 96 | 14 | 18 ms | 14 ms | 4 ms | 8 ms | **45 ms** | **7.7 MB** |

后端 < 50 ms。前端首次 mount：96 chunk × 同步 marked.parse + highlight.js + DOMPurify ≈ 200-500 ms（按 mac 中端机经验值）。

### 现状链路

`ui/src/routes/SessionDetail.svelte`（直接渲染）：

```svelte
{#each detail.chunks as chunk, i (chunkKey(chunk))}
  ...
  <div class="prose">{@html renderMarkdown(text)}</div>
  ...
{/each}
```

`renderMarkdown` 同步：marked → highlight.js (5 语言已注册) → DOMPurify。一次首屏 mount，96 个 chunk 至少 100+ 次 `renderMarkdown` 同步调用。

`processMermaidBlocks` 在 `$effect` 内 `tick()` 后扫 `conversationEl` 全树，命中所有 `.mermaid-block:not(.mermaid-done)`。

### 反闪烁三原则

CLAUDE.md 已明示：稳定 key、`silent=true`、不经"加载中..."中间态。本次 skeleton 仅替换**首次** loading 文案；file-change 自动刷新路径仍走 silent 不显示 skeleton。

## Goals / Non-Goals

**Goals：**
- 1221 条 session 首屏 IPC 完成到第一帧可见 < 200 ms（视口外 markdown 不参与首帧 CPU）。
- 视口外 markdown 不渲染、不进入 DOMPurify、不进 highlight.js。
- 滚动到位时单 chunk 渲染 < 16 ms（一帧内）；prefer 200 px 余量预渲染避免视口边缘空白。
- 首屏 loading 期间用户看到布局级反馈（skeleton 卡片），而非纯文本。
- 反闪烁三原则保留：file-change 自动刷新路径不显示 skeleton。

**Non-Goals：**
- 后端 payload 形态变更（`SessionDetail.chunks` 仍全量返回）。
- 虚拟滚动（DOM 全 mount，只懒渲染 markdown 内容）。
- Tool viewer 内部 diff/syntax 渲染的懒加载（已经在 `expandedItems` 控制下，默认折叠）。
- 搜索高亮跨视口（`SearchBar` 的高亮链路本期不动；如未来命中"高亮目标在未渲染 chunk 内"，加 followup 处理）。
- Sidebar / Dashboard / ContextPanel 的 lazy 化。

## Decisions

### 1. 懒渲染机制：`IntersectionObserver` + `data-rendered` 标记

**选：** 在 SessionDetail mount 时创建一个 `IntersectionObserver`（root = `conversationEl`、`rootMargin = '200px 0px'`、`threshold = 0`）。每个 markdown 占位 div 通过 Svelte 5 `{@attach}` 注册到 observer。entry 进入视口时同步 `renderMarkdown(text)` 并 `el.innerHTML = ...`，标记 `data-rendered="1"` 后从 observer `unobserve(el)`。

**替代：**
- (a) `requestIdleCallback` 全量预渲染——Safari 不支持；首屏一定不能让所有 chunk 同时跑 marked，否则与现状无差。
- (b) `content-visibility: auto`——只跳过 paint/layout，不跳过 marked.parse 这步同步 CPU；不解决根因。
- (c) Web Worker 跑 marked——marked 主线程足够快，问题是**频次**而非单次成本；worker 引入 message passing 复杂度。

**理由：** IntersectionObserver 原生、单次 fire、性能成本低（浏览器侧已优化）；Svelte 5 `{@attach}` 是项目已有约定（CLAUDE.md "Svelte 5 `{@attach}` 挂副作用"），cleanup 内聚。

### 2. 占位高度估算

**选：** 占位 div 设 `min-height` 而非固定 height。按 chunk 类型估算：
- UserChunk：`Math.max(80, text.length / 80 * 22)` (px) —— 单气泡，line-height 22
- AIChunk lastOutput：`Math.max(60, text.length / 80 * 22)` —— 同上
- thinking / output：默认折叠，占位为 0
- system pre：`Math.max(60, lines * 18)` —— line-height 18

**替代：** 固定 200 px 占位 —— 短消息浪费空间，长消息进入视口时还会跳一下。

**理由：** 估算准则简单；进入视口后 `min-height` 由真实内容覆盖（block 自然 grow），不引发 layout shift（max 偏差 ~50 px，被 `200px` rootMargin 吸收）。

### 3. Mermaid 处理时机

**选：** 不再首屏 `$effect` 扫全树。改为 lazy markdown observer fire 后**仅扫该 block 内的 `.mermaid-block`**：

```ts
// in observer callback after renderMarkdown
await processMermaidBlocks(el); // el = 占位 div
```

`processMermaidBlocks` 内部已有 `:not(.mermaid-done)` 幂等 guard，无需改。

**替代：** 保留全量扫 + 加 batched scan —— 仍触发首屏 mermaid 动态 import（30+ KB），首帧 CPU 不省。

**理由：** mermaid 库本身就是 lazy 加载（首次需要时才 dynamic import），跟 markdown lazy render 时机自然同步；不打折扣。

### 4. Skeleton 卡片设计

**选：** 5 条静态灰色矩形（混合宽高，模拟 user 气泡 + AI 长块），无 shimmer 动画：
```html
<div class="skel-card skel-user"></div>  <!-- 80 px -->
<div class="skel-card skel-ai"></div>    <!-- 200 px -->
<div class="skel-card skel-user"></div>  <!-- 60 px -->
<div class="skel-card skel-ai"></div>    <!-- 240 px -->
<div class="skel-card skel-system"></div><!-- 100 px -->
```
背景色 `var(--color-border)` + 12px border-radius。

**替代：**
- (a) shimmer 动画 —— 与 OngoingIndicator 的 ping 视觉竞争；增加 GPU 工作。
- (b) 复用 `<BaseItem>` 渲染空 props —— 引入额外层级，不直观。

**理由：** 静态骨架的"加载中"信号已足够（位置占位 + 颜色暗示），过度设计反而干扰。

### 5. 反闪烁与 file-change 兼容

`Auto refresh on file change` requirement 已规定 `silent=true` 路径不经"加载中..."中间态。本次 skeleton 仅在初次（无缓存且 detail==null）显示；file-change 路径走 `refreshDetail()`，不会触发 `loading=true`，因此与现有反闪烁约束兼容。

### 6. 回滚开关

`ui/src/lib/lazyMarkdown.svelte.ts` 顶部：

```ts
export const LAZY_MARKDOWN_ENABLED = true;
```

`SessionDetail.svelte` 在调用 observer.observe 前 `if (!LAZY_MARKDOWN_ENABLED) return el.innerHTML = renderMarkdown(text);`。出现严重回归（如视口外 click-to-focus / accessibility 问题）时一行切回。

### 7. 性能探针保留

后端 `crates/cdt-api/src/ipc/local.rs::get_session_detail` 的 `tracing::info!(target: "cdt_api::perf", ...)` 与 `scan_subagent_candidates` 的 per-candidate timings 保留（已合入主路径）；前端 `[perf]` console.info 保留。理由：未来回归判定有据。

## Risks / Trade-offs

- **[风险] 滚动太快出现空白**：`rootMargin: 200px` 给一屏外预渲染余量，常规滚动速度（< 5 屏/s）够用；超快滚（拖滚动条）会短暂看到占位 → 可接受（用户主动操作）。
- **[风险] markdown 内 anchor link / table-of-contents 跳转**：跳转目标在未渲染 chunk 内时跳不到 → 浏览器原生 `#anchor` 走 `scrollIntoView`，未渲染元素 height 偏差小；进入视口后渲染会修正位置。可加 followup。
- **[风险] 搜索高亮跨视口**：`SearchBar` 当前 `searchHighlight.ts` 在 conversation 容器内通过 textNode walk 高亮 — 未渲染 chunk 内是占位 div 而非 markdown HTML，搜不到 → followup 单列：搜索时强制把目标 chunk 渲染。**本期 design 显式 punt**。
- **[权衡] 视口外文本不可被浏览器全文搜索（Cmd+F）**：浏览器内置 Find-in-Page 也不会命中未渲染内容 → 这是 lazy render 通病。后续可在 `Cmd+F` 触发时切到 `LAZY_MARKDOWN_ENABLED=false` 模式（all-render），代价是首帧慢。本期 punt。
- **[权衡] file-change pinned-to-bottom 行为**：当前 `refreshDetail` 在 pinned-to-bottom 时 `tick()` 后 `scrollTop = scrollHeight`。lazy render 不影响该路径——bottom 进入视口的 chunk 自然渲染。

## Migration Plan

1. 实现 `lazyMarkdown.svelte.ts` + `SessionDetailSkeleton.svelte`，不接 `SessionDetail.svelte`。
2. SessionDetail 把所有 `{@html renderMarkdown(text)}` 改为占位 div + `{@attach observer.observe(el, text)}`；mermaid 调用迁移到 observer callback。
3. loading 分支替换为 `<SessionDetailSkeleton />`。
4. `LAZY_MARKDOWN_ENABLED = true` 默认启用。
5. 用 1221 条样本 session 在 Tauri 窗口验证 console `[perf]` 数据，对比改造前后。
6. 留 24 h 自用观察；无回归则 archive。
7. 出现回归（搜索高亮 / a11y / 视觉抖动）时切 `LAZY_MARKDOWN_ENABLED = false` 紧急 disable，再走小补丁修单点。

无数据迁移。回滚一行 const 修改。
