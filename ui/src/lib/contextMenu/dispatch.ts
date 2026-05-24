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
import { toastStore } from "../toastStore.svelte";

/**
 * 单例：app 启动时初始化一次的 dispatch 闭包。
 *
 * 性能：单例避免每个右键事件都 new 一个对象；纯闭包无状态，多 surface 共享安全。
 */
let cachedDispatch: MenuItemDispatch | null = null;

/**
 * 提取 IPC 错误的人类可读 message（兼容 ApiError / Error / unknown 三种形态）。
 *
 * codex PR 二审 MEDIUM #4 修订：dispatch 失败必须有用户可见反馈
 * （toast），不能仅 console.error 吞掉。
 */
function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object") {
    const obj = e as Record<string, unknown>;
    // ApiError shape: { code, message }
    if (typeof obj.message === "string" && obj.message.length > 0) {
      return obj.message;
    }
    if (typeof obj.toString === "function") {
      const s = obj.toString();
      if (s !== "[object Object]") return s;
    }
  }
  return String(e);
}

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
        const msg = errorMessage(e);
        console.error("[contextMenu] clipboard.writeText failed:", e);
        toastStore.push(`复制失败：${msg}`);
      }
    },
    openInEditor: async (path: string, line?: number, column?: number) => {
      try {
        await invoke<void>("open_in_editor", { path, line, column });
      } catch (e) {
        const msg = errorMessage(e);
        console.error("[contextMenu] open_in_editor failed:", e);
        toastStore.push(`在编辑器打开失败：${msg}`);
      }
    },
    openInTerminal: async (path: string) => {
      try {
        await invoke<void>("open_in_terminal", { path });
      } catch (e) {
        const msg = errorMessage(e);
        console.error("[contextMenu] open_in_terminal failed:", e);
        toastStore.push(`在终端打开失败：${msg}`);
      }
    },
    revealInDir: async (path: string) => {
      try {
        await revealItemInDir(path);
      } catch (e) {
        const msg = errorMessage(e);
        console.error("[contextMenu] revealItemInDir failed:", e);
        toastStore.push(`在文件管理器中显示失败：${msg}`);
      }
    },
    openUrl: async (url: string) => {
      try {
        await openUrl(url);
      } catch (e) {
        const msg = errorMessage(e);
        console.error("[contextMenu] openUrl failed:", e);
        toastStore.push(`打开浏览器失败：${msg}`);
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
