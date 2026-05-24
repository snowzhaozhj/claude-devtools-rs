// 锚点法滚动状态保存/恢复（change `tab-scroll-restore-anchor`）
//
// 不再保存绝对 scrollTop 数值——lazy markdown 占位高度 ≠ 真实渲染高度，
// 切回 cached path 时 scrollHeight ≈ Σ 占位 < 切走时点，浏览器把
// `scrollTop = savedValue` clamp 到 max → 底部场景偏差几千 px。社区参照
// react-virtuoso `getState/restoreStateFrom` / TanStack Virtual `scrollToIndex`。
//
// 抽到独立 module 而非 SessionDetail 内部 helper：纯 DOM 操作 + 无副作用 +
// 可单独 vitest 覆盖（jsdom 可 mock getBoundingClientRect / scrollHeight）。

export interface ScrollAnchorState {
  /** 距底 ≤ 16 px 视为粘底 */
  atBottom: boolean;
  /** 视口顶第一个 bottom > containerTop 的 chunk 的 chunkId；atBottom=true 时为 null */
  anchorChunkId: string | null;
  /** anchor 元素 rect.top - container rect.top；可正（视口内）可负（跨越视口顶） */
  anchorOffsetPx: number;
}

/** 距底 ≤ 16 px 视为粘底——与 PR #223 旧实现 wasAtBottom 阈值同源 */
export function isAtBottom(el: HTMLElement): boolean {
  return el.scrollTop + el.clientHeight >= el.scrollHeight - 16;
}

/**
 * 捕获当前滚动锚点。算法：
 * - 粘底（distanceFromBottom ≤ 16）→ atBottom=true，不需要 chunk anchor
 * - 否则找「跨越视口顶或完全在视口内的第一个 chunk」（rect.bottom > containerTop）
 *   → 用其 chunkId 与 `rect.top - containerTop` 作为锚点
 * - 兜底（无 chunk 命中）→ 三件套全 0/null，恢复时降级到首屏顶部
 */
export function captureScrollAnchor(conversationEl: HTMLElement | undefined): ScrollAnchorState {
  if (!conversationEl) return { atBottom: false, anchorChunkId: null, anchorOffsetPx: 0 };
  if (isAtBottom(conversationEl)) {
    return { atBottom: true, anchorChunkId: null, anchorOffsetPx: 0 };
  }
  const containerRect = conversationEl.getBoundingClientRect();
  const chunkEls = conversationEl.querySelectorAll<HTMLElement>("[data-chunk-id]");
  for (const el of chunkEls) {
    const rect = el.getBoundingClientRect();
    // bottom > containerTop 意味着该 chunk 至少有一部分仍在视口内或之下。
    // DOM 顺序遍历下，第一个命中的元素要么跨越视口顶（rect.top < containerTop，
    // offset 负），要么是完全在视口内的第一个（rect.top ≥ containerTop，offset 正）
    if (rect.bottom > containerRect.top + 1) {
      return {
        atBottom: false,
        anchorChunkId: el.dataset.chunkId ?? null,
        anchorOffsetPx: rect.top - containerRect.top,
      };
    }
  }
  return { atBottom: false, anchorChunkId: null, anchorOffsetPx: 0 };
}

/**
 * MutationObserver 粘底 pin 状态机。返回 cleanup 函数。
 *
 * - 单次 scrollTop = scrollHeight（除非 `skipInitialJump`，调用方已在 smooth scroll
 *   到位 / 'auto' 已同步落地，再 hard set 会取消 smooth animation 或视觉冗余）
 * - 监听 conversationEl 子树（dataset.rendered / innerHTML / childList / characterData）
 *   每次 mutation 重写 scrollTop 并 reset 200 ms 稳定 timer
 * - 用户主动 scroll（distanceFromBottom > 16）/ 200 ms 内无 mutation / 5 s 上限 三路终止
 *
 * ResizeObserver(conversationEl) 不行：容器外框尺寸 fixed，lazy hydrate 改的是
 * 内部子节点高度 → 不触发。MutationObserver `subtree: true` 才能覆盖。
 */
export function startBottomPin(
  conversationEl: HTMLElement,
  opts: { skipInitialJump?: boolean } = {},
): () => void {
  const el = conversationEl;
  if (!opts.skipInitialJump) {
    el.scrollTop = el.scrollHeight;
  }

  let pinning = true;
  let stableTimer: ReturnType<typeof setTimeout> | null = null;
  const STABLE_MS = 200;
  const HARD_LIMIT_MS = 5000;

  function stopPin() {
    if (!pinning) return;
    pinning = false;
    mo.disconnect();
    el.removeEventListener("scroll", onScrollPin);
    if (stableTimer !== null) clearTimeout(stableTimer);
    clearTimeout(hardLimitTimer);
  }
  function resetStableTimer() {
    if (stableTimer !== null) clearTimeout(stableTimer);
    stableTimer = setTimeout(stopPin, STABLE_MS);
  }
  const mo = new MutationObserver(() => {
    if (!pinning) return;
    el.scrollTop = el.scrollHeight;
    resetStableTimer();
  });
  mo.observe(el, {
    subtree: true,
    attributes: true,
    attributeFilter: ["data-rendered"],
    childList: true,
    characterData: true,
  });
  function onScrollPin() {
    if (!pinning) return;
    // pin 自身写 scrollTop 也触发 scroll event——但 distanceFromBottom 此刻 ≤ 0
    // 不会误判；用户主动滚才会让 distanceFromBottom > 16
    const dist = el.scrollHeight - el.scrollTop - el.clientHeight;
    if (dist > 16) stopPin();
  }
  el.addEventListener("scroll", onScrollPin, { passive: true });
  const hardLimitTimer = setTimeout(stopPin, HARD_LIMIT_MS);
  resetStableTimer();
  return stopPin;
}

