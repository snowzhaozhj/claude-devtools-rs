/** 应用主题到 document.documentElement */
export function applyTheme(theme: string): void {
  document.documentElement.setAttribute("data-theme", theme);
}
