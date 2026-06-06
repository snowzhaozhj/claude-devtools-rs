// Lazy markdown render：用单一 IntersectionObserver 把 marked + highlight.js +
// DOMPurify 推迟到节点进入视口（含 200px rootMargin）后再触发。视口外节点
// 仅渲染高度估算占位。设计 + 风险参见
// `openspec/changes/session-detail-lazy-render/design.md`。
//
// 紧急回滚：把 LAZY_MARKDOWN_ENABLED 改 false，调用方一行 fallback 即可。

import { renderMarkdown } from "./render";

/** 紧急回滚开关：false 时 observe() 立即同步渲染（旧行为）。 */
export const LAZY_MARKDOWN_ENABLED = true;

/**
 * 补偿标志：手动 scrollTop 调整期间置 true，scroll listener 应跳过
 * 该帧的 anchor 捕获和 isFar 计算，避免补偿触发的 scroll event 产生
 * 误判（如 bottom-pin、isFar 重显）。
 */
let _compensating = false;
export function isScrollCompensating(): boolean {
  return _compensating;
}

type Kind = "user" | "ai" | "system" | "thinking" | "output" | "slash" | "teammate";

/**
 * 估算占位高度（px）。进入视口后真实内容覆盖 min-height，
 * 偏差被 IntersectionObserver 的 200px rootMargin 吸收。
 */
export function estimatePlaceholderHeight(text: string, kind: Kind): number {
  const len = text.length;
  switch (kind) {
    case "user":
    case "ai":
    case "thinking":
    case "output":
    case "slash":
    case "teammate":
      return Math.max(60, Math.ceil((len / 80) * 22));
    case "system": {
      const lines = (text.match(/\n/g)?.length ?? 0) + 1;
      return Math.max(60, lines * 18);
    }
  }
}

interface LazyMarkdownObserver {
  observe(
    el: HTMLElement,
    text: string,
    onRendered?: (el: HTMLElement) => void | Promise<void>,
  ): void;
  flushAll(): void;
  disconnect(): void;
}

/**
 * 创建一个 lazy markdown observer。`root` 必须是 conversation 容器（拥有
 * `overflow-y: auto`），否则 IntersectionObserver 会以 viewport 计算可见性，
 * 导致面板内滚动不触发回调。
 */
export function createLazyMarkdownObserver(
  root: HTMLElement,
): LazyMarkdownObserver {
  if (!LAZY_MARKDOWN_ENABLED) {
    return {
      observe(el, text, onRendered) {
        renderInto(el, text, onRendered);
      },
      flushAll() {},
      disconnect() {},
    };
  }

  const pending = new Map<
    Element,
    { text: string; onRendered?: (el: HTMLElement) => void | Promise<void> }
  >();

  // ResizeObserver 捕获异步高度变化（mermaid 图表、图片加载等）。
  // 只对视口上方元素补偿 scrollTop，稳定后自动 unobserve。
  const resizeLastHeight = new WeakMap<Element, number>();
  const resizeStableTimers = new WeakMap<Element, ReturnType<typeof setTimeout>>();
  const RESIZE_STABLE_MS = 500;

  const ro = new ResizeObserver((entries) => {
    let totalDelta = 0;
    const rootRect = root.getBoundingClientRect();
    for (const entry of entries) {
      const el = entry.target as HTMLElement;
      const elRect = el.getBoundingClientRect();
      // 只补偿完全在视口上方的元素
      if (elRect.bottom > rootRect.top) continue;

      const oldH = resizeLastHeight.get(el) ?? 0;
      const newH = el.offsetHeight;
      if (newH !== oldH) {
        totalDelta += newH - oldH;
        resizeLastHeight.set(el, newH);
      }

      const existing = resizeStableTimers.get(el);
      if (existing !== undefined) clearTimeout(existing);
      resizeStableTimers.set(el, setTimeout(() => {
        ro.unobserve(el);
        resizeStableTimers.delete(el);
        resizeLastHeight.delete(el);
      }, RESIZE_STABLE_MS));
    }
    if (totalDelta !== 0) {
      _compensating = true;
      root.scrollTop += totalDelta;
      requestAnimationFrame(() => { _compensating = false; });
    }
  });

  const io = new IntersectionObserver(
    (entries) => {
      // Phase 1: 收集需要渲染的元素
      const toRender: Array<{ el: HTMLElement; text: string; onRendered?: (el: HTMLElement) => void | Promise<void> }> = [];
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const el = entry.target as HTMLElement;
        const data = pending.get(el);
        if (!data) continue;
        toRender.push({ el, text: data.text, onRendered: data.onRendered });
      }
      if (toRender.length === 0) return;

      const rootRect = root.getBoundingClientRect();

      // Phase 2: 读取视口上方元素的旧高度（geometry reads 全在 DOM writes 前）
      const aboveItems: Array<{ el: HTMLElement; oldHeight: number }> = [];
      for (const item of toRender) {
        const rect = item.el.getBoundingClientRect();
        if (rect.bottom <= rootRect.top) {
          aboveItems.push({ el: item.el, oldHeight: item.el.offsetHeight });
        }
      }

      // Phase 3: 批量 DOM 写入
      for (const item of toRender) {
        renderInto(item.el, item.text, item.onRendered);
        pending.delete(item.el);
        io.unobserve(item.el);
      }

      // Phase 4: 读新高度并一次性补偿 scrollTop
      if (aboveItems.length > 0) {
        let totalDelta = 0;
        for (const item of aboveItems) {
          const newHeight = item.el.offsetHeight;
          totalDelta += newHeight - item.oldHeight;
          resizeLastHeight.set(item.el, newHeight);
          ro.observe(item.el);
        }
        if (totalDelta !== 0) {
          _compensating = true;
          root.scrollTop += totalDelta;
          requestAnimationFrame(() => { _compensating = false; });
        }
      }

      // 视口内元素也注册 RO（滚过后可能变为"上方"，异步变化仍需补偿）
      for (const item of toRender) {
        if (!aboveItems.some(a => a.el === item.el)) {
          resizeLastHeight.set(item.el, item.el.offsetHeight);
          ro.observe(item.el);
        }
      }
    },
    { root, rootMargin: "200px 0px", threshold: 0 },
  );

  return {
    observe(el, text, onRendered) {
      if (el.dataset.rendered === "1") {
        if (onRendered) {
          void Promise.resolve(onRendered(el));
        }
        return;
      }
      pending.set(el, { text, onRendered });
      io.observe(el);
    },
    flushAll() {
      if (pending.size === 0) return;
      for (const [el, data] of pending) {
        renderInto(el as HTMLElement, data.text, data.onRendered);
        io.unobserve(el);
      }
      pending.clear();
    },
    disconnect() {
      io.disconnect();
      ro.disconnect();
      pending.clear();
    },
  };
}

function renderInto(
  el: HTMLElement,
  text: string,
  onRendered?: (el: HTMLElement) => void | Promise<void>,
): void {
  el.innerHTML = renderMarkdown(text);
  el.dataset.rendered = "1";
  if (onRendered) {
    void Promise.resolve(onRendered(el));
  }
}