/**
 * 按锚点状态恢复滚动。SHALL 在 `await tick()` 之后调，确保 `<div class="conversation">`
 * 已 mount + chunk 元素已渲染 placeholder 节点。
 *
 * 三路分支：
 * - 粘底 → 启动 bottom pin 状态机，返回的 cleanup SHALL 由调用方持有引用
 * - anchorChunkId 命中 → scrollIntoView + offset 微调；若 conversation scrollHeight
 *   不足导致 scrollIntoView 被 clamp（lazy markdown 尚未 hydrate / 子树高度还在增长
 *   等场景，spec `tab-management::滚动位置恢复 - 切回时 lazy chunks 尚未 hydrate`），
 *   返回 MutationObserver 状态机 cleanup，等子树后续 mutation 触发 re-align
 * - 兜底 / anchor 失效 → console.warn，保留浏览器默认 scrollTop=0，返回 null
 */
export function restoreScrollAnchor(
  conversationEl: HTMLElement | undefined,
  state: ScrollAnchorState,
): (() => void) | null {
  if (!conversationEl) return null;

  if (state.atBottom) {
    return startBottomPin(conversationEl);
  }
  if (!state.anchorChunkId) {
    return null;
  }
  const el = conversationEl;
  const target = el.querySelector<HTMLElement>(
    `[data-chunk-id="${CSS.escape(state.anchorChunkId)}"]`,
  );
  if (!target) {
    console.warn(`[scroll-restore] anchorChunkId not found: ${state.anchorChunkId}, falling back to top`);
    return null;
  }

  const ALIGN_TOLERANCE_PX = 16;
  const HARD_LIMIT_MS = 5000;
  let active = true;
  let lastAppliedScrollTop = -1;

  function applyAnchor(): void {
    // scrollIntoView 把 anchor 顶贴齐视口顶 → rect.top - containerTop ≈ 0；
    // 减去 offset：offset 正 → scrollTop 减小 → 内容下移让 anchor 离开视口顶；
    // offset 负 → scrollTop 增大 → anchor 继续被滚出视口
    target!.scrollIntoView({ block: "start" });
    el.scrollTop -= state.anchorOffsetPx;
    lastAppliedScrollTop = el.scrollTop;
  }

  function isAligned(): boolean {
    const c = el.getBoundingClientRect();
    const r = target!.getBoundingClientRect();
    return Math.abs((r.top - c.top) - state.anchorOffsetPx) <= ALIGN_TOLERANCE_PX;
  }

  applyAnchor();

  // 首次 apply 已对齐（scrollHeight 充足）→ 不挂监听
  if (isAligned()) return null;

  // 未对齐：scrollIntoView 被 clamp（典型场景：mount 时 conversation 内 chunks 真实
  // 高度短，scrollHeight < clientHeight + anchor 累计位置）。挂 MutationObserver 等
  // 子树 mutation（lazy markdown hydrate / 外部 append / 新内容到达）后 re-apply，
  // 直到对齐或上限。spec `tab-management::滚动位置恢复 - 切回时 lazy chunks 尚未
  // hydrate` 明确要求 hydrate 后 anchor 视觉位置不偏离。
  function stop(): void {
    if (!active) return;
    active = false;
    mo.disconnect();
    el.removeEventListener("scroll", onScroll);
    clearTimeout(hardTimer);
  }

  function onScroll(): void {
    if (!active) return;
    // applyAnchor 内部写 scrollTop 也触发 scroll event——lastAppliedScrollTop 记
    // 最后一次 apply 后的真值；用户主动 scroll 让 scrollTop 偏离 last apply > 容差
    // → 视为用户中断，停止 retry 让用户接管
    if (Math.abs(el.scrollTop - lastAppliedScrollTop) > ALIGN_TOLERANCE_PX) {
      stop();
    }
  }

  const mo = new MutationObserver(() => {
    if (!active) return;
    applyAnchor();
    if (isAligned()) stop();
  });
  mo.observe(el, {
    subtree: true,
    attributes: true,
    attributeFilter: ["data-rendered"],
    childList: true,
    characterData: true,
  });
  el.addEventListener("scroll", onScroll, { passive: true });
  const hardTimer = setTimeout(stop, HARD_LIMIT_MS);

  return stop;
}
