## Context

PR #223（commit `3db8c8f`）修了 Svelte 5 `onDestroy` 在 element unmount 后读 `scrollTop` 永远 0 的 bug，引入 `latestScrollTop` 由 scroll listener 同步维护，作为保存源。该方案在 chunk 完整渲染的场景下能让 `scrollTop` 数值如实回填——但与 `change session-detail-lazy-render`（`ui/src/lib/lazyMarkdown.svelte.ts`）配合时仍有结构性问题：

- `estimatePlaceholderHeight()` 按字符数死算占位（user/ai/thinking/output/slash/teammate 走 `ceil(len/80) × 22 px`，system 按行数 × 18 px），highlight.js / 表格 / code block / mermaid / 图片占位让真实渲染高度普遍偏离估算
- 切回 cached path：`detail = cached` → `await tick()` → `<div class="conversation">` mount，但 `IntersectionObserver` 第一轮 callback 尚未触发，`data-rendered != "1"` 的 lazy-md 节点维持占位 min-height
- 此时 `conversationEl.scrollTop = savedValue` 写入：scrollHeight ≈ Σ 占位 < 保存时点 scrollHeight，浏览器 clamp `scrollTop` 到 `max = scrollHeight - clientHeight`
- 用户在底部时 `savedValue ≈ savedScrollHeight - clientHeight`，clamp 后偏差 = 保存时点与恢复时点 scrollHeight 之差，实测可达数千 px
- 后续 lazy chunks hydrate 让 scrollHeight 膨胀回原值，CSS scroll anchoring（`overflow-anchor: auto` 默认开启）只对**视口内已渲染元素位移**生效——不修复初始 clamp，也不接管视口外占位的尺寸增长

PR #223 描述把这归因为 "scroll anchoring 后续偏移属于浏览器层渲染时序" 误诊；mock fixture 上观测到的 saved=20 → final=48.5 的 28 px 漂移确实是 anchoring 在锚点元素位移时调整 scrollTop 的副作用，但底部场景几千 px 的偏差与 anchoring 无关，是 clamp。

社区参照（详见会话上文调研）：

- `react-virtuoso::getState() / restoreStateFrom`：保存对象包含 measured item sizes + 滚动锚点，不是数值
- `react-virtuoso::followOutput`：`isAtBottom` 是与 scrollTop 数值正交的 boolean
- `react-virtuoso::initialTopMostItemIndex`：按 index 而非像素恢复
- `TanStack Virtual::measureElement`：用 ResizeObserver 实测 item 高度，恢复按 index
- 我们没有引入虚拟滚动库（手写 conversation 容器），但同构思路是「锚点元素 + 偏移 + 粘底语义」

## Goals / Non-Goals

**Goals:**

- G1. 用户切走 tab 再切回时，**滚动位置在视觉上与切走前对齐**（底部仍在底部 / 中间位置偏差 ≤ 50 px / 任意位置不被 lazy 占位估算偏差影响）
- G2. 与现有 `session-detail-lazy-render` 架构兼容——不要求 lazy-md hydrate 在恢复前完成，不要求 scrollHeight 在恢复时点达到保存值
- G3. 关闭 PR #223 known limitation：archive 时把 followups 里的对应条目（如有）标 ✅ 并迁移到 archive 内
- G4. spec scenario 措辞从「数值契约」收紧为「视觉位置契约」，避免下次再用纯数值实现

**Non-Goals:**

- N1. 不引入虚拟滚动库（react-virtuoso / TanStack Virtual）——本 change 仅改保存 / 恢复算法，渲染管线维持不变
- N2. 不解决"切回 tab 时 lazy chunks hydrate 引起 anchor chunk 上方未渲染节点高度变化导致 anchor 像素位置漂移"——锚点法已天然规避（见 D2 选项分析），但若 anchor chunk 自身在视口顶且自身也未 hydrate，会有 1-2 帧抖动，这属于浏览器渲染时序边界，不在本 change 内追求 0 帧
- N3. 不持久化滚动状态到磁盘——`TabUIState` 仍是 per-session 内存级，关闭桌面应用即清
- N4. 不动后端 / IPC / Tauri 配置 / lazy-md 渲染管线本身

## Decisions

### D1：保存对象从「scrollTop 数值」改为「atBottom + anchor 三件套」

`TabUIState.scrollTop: number` 删除；新增 `atBottom: boolean` / `anchorChunkId: string | null` / `anchorOffsetPx: number`。

**为什么不沿用 scrollTop 数值方案 + ResizeObserver 重试**（备选 A）：

