// Lazy markdown render：用单一 IntersectionObserver 把 marked + highlight.js +
// DOMPurify 推迟到节点进入视口（含 200px rootMargin）后再触发。视口外节点
// 仅渲染高度估算占位。设计 + 风险参见
// `openspec/changes/session-detail-lazy-render/design.md`。
//
// 紧急回滚：把 LAZY_MARKDOWN_ENABLED 改 false，调用方一行 fallback 即可。

import { renderMarkdown } from "./render";

/** 紧急回滚开关：false 时 observe() 立即同步渲染（旧行为）。 */
export const LAZY_MARKDOWN_ENABLED = true;

type Kind = "user" | "ai" | "system" | "thinking" | "output" | "slash";

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
      // 假设折行 80 字符/行，line-height ~22 px
      return Math.max(60, Math.ceil((len / 80) * 22));
    case "system": {
      const lines = (text.match(/\n/g)?.length ?? 0) + 1;
      return Math.max(60, lines * 18);
    }
  }
}

interface LazyMarkdownObserver {
  /**
   * 注册一个占位元素 + 它对应的 markdown 文本。元素首次进入视口时同步
   * 调用 `renderMarkdown(text)` 并 innerHTML 注入；标记 `data-rendered="1"`
   * 后从 IntersectionObserver `unobserve`。`onRendered` 回调用于触发
   * mermaid 等后处理（仅扫该元素子树）。
   */
  observe(
    el: HTMLElement,
    text: string,
    onRendered?: (el: HTMLElement) => void | Promise<void>,
  ): void;
  /** SessionDetail unmount 时调用。 */
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
      disconnect() {
        // no-op
      },
    };
  }

  const pending = new WeakMap<
    Element,
    { text: string; onRendered?: (el: HTMLElement) => void | Promise<void> }
  >();

  const io = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const el = entry.target as HTMLElement;
        const data = pending.get(el);
        if (!data) continue;
        renderInto(el, data.text, data.onRendered);
        pending.delete(el);
        io.unobserve(el);
      }
    },
    { root, rootMargin: "200px 0px", threshold: 0 },
  );

  return {
    observe(el, text, onRendered) {
      if (el.dataset.rendered === "1") return;
      pending.set(el, { text, onRendered });
      io.observe(el);
    },
    disconnect() {
      io.disconnect();
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
