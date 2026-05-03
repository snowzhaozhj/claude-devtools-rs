## Context

`session-detail-lazy-render` 在 `ui/src/lib/lazyMarkdown.svelte.ts` 引入了 `IntersectionObserver` 控制的视口懒渲染：每个 markdown 占位元素首次进入视口（含 200 px rootMargin）时才同步调用 `renderMarkdown(text)` 注入 HTML，未进入视口的占位 div 是空背景色块。该改造把 96 条大会话首屏耗时砍掉了主要的 marked / highlight.js / DOMPurify / mermaid 成本。

`ui/src/components/SearchBar.svelte` 通过 `ui/src/lib/searchHighlight.ts::highlightMatches` 走 `document.createTreeWalker(..., NodeFilter.SHOW_TEXT)` 遍历 conversation 容器内的文本节点高亮匹配项。占位 div 没有文本节点 — 视口外的全部 chunk 在搜索时不可见、不可命中、不参与 next/prev 导航。该副作用在 `openspec/followups.md` 已明确记录，标记为 `coverage-gap` + "等真实痛点出现再修"，现确认要在发版前关闭。

约束：
- 不能改 lazy markdown 的默认行为（首屏 / 滚动渲染节奏不变，回滚开关 `LAZY_MARKDOWN_ENABLED = false` 仍生效）
- 不能改后端 IPC 协议
- 用户主动按 Cmd+F 是低频但重要的交互，可以接受一次性渲染卡顿换全文搜索
- SearchBar 当前与 lazy markdown 控制器互不知道彼此的存在，新链路需要在 `SessionDetail.svelte`（共同父级）做粘合

## Goals / Non-Goals

**Goals:**
- 应用内 SearchBar 输入查询后，匹配总数与全文匹配数一致（包含视口外段）
- next / prev 导航能跳到任意匹配项所在 chunk
- lazy markdown 控制器对外暴露的 hydrate API 形式简单，便于打印 / 导出等未来场景复用
- 实现可在不破坏现有 lazy 行为的前提下加入；回滚仅需 revert 一个 commit

**Non-Goals:**
- 不修浏览器原生 Cmd+F（Chrome / WebView Find-in-Page）— `followups.md` 单列、低优先级
- 不替换 SearchBar 的 DOM 高亮算法为后端全文索引（开销过大、超本 change 范围）
- 不优化 hydrate 时机为"按需 hydrate 单段"（方案 B），保持方案 A 全量 hydrate
- 不引入 `searchHighlight` 算法的双阶段（raw text 索引 + 局部 DOM 高亮），保持现有单阶段 TreeWalker

## Decisions

### D1: hook 注入 vs 直传 observer 实例

**选 hook 注入**：在 `SearchBar` 的 `Props` 上加 `onBeforeSearch?: () => void` 回调，由 `SessionDetail.svelte` 用 `() => lazyObserver.flushAll()` 传入。

**候选**：
- A. **Hook 注入**（选）：`SearchBar` 不知道 lazyMarkdown 的存在，仅承诺"在 `doSearch` 调用 `highlightMatches` 之前先 await 一次外部回调"。`SessionDetail` 持有 lazyObserver，决定如何 hydrate
- B. **直传 observer**：`SearchBar` 接受 `LazyMarkdownObserver` 作为 prop，内部自己调 `flushAll`。耦合 SearchBar ↔ lazyMarkdown 的概念，让 SearchBar 知道太多

选 A 的理由：
- SearchBar 的职责是"在容器内搜索"，hydrate 是"准备容器"的预处理 — 用 hook 表达更准确
- 未来如果引入打印 / 导出场景，可复用同一回调机制（"准备容器" → 全文操作）
- vitest 单测可直接 mock hook，不需要构造完整 observer

### D2: hydrate 触发时机

**选 `doSearch` 调用前**（即用户输入第一个字符 / 按 Enter / `query` 变化触发 debounce 后 → `doSearch` → 先 hook → 再 highlight）。

**候选**：
- A. **SearchBar `visible` 变 true 时 hydrate**：用户按 Cmd+F 一显示搜索栏就 hydrate，输入即时反馈
- B. **`doSearch` 调用前 hydrate**（选）：用户输入触发 debounce 后才 hydrate，避免"开 SearchBar 但没搜东西就关闭"白付出渲染成本
- C. **每次 `doSearch` 都 hydrate**：与 B 等价但语义更显式