- 切回时 lazy hydrate 是逐 chunk 进行（IntersectionObserver 触发顺序与视口位置耦合），scrollHeight 增长是不连续阶跃；ResizeObserver 每次回调都重写 scrollTop 会与 lazy hydrate 互相诱发——scrollTop 变化引起新的可见 chunk hydrate → scrollHeight 再变 → 死循环或视觉抖动
- 即使加节流，最终落点仍依赖 scrollHeight 达到保存值，而保存值在 lazy 架构下不是稳定函数（取决于哪些 chunk 已 hydrate）
- 用户感知最强的是「在底部」语义，绝对数值方案对此始终是 indirect

**为什么不用 ratio 缩放**（备选 B）：

- `ratio = savedScrollTop / savedScrollHeight × currentScrollHeight` 在均匀内容假设下成立，lazy 占位估算偏差不均匀（user message 偏多，AI 含 mermaid/table 偏少），ratio 漂移 ±100~300 px
- 实现复杂度（需要监听 scrollHeight 变化持续重算）已经接近锚点法，但准度反而更差

**为什么选锚点法**：

- anchor chunk 自身一旦定位（`scrollIntoView({ block: 'start' })`），其在 layout 中的位置就锁定，**视口外** chunk 后续 hydrate 不影响 anchor 在容器内的相对位置
- 与 react-virtuoso `restoreStateFrom`、TanStack Virtual `scrollToIndex` 思路同构——主流方案
- spec scenario 措辞与"恢复到用户视觉位置"语义对齐，下次再有 lazy 改动也不必重写

### D2：anchor chunk 选择策略——第一个底部仍在视口内或之下的 chunk

`captureScrollAnchor()` 实现：

```ts
function captureScrollAnchor(): { atBottom: boolean; anchorChunkId: string | null; anchorOffsetPx: number } {
  if (!conversationEl) return { atBottom: false, anchorChunkId: null, anchorOffsetPx: 0 };
  const { scrollTop, scrollHeight, clientHeight } = conversationEl;
  const distanceFromBottom = scrollHeight - scrollTop - clientHeight;
  const atBottom = distanceFromBottom <= 16;  // 与 wasAtBottom 阈值同源
  if (atBottom) {
    return { atBottom: true, anchorChunkId: null, anchorOffsetPx: 0 };
  }
  // 找「跨越视口顶或完全在视口内的第一个」chunk —— 即第一个 bottom > containerTop 的元素：
  // - 该元素 bottom > containerTop 意味着它至少有一部分还在视口内或之下
  // - DOM 顺序遍历下，它要么是跨越视口顶的那个（rect.top < containerTop），要么是
  //   完全在视口内的第一个（rect.top >= containerTop）
  // 选这个语义而非"完全可见"：跨越视口顶的 chunk 通常正是用户视觉焦点所在
  const containerRect = conversationEl.getBoundingClientRect();
  const chunkEls = conversationEl.querySelectorAll<HTMLElement>('[data-chunk-id]');
  for (const el of chunkEls) {
    const rect = el.getBoundingClientRect();
    if (rect.bottom > containerRect.top + 1) {
      return {
        atBottom: false,
        anchorChunkId: el.dataset.chunkId ?? null,
        anchorOffsetPx: rect.top - containerRect.top,  // 跨越视口顶时为负
      };
    }
  }
  return { atBottom: false, anchorChunkId: null, anchorOffsetPx: 0 };
}
```

**为什么选「跨越视口顶或完全在视口内的第一个」而非「完全可见」**（备选 D2-alt）：

- 完全可见会跳过跨越视口顶的 chunk，那个 chunk 通常正是用户视觉焦点所在；anchor 选完全可见的下一个 chunk 等于人为下移焦点，恢复时反而错位
- 跨越视口顶的 anchor 在恢复时通过负 `anchorOffsetPx` 精确还原"chunk 顶部已被滚出视口 |offset| px"的状态，无信息损失
- 实现简单、`querySelectorAll` 按 DOM 树前序顺序返回，与 chunk 时间顺序一致

**anchor offset 含义**：anchor 元素 `getBoundingClientRect().top - container.getBoundingClientRect().top`：

- chunk 完全在视口内：offset 正值，表示 anchor 顶距容器顶 N px
- chunk 跨越视口顶：offset 负值，表示 anchor 顶已被滚出视口 |N| px

恢复时 `scrollIntoView({ block: 'start' })` 把 anchor 顶贴齐视口顶（`rect.top - containerTop = 0`），然后 `conversationEl.scrollTop -= anchorOffsetPx` 还原原始偏移：

