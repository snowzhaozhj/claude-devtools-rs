/**
 * SessionDetail per-tab 回调注册表（D8 单 binding 单 spec 1:1 关系的 fanout 实现）。
 *
 * **背景**：`session.jump-to-latest` / `search.in-session` 两条快捷键的语义都是
 * "对当前 active SessionDetail 实例执行某动作"，但 D8 强制每条 binding 只能
 * `registerShortcut` 一次（多 instance 同 ID 注册会触发"重复 ID 抛错"）。
 *
 * **解法**：单 instance 注册点（`PaneContainer.svelte`）持有共用 dispatcher；
 * 每个 SessionDetail mount 时把自己的回调按 `tabId` 投递到本表，PaneContainer
 * 的 dispatcher handler 通过 `getActiveTabId()` 找到当前应响应的 tab，再调对应
 * 回调；active tab 非 SessionDetail（如 Dashboard / Settings）时 trigger 返回
 * `false` 让 dispatcher 不 preventDefault，保留浏览器原生行为。
 *
 * 详 design.md::D8 与 tasks.md §6.1。
 */

export interface SessionDetailCallbacks {
  /** 跳到最新消息（绑定 mod+ArrowDown / Ctrl+End） */
  jumpToLatest: () => void;
  /** 打开会话内查找面板（绑定 mod+F） */
  openSearch: () => void;
}

const registry = new Map<string, SessionDetailCallbacks>();

/**
 * SessionDetail mount 时调一次；后续 active tab 切到此 tabId 时
 * dispatcher 会通过 trigger 调对应回调。
 *
 * 同 tabId 重复 register（如 file-change 触发 hot-reload 误重挂）会覆盖旧回调，
 * 不抛错——回调引用更新为最新 instance。
 */
export function registerSessionDetailHandlers(
  tabId: string,
  cb: SessionDetailCallbacks,
): void {
  registry.set(tabId, cb);
}

/** SessionDetail unmount 时调；空 tabId 安全 no-op。 */
export function unregisterSessionDetailHandlers(tabId: string): void {
  registry.delete(tabId);
}

/**
 * 触发指定 tab 的 jumpToLatest 回调。
 * @returns true = 已触发；false = 该 tabId 未注册（让 dispatcher 不 preventDefault）
 */
export function triggerJumpToLatest(tabId: string | null): boolean {
  if (!tabId) return false;
  const cb = registry.get(tabId);
  if (!cb) return false;
  cb.jumpToLatest();
  return true;
}

/**
 * 触发指定 tab 的 openSearch 回调。
 * @returns true = 已触发；false = 该 tabId 未注册
 */
export function triggerOpenSearch(tabId: string | null): boolean {
  if (!tabId) return false;
  const cb = registry.get(tabId);
  if (!cb) return false;
  cb.openSearch();
  return true;
}

/** 测试用：清空所有注册回调。 */
export function _resetForTest(): void {
  registry.clear();
}

/** 测试用：当前已注册的 tabId 集合（不含值）。 */
export function _registeredTabIdsForTest(): string[] {
  return Array.from(registry.keys());
}