选 B 的理由：
- 用户开 SearchBar 后改主意按 Esc 关掉的场景不少，A 会白白触发渲染
- B 在 `clearTimeout` debounce 之后触发，自然合并多次输入
- 性能成本：B 的首次输入会被 hydrate 阻塞一帧，但 debounce 已是 300 ms，用户感知较弱
- `flushAll` 是幂等的（已渲染元素 `dataset.rendered === "1"` 时跳过），重复调用零成本

### D3: `flushAll` 的实现细节

**实现**：遍历 `pending` map 的 entries，对每个 `(el, {text, onRendered})` 调 `renderInto(el, text, onRendered)`，从 `IntersectionObserver` `unobserve(el)`，最后清空 `pending` map。`renderInto` 会设 `dataset.rendered = "1"` 避免后续 IO 回调重渲染。

关键点：
- `pending` 当前是 `WeakMap<Element, ...>`，不可枚举 — **MUST 改为 `Map<Element, ...>`**。WeakMap 的 GC 优势在 lazy 场景没有实际收益（observer 持有 IO entry 对元素的强引用，元素不会被 GC），改 Map 不影响内存
- 顺序：按 `Map` 插入顺序（与 `observe` 调用顺序一致），保证 mermaid / highlight.js 副作用在 chunk 间的依赖（虽然实际无依赖）有可预期顺序
- 同步执行：`renderInto` 是同步的，`flushAll` 完成后 DOM 已就绪供 TreeWalker 扫描；`onRendered` 回调（mermaid 后处理）是 `Promise.resolve(...)` 异步，但 mermaid 是 SVG 替换，不影响文本节点搜索

### D4: `LAZY_MARKDOWN_ENABLED = false` 分支的 `flushAll`

**实现 no-op**：回滚分支下 `observe()` 已经同步 `renderInto`，没有 pending 元素需要 flush。返回的 observer SHALL 提供 `flushAll: () => {}`，保持接口一致避免调用方分支判断。

### D5: 测试策略

- vitest 单测覆盖 `lazyMarkdown.flushAll`：构造 mock IntersectionObserver，注册 N 个占位，断言 `flushAll` 后所有 `el.innerHTML` 含 markdown 产物 + `dataset.rendered === "1"` + `pending` 已清空
- vitest 单测覆盖 `SearchBar.doSearch` 调用顺序：mock `onBeforeSearch` + `highlightMatches`，断言 `onBeforeSearch` 在 `highlightMatches` 之前调用
- Playwright 用户故事（仅当 e2e 已稳定时加）：构造 fixture 含 96 条 chunk，唯一关键词放在第 80 条（视口外），按 Cmd+F 输入查询，断言 `totalMatches === 1` + 当前匹配项滚动进视口

## Risks / Trade-offs

- [大会话首次按 Cmd+F 触发一次性渲染卡顿，96 条大会话实测可能 800 ms ~ 2 s] → 用户主动触发 = 心理预期可承受；可在 `SearchBar` 输入框 placeholder 加 "搜索（首次准备中…)"提示，必要时 spinner 占位
- [`flushAll` 同步执行可能阻塞主线程导致输入延迟] → debounce 300 ms 已吸收；如实测仍卡，未来可改 `requestIdleCallback` 分批，但本 change 保持简单同步
- [`pending` 从 WeakMap 改 Map 后，SessionDetail unmount 漏调 `disconnect` 会留内存] → 当前 `disconnect` 已正确在 `onDestroy` 调用；为保险加 `disconnect` 内 `pending.clear()` 与 `io.disconnect()` 双兜底
- [hook 注入引入异步语义（回调可能返回 Promise）但当前实现同步] → `flushAll` 设计为同步，`onBeforeSearch` 类型签名 `() => void` 不带 Promise；未来需要异步时再改类型
- [Playwright 测试在 mock 环境下无真实 IntersectionObserver，可能 false positive] → vitest 单测兜底；Playwright 仅作冒烟，不依赖 IO 时序

## Migration Plan

1. 先改 `lazyMarkdown.svelte.ts` 加 `flushAll`，跑 vitest 单测
2. 再改 `SearchBar.svelte` 加 `onBeforeSearch` prop + 在 `doSearch` 入口调用
3. 最后改 `SessionDetail.svelte` 把 `lazyObserver.flushAll` 透传给 SearchBar
4. 跑 `just preflight` + 手动桌面 smoke（按 Cmd+F 在大会话搜索）
5. 回滚：revert 三个文件 commit + spec delta archive 撤销