- offset 正：`scrollTop` 减小 → 内容相对视口下移 → anchor 顶离开视口顶向下偏 offset px ✓
- offset 负：`scrollTop` 增大 → 内容相对视口上移 → anchor 顶被滚出视口 |offset| px ✓

### D3：粘底恢复用 MutationObserver pin 状态机

`atBottom = true` 时单次 `scrollTop = scrollHeight` 不够——切回后 lazy chunks 持续 hydrate 让 scrollHeight 增长，scrollTop 不会自动追随。

**为什么不用 ResizeObserver(conversationEl)**（备选 D3-alt）：

- conversationEl 是 `overflow-y: scroll` 容器，外框 content-box 尺寸由 flex layout 固定 → lazy hydrate 改变的是子节点高度 → 容器自身 size 不变 → ResizeObserver 不触发
- ResizeObserver 观察容器**内部**子元素能触发，但内部子元素是各 chunk 的 div，每条 chunk 都要观察成本不可接受；新增 wrapper sentinel 又要改 svelte template

**选 MutationObserver subtree 监听 `data-rendered` 属性变化**：lazy markdown observer 的 hydrate 路径是 `el.innerHTML = renderMarkdown(text); el.dataset.rendered = "1"`，每次 hydrate 都改 dataset.rendered 属性 + innerHTML（characterData / childList）。MutationObserver 一次性覆盖三种变化。状态机：

```
START → scrollTop = scrollHeight → 挂 MutationObserver(conversationEl, { subtree: true,
        attributes: true, attributeFilter: ['data-rendered'], childList: true, characterData: true })
        + 挂 onScroll 监听（用户主动）+ 启动 stableTimer(200 ms) + 启动 hardLimitTimer(5 s)
进入 PINNING 状态：
  - MutationObserver 回调：if (still pinning) { scrollTop = scrollHeight; reset stableTimer }
  - onScroll 监听：if (isPinning && distanceFromBottom > 16) stopPin() —— 用户主动滚
  - stableTimer: 200 ms 内无 mutation → stop（hydrate 在该窗口稳定）
  - hardLimitTimer: 5 s 上限 → stop（mermaid 等慢渲染兜底）
STOP → disconnect MutationObserver + 解绑 onScroll + clear 两个 timer
```

**为什么 200 ms 稳定窗口而非 scrollend 事件**：

- `scrollend` 只在 scrollTop 变化结束时触发；pin 期间 scrollTop 持续被 programmatic 写入，scrollend 不可靠
- 200 ms 是 IntersectionObserver `rootMargin: 200px` 配合视口高度的 hydrate 完成经验值（首屏 long session ~150 ms，含 mermaid block ~250 ms）

**为什么 5 s 上限**：

- 极端长会话 + 全 mermaid 渲染可能持续 1-2 s，5 s 给足余量
- 超时后强制 stop 避免泄漏 MutationObserver / 两个 timer

**cleanup 调用点**（response codex 反馈）：`startBottomPin()` 返回的 cleanup 由 SessionDetail 持有引用 `currentBottomPinCleanup: (() => void) | null`：

- 自终止路径（scroll 介入 / stableTimer / hardLimitTimer）内部调 cleanup 后置空
- 新一轮 `restoreScrollAnchor` 启动 pin 前先 `currentBottomPinCleanup?.()` 强制收敛
- `onDestroy` 内强制 `currentBottomPinCleanup?.()` 兜底防止 element unmount 后 MutationObserver / timer 泄漏

**用户主动 scroll 检测**：scroll listener 内 `if (isPinning && distanceFromBottom > 16) stopPin()`。pin 自身 `scrollTop = scrollHeight` 也触发 scroll event——但浏览器先 commit scrollTop 再 dispatch scroll，distanceFromBottom 在事件读取时必 ≤ 0（pin 后），不会误判。安全。

### D4：anchor 兜底降级

切回时 `anchorChunkId` 找不到（比如 chunk 因为后台 refresh 被合并 / 顺序变化 / id 失效）：

1. console.warn 记录失败的 anchorChunkId（开发阶段排错）
2. 降级到首屏顶部（不 setScrollTop，让浏览器默认 scrollTop=0）
3. **不**回退到旧 scrollTop 数值方案——降级路径越简单越好；而且数值方案本就有问题，回退到它不是正确兜底

**为什么不尝试相邻 anchor**（备选 D4-alt）：

- 实现复杂度高（要存多个候选 anchor + offset）
- chunk 顺序变化场景在本仓极少（only file rebase / external edit）
- 简单降级 + console.warn 让 bug 暴露而非隐藏

### D5：`data-chunk-id` 渲染处统一注入

