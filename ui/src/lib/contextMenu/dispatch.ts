/*
 * MenuItemContext.dispatch 真实 IPC 实现（与 menu-items.ts factory 间接调用对应）。
 *
 * 抽出独立模块，便于：
 * - main.ts 启动时 build 一次，传给 selectionMenu 的 ctxProvider
 * - surface 组件（SessionDetail / ToolViewer 等）build 局部 ctx 时复用
 * - 单测里换 mock dispatch 不绑定到真 IPC（factory 单测已用此模式）
 */

import { invoke } from "@tauri-apps/api/core";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import type { MenuItemDispatch } from "./menu-items";

/**
 * 单例：app 启动时初始化一次的 dispatch 闭包。
 *
 * 性能：单例避免每个右键事件都 new 一个对象；纯闭包无状态，多 surface 共享安全。
 */
let cachedDispatch: MenuItemDispatch | null = null;

export function getMenuItemDispatch(): MenuItemDispatch {
  if (cachedDispatch) return cachedDispatch;
  cachedDispatch = {
    copyToClipboard: async (text: string) => {
      // 优先 navigator.clipboard（Tauri WKWebView / 现代浏览器都支持）；
      // 失败时 fallback execCommand('copy')——但 secure context 下两者等价
      // 不再分流。
      try {
        await navigator.clipboard.writeText(text);
      } catch (e) {
        console.error("[contextMenu] clipboard.writeText failed:", e);
      }
    },
    openInEditor: async (path: string, line?: number, column?: number) => {
      try {
        await invoke<void>("open_in_editor", { path, line, column });
      } catch (e) {
        console.error("[contextMenu] open_in_editor failed:", e);
      }
    },
    openInTerminal: async (path: string) => {
      try {
        await invoke<void>("open_in_terminal", { path });
      } catch (e) {
        console.error("[contextMenu] open_in_terminal failed:", e);
      }
    },
    revealInDir: async (path: string) => {
      try {
        await revealItemInDir(path);
      } catch (e) {
        console.error("[contextMenu] revealItemInDir failed:", e);
      }
    },
    openUrl: async (url: string) => {
      try {
        await openUrl(url);
      } catch (e) {
        console.error("[contextMenu] openUrl failed:", e);
      }
    },
  };
  return cachedDispatch;
}

/**
 * 测试 helper：单测可调此函数 reset 单例。
 */
export function resetMenuItemDispatchForTesting(): void {
  cachedDispatch = null;
}
