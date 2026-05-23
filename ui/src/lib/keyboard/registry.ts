/**
 * 全局快捷键注册中心。
 *
 * 单一 `Map<NormalizedKey, ShortcutSpec>` 索引；keydown 在 `document` 的 bubble phase
 * （`capture: false`）按顺序走守卫与命中：
 *   suspend → IME guard → repeat guard → normalize → 查表 → input 焦点守卫
 *   → handler 调用（返回 false 表示放行）→ preventDefault
 *
 * **边界**（D6 / 详 design.md）：
 * - 全局 mod-key 组合（`mod+...`）+ 全局非 mod 单键（`/` 聚焦）走 registry。
 * - **局部** keydown（Modal / Dropdown / CommandPalette / SearchBar / ImageBlock /
 *   TabContextMenu / SessionContextMenu / UpdatePopover / WorkspaceIndicator /
 *   MemoryView / Connection 内部的 Escape / Enter / 方向键）**SHALL NOT** 并入
 *   registry——这些与"关闭当前 surface"语义紧耦合，强行抽出会破坏 dispatcher 的纯函数
 *   性质。组件 SHALL 通过 `event.stopPropagation()` 阻止局部键继续冒泡到 dispatcher。
 * - **多 instance shared shortcut**（如 SessionDetail per-tab 的 jump-to-latest /
 *   in-session search）：D8 强制每条 binding 只能 `registerShortcut` 一次，多 instance
 *   注册同 ID 会触发"重复 ID 抛错"。解法是同层 controller（`PaneContainer.svelte`）
 *   作为单 instance 注册点，按 active tabId 经 `session-detail-handlers.ts` registry
 *   fanout 到对应实例回调；active tab 非该 surface 时 trigger 返回 `false` 让
 *   dispatcher 不 preventDefault，保留浏览器原生行为（如 mod+f 浏览器查找）。
 *
 * 详细行为契约见 `openspec/specs/keyboard-shortcuts/spec.md`。
 */

import {
  resolveBinding,
  normalize,
  type ShortcutBinding,
} from "../platform";

export type ShortcutCategory = "global" | "tabs" | "sidebar" | "search" | "session";

export interface ShortcutSpec {
  /** 唯一 ID，kebab-case，形如 `sidebar.toggle` / `tab.switch.1` */
  id: string;
  category: ShortcutCategory;
  /** 用户可读，简体中文 */
  description: string;
  defaultBinding: ShortcutBinding;
  /** 命中后调用；返回 `false` 表示不消费、放行（dispatcher 不 preventDefault） */
  handler: (e: KeyboardEvent) => boolean | void;
  /** 默认 false：input/textarea/contenteditable 焦点时跳过 */
  allowInInput?: boolean;
  /** 默认 true：handler 不返回 false 时调用 event.preventDefault() */
  preventDefault?: boolean;
}

export type ConflictError = {
  kind: "Conflict";
  conflictId: string;
  sourceId: string;
};

export type RegistryResult<T = void> =
  | { ok: true; value: T }
  | { ok: false; error: ConflictError };

interface RegistryEntry {
  spec: ShortcutSpec;
  /** Resolve 后的 NormalizedKey；空串表示该 spec 当前未占位（如 bootstrap 冲突） */
  effective: string;
}

// ---------------------------------------------------------------------------
// 模块级状态
// ---------------------------------------------------------------------------

const specs = new Map<string, RegistryEntry>(); // id -> entry
const keymap = new Map<string, string>(); // normalizedKey -> id
let pendingOverrides: Record<string, string> = {};
let suspendCount = 0;
let listenerInstalled = false;

let configLoadError: string | null = null;
let configLoadErrorListeners: Array<(err: string | null) => void> = [];

// ---------------------------------------------------------------------------
// dispatcher
// ---------------------------------------------------------------------------

function isInputFocused(): boolean {
  if (typeof document === "undefined") return false;
  const el = document.activeElement as HTMLElement | null;
  if (!el) return false;
  const tag = el.tagName?.toLowerCase();
  if (tag === "input" || tag === "textarea") return true;
  if (el.isContentEditable) return true;
  // jsdom 不实现 isContentEditable（undefined）；兜底看 .contentEditable 属性
  // （jsdom 下 div.contentEditable = "true" 写 property 但不映射到 attribute，
  //  所以 getAttribute("contenteditable") 也拿不到）
  const ce = el.contentEditable;
  if (ce === "true" || ce === "plaintext-only" || ce === "") return true;
  return false;
}

function dispatcher(event: KeyboardEvent): void {
  // suspend
  if (suspendCount > 0) return;
  // IME guard：composition 期间放行（Chinese / Japanese 输入法把 keyCode 设成 229）
  if (event.isComposing || event.keyCode === 229) return;
  // key-repeat guard：长按系统连发不重复 dispatch
  if (event.repeat) return;
  // normalize
  const normalized = normalize(event);
  if (!normalized) return;
  // 查表
  const id = keymap.get(normalized);
  if (!id) return;
  const entry = specs.get(id);
  if (!entry) return;
  const spec = entry.spec;
  // input 焦点守卫
  if (!spec.allowInInput && isInputFocused()) return;
  // handler 调用：返回 false → 不消费、放行
  const result = spec.handler(event);
  if (result === false) return;
  // preventDefault
  if (spec.preventDefault !== false) event.preventDefault();
}

