import type { AppConfig } from "./api";

/** 把 config.display.fontSans / fontMono 应用到 :root；空/null 时移除属性让 app.css 默认 token 生效。 */
export function applyFonts(config: AppConfig | null | undefined): void {
  const root = document.documentElement;
  applyOne(root, "--font-sans", config?.display?.fontSans);
  applyOne(root, "--font-mono", config?.display?.fontMono);
}

function applyOne(root: HTMLElement, varName: string, value: string | null | undefined): void {
  if (value && value.trim()) {
    root.style.setProperty(varName, value);
  } else {
    root.style.removeProperty(varName);
  }
}
