/**
 * 自适应输出限高 viewport 的键盘可访问性 attachment（spec
 * `session-display::输出内部滚动区域键盘可访问` /
 * `tool-viewer-routing::工具查看器内部滚动键盘可访问`）。
 *
 * 仅当内容沿任一轴实际溢出时，元素才进入 Tab 序列（tabindex=0 +
 * role=region + 可访问名）；未溢出时移除三者，不产生多余 Tab 停靠点。
 * 内容懒加载 / 高亮完成 / 窗口缩放导致的尺寸变化由 ResizeObserver +
 * MutationObserver 触发重测。
 *
 * 用法（Svelte attachment）：
 *   <div class="viewport" {@attach adaptiveScrollViewport(() => `${label}（可滚动）`)}>
 */
export function adaptiveScrollViewport(getLabel: () => string) {
  return (el: HTMLElement) => {
    const measure = () => {
      const overflowing =
        el.scrollHeight > el.clientHeight + 1 || el.scrollWidth > el.clientWidth + 1;
      if (overflowing) {
        el.tabIndex = 0;
        el.setAttribute("role", "region");
        el.setAttribute("aria-label", getLabel());
      } else {
        el.removeAttribute("tabindex");
        el.removeAttribute("role");
        el.removeAttribute("aria-label");
      }
    };
    measure();
    // jsdom（vitest 组件 smoke）无 ResizeObserver：跳过观察仅测一次，
    // 真浏览器路径不受影响。
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    const mo = new MutationObserver(measure);
    mo.observe(el, { childList: true, subtree: true, characterData: true });
    return () => {
      ro.disconnect();
      mo.disconnect();
    };
  };
}