function ensureListener(): void {
  if (listenerInstalled || typeof document === "undefined") return;
  document.addEventListener("keydown", dispatcher, { capture: false });
  listenerInstalled = true;
}

// ---------------------------------------------------------------------------
// 公开 API
// ---------------------------------------------------------------------------

/**
 * 注册一条快捷键 spec。返回 unregister 闭包供 onDestroy 调用。
 *
 * - 重复 ID 抛 `Error("Shortcut id already registered: <id>")`。
 * - 启动期 binding 冲突（含已有 spec 占位）记 console.warn 并把该 spec 设为"无 binding"
 *   占位（不抛错以免一条坏配置导致整个 dispatcher 不可用），用户可在 Settings 改键解冲突。
 * - 若 `pendingOverrides[id]` 存在，自动用 override 替代 defaultBinding。
 */
export function registerShortcut(spec: ShortcutSpec): () => void {
  if (specs.has(spec.id)) {
    throw new Error(`Shortcut id already registered: ${spec.id}`);
  }
  const overrideRaw = pendingOverrides[spec.id];
  const binding: ShortcutBinding =
    overrideRaw !== undefined ? overrideRaw : spec.defaultBinding;
  const effective = resolveBinding(binding);

  if (effective && keymap.has(effective)) {
    // 启动期冲突：保留 spec 元数据但不占位 keymap，UI 可在 Settings 中显示并改键
    // eslint-disable-next-line no-console
    console.warn(
      `[keyboard] binding conflict on "${effective}": "${spec.id}" vs "${keymap.get(
        effective,
      )}"; spec registered without binding`,
    );
    specs.set(spec.id, { spec, effective: "" });
    ensureListener();
    return () => unregister(spec.id);
  }

  specs.set(spec.id, { spec, effective });
  if (effective) keymap.set(effective, spec.id);
  ensureListener();
  return () => unregister(spec.id);
}

/**
 * 解除注册。已 unregister 的 ID 再调一次安全（no-op）。
 */
export function unregister(id: string): void {
  const entry = specs.get(id);
  if (!entry) return;
  if (entry.effective && keymap.get(entry.effective) === id) {
    keymap.delete(entry.effective);
  }
  specs.delete(id);
}

/**
 * 运行期更新某 ID 的 binding。冲突时返回 `Result.Err`，内存 keymap 不变。
 *
 * @param id          目标 ID
 * @param newBinding  新 binding（string 或 `{ mac, other }`）
 * @param overlay     可选 pendingOverrides 视图（id -> raw binding 字符串），用于
 *                    Settings 录键串行冲突检测。详 spec `pending overlay 串行冲突检测`。
 */
export function update(
  id: string,
  newBinding: ShortcutBinding,
  overlay?: Map<string, string>,
): RegistryResult {
  const entry = specs.get(id);
  if (!entry) {
    return {
      ok: false,
      error: { kind: "Conflict", conflictId: "<unknown-id>", sourceId: id },
    };
  }
  const newEffective = resolveBinding(newBinding);
  const conflictId = findConflictAt(newEffective, id, overlay);
  if (conflictId) {
    return {
      ok: false,
      error: { kind: "Conflict", conflictId, sourceId: id },
    };
  }
  if (entry.effective && keymap.get(entry.effective) === id) {
    keymap.delete(entry.effective);
  }
  entry.effective = newEffective;
  if (newEffective) keymap.set(newEffective, id);
  return { ok: true, value: undefined };
}

function findConflictAt(
  normalized: string,
  excludeId: string | undefined,
  overlay?: Map<string, string>,
): string | null {
  if (!normalized) return null;
  // 构建 effective view = 当前 keymap 应用 overlay 后的视图
  const view = new Map(keymap);
  if (overlay && overlay.size > 0) {
    // 第一遍：把 overlay 涉及到的所有 ID 在视图中的旧位置挪除
    for (const [overlayId] of overlay) {
      const e = specs.get(overlayId);
      if (e?.effective && view.get(e.effective) === overlayId) view.delete(e.effective);
    }
    // 第二遍：把 overlay 的新 binding 写入视图（excludeId 自身的 overlay 不写入）
    for (const [overlayId, overlayRaw] of overlay) {
      if (overlayId === excludeId) continue;
      // `__RESET__` sentinel：仅"剥旧位置"语义（第一遍已做），不写入新位置——
      // 该 id reset 后回到 default，default 是否冲突由其他 row 自己计算时发现。
      // 不跳过会让 resolveBinding("__RESET__") 产 "__RESET__" 假 normalized 写入
      // view，污染冲突检测。
      if (overlayRaw === "__RESET__") continue;
      const overlayNormalized = resolveBinding(overlayRaw);
      if (!overlayNormalized) continue;
      view.set(overlayNormalized, overlayId);
    }
  }
  // 排除自身在视图中的当前 binding（避免 update 自己的 binding 时把自己当冲突）
  if (excludeId) {
    const e = specs.get(excludeId);
    if (e?.effective && view.get(e.effective) === excludeId) {
      view.delete(e.effective);
    }
  }
  return view.get(normalized) ?? null;
}

