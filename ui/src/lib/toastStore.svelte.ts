/*
 * 全局 Toast Store —— 应用级可见反馈最小实现。
 *
 * 用例：context menu dispatch 失败（IPC error / 路径校验失败 / clipboard 拒绝等）
 * 时显示用户可见的错误反馈，避免 console.error 被吞掉。
 *
 * 设计点（codex PR 二审 MEDIUM #4 修订）：
 * - 模块级 `$state`：单例，跨 surface 共享；ToastContainer.svelte 订阅渲染
 * - 自动消失：默认 4s 后 dismiss；用户可 hover/click 取消（暂不实现，未来 polish）
 * - level：`error` 红色 / `info` 默认；不引入 success（菜单复制反馈走 item.feedback）
 * - 不依赖任何第三方库（zero deps），完全 inline 实现避免引入 toast 库
 *
 * 与 Phase 1 SessionMetaMenu 的 inline toast 区别：那是 menu-scoped 复制反馈
 *（"已复制!"），生命周期绑 menu；本模块是应用级错误 toast，跨菜单 / 跨 surface 共享。
 */

let nextId = 1;

export type ToastLevel = "error" | "info";

export interface ToastEntry {
  id: number;
  level: ToastLevel;
  message: string;
  /** epoch ms */
  createdAt: number;
}

class ToastStore {
  toasts = $state<ToastEntry[]>([]);

  push(message: string, level: ToastLevel = "error", durationMs = 4000): number {
    const id = nextId++;
    const entry: ToastEntry = {
      id,
      level,
      message,
      createdAt: Date.now(),
    };
    this.toasts.push(entry);
    if (durationMs > 0) {
      window.setTimeout(() => this.dismiss(id), durationMs);
    }
    return id;
  }

  dismiss(id: number): void {
    const idx = this.toasts.findIndex((t) => t.id === id);
    if (idx >= 0) {
      this.toasts.splice(idx, 1);
    }
  }

  clear(): void {
    this.toasts.splice(0, this.toasts.length);
  }
}

export const toastStore = new ToastStore();

/** 测试 helper：单测 reset 单例。 */
export function resetToastStoreForTesting(): void {
  toastStore.clear();
}