`SessionDetail.svelte` 的 chunk 渲染处（user / ai / system / compact）原有的 `key={chunk.chunkId}` 已是稳定 ID，直接 `<div data-chunk-id={chunk.chunkId} ...>` 注入即可。

**注意**：AIChunk 的 chunkId 是 `responses[0].uuid`，UserChunk / System / Compact 的 chunkId 是 `uuid`——已在 `ui/CLAUDE.md::列表 / 详情自动刷新反闪烁三原则` 第 1 条沉淀过，不需要新约定。

不引入新 ID schema，不增加 IPC 字段。

### D6：`refreshDetail` 路径下的 `wasAtBottom` 改用 `atBottom` 共享逻辑

`refreshDetail()` 已有 `wasAtBottom = scrollTop + clientHeight >= scrollHeight - 16` 判断 + `tick()` 后 `scrollTop = scrollHeight` 重新粘底——本 change 提取共享 helper：

- `isAtBottom(el): boolean`：纯函数判断
- `pinToBottom(el): cleanup`：单次粘底（refresh 路径）/ 持续 pin（restore 路径）共用底层

**为什么不改 refresh 行为**：

- refresh 路径目前没有 lazy hydrate 时序问题（detail 替换时 chunk 已渲染，scrollHeight 稳定）
- 仅做代码复用，不改语义；refresh 仍单次粘底而非持续 pin

## Risks / Trade-offs

[Risk 1] anchor chunk 自身在视口顶且未 hydrate → scrollIntoView 把占位顶推到视口顶后，hydrate 让 anchor 高度增大，视觉上原内容下沉。
→ Mitigation：lazy markdown 的 IntersectionObserver `rootMargin: 200px` 让"距视口边界 ≤ 200 px 内"的 chunk 提前 hydrate。anchor 选「跨越视口顶或完全在视口内的第一个 chunk」（D2），保存时点这个 chunk 必然在视口内或紧挨视口顶 ≤ 200 px 范围内 → 保存时它通常已 hydrate；切回 cached path 时 IntersectionObserver 重建后会把它再次纳入 hydrate 优先窗口。极端情况（保存时点 anchor 才刚进 rootMargin 触发 hydrate 但 mermaid 还没渲染完，恢复时 mermaid 渲染让 anchor 高度增大）会有 1-2 帧抖动，可接受。

[Risk 2] `data-chunk-id` 选择器在 chunk 之间嵌套元素也含同名 attr 时误中。
→ Mitigation：仅在 chunk 顶层 `<div>` 注入 `data-chunk-id`；选择器 `conversationEl.querySelectorAll('[data-chunk-id]')` 直接子集化。代码 review 时 grep 确认无内层注入。

[Risk 3] MutationObserver pin 与浏览器 CSS scroll anchoring 互相干扰。
→ Mitigation：pin 期间 scrollTop 被 `scrollTop = scrollHeight` 持续覆盖，覆盖了 anchoring 的细微调整；pin 解绑后 anchoring 恢复正常作用域，没有冲突。MutationObserver 回调与 anchoring 都在 microtask 阶段，但 anchoring 本质是浏览器 layout 后的 scrollTop 调整，pin 在下一次 mutation 回调里覆盖回 scrollHeight。

[Risk 4] schema 改动让旧用户内存里的 `TabUIState`（如有 hot reload 残留）字段不匹配。
→ Mitigation：`TabUIState` 是 per-session 内存级（不持久化），重启即清；hot reload 由 vite 触发，开发期间残留没问题；用户场景不存在跨版本 state 兼容问题。

[Risk 5] vitest jsdom 不复现 lazy markdown IntersectionObserver 行为，单测覆盖不到锚点法的核心收益。
→ Mitigation：vitest 仅覆盖 `captureScrollAnchor` 选锚点逻辑（mock DOM rect）+ `atBottom` 阈值；真行为靠 Playwright e2e 三场景兜底（与 PR #223 相同测试金字塔分工）。

## Migration Plan

1. propose / design / specs delta / tasks → openspec validate --strict 通过
2. codex design 二审（状态机 / 锚点选择 / pin 终止条件 / spec scenario 边界）
3. 实现：`tabStore.svelte.ts` 字段调整 + `SessionDetail.svelte` 三函数改写 + chunk 渲染注入 `data-chunk-id`
4. 删除 `latestScrollTop` 与原 onMount 恢复 `scrollTop` 块（替换为 `restoreScrollAnchor`）
5. vitest + Playwright 三场景测试通过
6. PR push → wait-ci 与 codex PR 二审并行 → archive
7. 回滚：恢复 PR #223 的 `latestScrollTop` 三段式即可（scrollTop 字段保留期间无用，archive 后才彻底删）

## Open Questions

无。