/**
 * 检查 binding 是否被其他 ID 占用（可选合并 pending overlay 视图）。
 *
 * @param binding   待校验 binding（string 或 `{ mac, other }`）
 * @param excludeId 排除自身（避免改键时把自己旧 binding 当冲突）
 * @param overlay   pendingOverrides 视图（id -> raw binding 字符串）
 */
export function findConflict(
  binding: ShortcutBinding,
  excludeId?: string,
  overlay?: Map<string, string>,
): string | null {
  const normalized = resolveBinding(binding);
  return findConflictAt(normalized, excludeId, overlay);
}

/** 列出所有当前已注册 spec，按注册顺序。 */
export function listAll(): ShortcutSpec[] {
  return Array.from(specs.values()).map((e) => e.spec);
}

/** 引用计数 +1：dispatcher 进入 suspended 态。 */
export function suspend(): void {
  suspendCount += 1;
}

/** 引用计数 -1（地板 0）：当且仅当回到 0 时 dispatcher 恢复。 */
export function resume(): void {
  suspendCount = Math.max(0, suspendCount - 1);
}

export function isSuspended(): boolean {
  return suspendCount > 0;
}

// ---------------------------------------------------------------------------
// bootstrap / overrides
// ---------------------------------------------------------------------------

/**
 * 设置 pendingOverrides（启动期 customization 调用）。后续 `registerShortcut` 时
 * 自动用 override 替代 defaultBinding；若已有 spec 注册过，**不会** 自动 reapply
 * ——必须显式调 `applyOverrides` 触发 keymap rebuild。
 */
export function setPendingOverrides(overrides: Record<string, string>): void {
  pendingOverrides = { ...overrides };
}

/** 读当前 pendingOverrides snapshot（仅供测试 / Settings panel 初始化）。 */
export function getPendingOverrides(): Record<string, string> {
  return { ...pendingOverrides };
}

/**
 * 把 overrides 应用到所有已注册 spec 的 keymap：
 * - 每个 spec：若 overrides[id] 存在用 override，否则回到 defaultBinding
 * - 旧 keymap entry 全清，重建
 * - 同时刷新 pendingOverrides
 *
 * 用于 Save 提交后 / IPC 重试成功后 batch update。
 */
export function applyOverrides(overrides: Record<string, string>): void {
  pendingOverrides = { ...overrides };
  // 全量重建：清空 keymap 后按当前 specs + overrides 逐个写回
  keymap.clear();
  for (const [id, entry] of specs) {
    const overrideRaw = overrides[id];
    const binding: ShortcutBinding =
      overrideRaw !== undefined ? overrideRaw : entry.spec.defaultBinding;
    const effective = resolveBinding(binding);
    if (effective && keymap.has(effective)) {
      // eslint-disable-next-line no-console
      console.warn(
        `[keyboard] applyOverrides conflict on "${effective}": "${id}" vs "${keymap.get(
          effective,
        )}"; spec entry has no binding`,
      );
      entry.effective = "";
      continue;
    }
    entry.effective = effective;
    if (effective) keymap.set(effective, id);
  }
}

// ---------------------------------------------------------------------------
// IPC 失败 fallback：错误条 store
// ---------------------------------------------------------------------------

export function setConfigLoadError(reason: string | null): void {
  if (configLoadError === reason) return;
  configLoadError = reason;
  for (const fn of configLoadErrorListeners) fn(reason);
}

export function getConfigLoadError(): string | null {
  return configLoadError;
}

export function subscribeConfigLoadError(fn: (err: string | null) => void): () => void {
  configLoadErrorListeners.push(fn);
  return () => {
    configLoadErrorListeners = configLoadErrorListeners.filter((l) => l !== fn);
  };
}

// ---------------------------------------------------------------------------
// 测试 hook
// ---------------------------------------------------------------------------

/** 仅 vitest / playwright 用：清空 registry 状态 + 卸 listener。 */
export function _resetForTest(): void {
  if (listenerInstalled && typeof document !== "undefined") {
    document.removeEventListener("keydown", dispatcher, { capture: false });
  }
  listenerInstalled = false;
  specs.clear();
  keymap.clear();
  pendingOverrides = {};
  suspendCount = 0;
  configLoadError = null;
  configLoadErrorListeners = [];
}

/** 仅 vitest 用：返回 dispatcher 引用以便手动 fire 事件做单测。 */
export function _dispatcherForTest(): (event: KeyboardEvent) => void {
  return dispatcher;
}
