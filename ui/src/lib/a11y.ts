/**
 * 给 `role="button"` 的非原生交互元素补键盘可达性：Enter / Space 触发等价 click 行为。
 *
 * 用于卡片 header / chip 等历史上写成 `<div onclick>` + `svelte-ignore a11y_*` 的
 * disclosure 元素。项目禁止 `<button>` 嵌套（见 `ui/CLAUDE.md::Svelte 5 陷阱`），故
 * 保留原 `<div>` + `role="button" tabindex="0"`，由本 helper 承载键盘激活。
 */
export function activateOnKey(e: KeyboardEvent, action: () => void): void {
  if (e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    action();
  }
}
