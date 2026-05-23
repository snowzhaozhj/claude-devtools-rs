<script lang="ts">
  /**
   * 键盘快捷键设置面板（spec keyboard-shortcuts::Settings）。
   *
   * **行为契约**（design.md::D3b "Save 唯一持久化路径" + spec scenario "Save / 丢弃 / 重置"）：
   * - 录键 commit 仅写 panel 内部 `pendingOverrides: Map<id, rawBinding>` overlay；
   *   **SHALL NOT** debounce 写 cdt-config / 改动 registry 实时 keymap。
   * - Save 走 `persistOverrides` IPC + `applyOverrides` registry batch update + 清空 overlay。
   *   写入前对每条 pending entry 再走一遍 `findConflict(binding, id, overlay)` 防御串行注入。
   * - 丢弃：清空 overlay（registry 不动）。
   * - 重置全部：clear overlay + `persistOverrides({})`（IPC 写空对象 → 内存回到 defaults）。
   *
   * **a11y**：错误条 role="alert"；conflict 视觉态由 ShortcutRow 自渲染（含 hint）；
   * Save / 丢弃 button disabled 与 pending 是否为空联动。
   *
   * **§11.3 + §M2 advisoryHints**（仅 `tab.close` 行）：
   * - macOS：`mod+w` 在 native menu / WKWebView 优先级高于 keydown handler，原版 TS 也是
   *   tooltip 提示而非真拦截 → "macOS 上 ⌘W 由系统优先处理，可能直接关闭窗口"。
   * - HTTP 浏览器（`!isTauriRuntime()`）：Chrome / Firefox 把 Ctrl+W 直接当成关闭 tab，
   *   keydown 拿不到事件 → "浏览器中 Ctrl+W 会被原生拦截，建议改用其他组合键"。
   *   两条独立判定，可同时显示（macOS Tauri：仅 Cmd+W 提示；macOS HTTP：两条都给）。
   */
  import { onMount, onDestroy, untrack } from "svelte";
  import {
    findConflict,
    subscribeConfigLoadError,
    getConfigLoadError,
    type ShortcutCategory,
  } from "../../lib/keyboard/registry";
  import {
    SHORTCUT_DEFAULTS,
    groupByCategory,
    getShortcutMeta,
    type ShortcutMeta,
  } from "../../lib/keyboard/defaults";
  import {
    persistOverrides,
    retryBootstrap,
  } from "../../lib/keyboard/customization";
  import { resolveBinding, isMac } from "../../lib/platform";
  import { isTauriRuntime } from "../../lib/runtime";
  import { ALERT_CIRCLE_SVG, ROTATE_CCW_SVG } from "../../lib/icons";
  import SettingsButton from "../../lib/components/SettingsButton.svelte";
  import ShortcutRow from "./ShortcutRow.svelte";

  interface Props {
    /** 初始 overrides（来自 cdt-config）；首次 mount 用来初始化 panel 的 baseline view。
     *  panel **不** 自己再调一次 `getConfig`——bootstrapOverrides 已经在 App.svelte 跑过。 */
    initialOverrides?: Record<string, string>;
  }

  let { initialOverrides = {} }: Props = $props();

  // ----- state -------------------------------------------------------------

  /** committed overrides（已经持久化的）—— Save 后从 pending 同步过来；初始 = initialOverrides。
   *  `untrack` 是必须的：组件 mount 时仅消费 props 一次快照（bootstrapOverrides 在 App.svelte
   *  早就跑过，此处不二次响应 prop 变化），避免 Svelte 5 `state_referenced_locally` 警告。 */
  let committed = $state<Record<string, string>>(untrack(() => ({ ...initialOverrides })));
  /** pendingOverrides overlay：录键 commit 写到这里；Save 才落 IPC */
  let pending = $state<Map<string, string>>(new Map());
  /** Save 进行中 → button disabled */
  let saving = $state(false);
  /** Save 失败的 inline 错误（与 IPC error banner 不同——后者是 bootstrap 阶段错误） */
  let saveError = $state<string | null>(null);
  /** 串行冲突自检触发的 inline 错误（Save 路径再走一遍 findConflict 时） */
  let preSaveConflict = $state<{ id: string; conflictId: string } | null>(null);
  /** IPC bootstrap 错误条 reason —— `subscribeConfigLoadError` 推送 */
  let configLoadError = $state<string | null>(getConfigLoadError());
  let retrying = $state(false);

  let unsubscribeConfigError: (() => void) | null = null;

  onMount(() => {
    unsubscribeConfigError = subscribeConfigLoadError((err) => {
      configLoadError = err;
    });
  });

  onDestroy(() => {
    unsubscribeConfigError?.();
  });

  // ----- 派生：每行的 effective binding ------------------------------------

  /**
   * 组合视图：committed 套上 pending overlay = 当前 panel 视角的"用户期望 binding"。
   * 录键 commit 改 pending → 该 derived 自动重算 → 行 currentBinding 即时反映新 binding。
   * 没 pending entry 的 row 走 committed[id] 兜底，再没有就走 defaultBinding。
   */
  const effectiveBindings = $derived.by(() => {
    const map = new Map<string, string>();
    for (const meta of SHORTCUT_DEFAULTS) {
      const overrideRaw = pending.has(meta.id)
        ? pending.get(meta.id)!
        : committed[meta.id];
      const raw = overrideRaw !== undefined ? overrideRaw : null;
      // 行展示用 normalized binding（与 KeyRecorderInput 内 formatShortcut 接受 string 一致）
      const normalized = raw !== null ? resolveBinding(raw) : resolveBinding(meta.defaultBinding);
      map.set(meta.id, normalized);
    }
    return map;
  });

  /** 每行的 default binding（当前平台展开后的 normalized）—— 用来判 reset 是否 disabled */
  const defaultBindings = $derived.by(() => {
    const map = new Map<string, string>();
    for (const meta of SHORTCUT_DEFAULTS) {
      map.set(meta.id, resolveBinding(meta.defaultBinding));
    }
    return map;
  });

  /** 5 category 分组（顺序保持 SHORTCUT_DEFAULTS 出现序） */
  const grouped = $derived.by(() => groupByCategory());

  /** 是否有未保存改动 */
  const hasPending = $derived(pending.size > 0);

  /** 是否有 commited override（用来判"重置全部"按钮是否 disabled） */
  const hasCommittedOverrides = $derived(Object.keys(committed).length > 0);

  // ----- helpers -----------------------------------------------------------

  /**
   * 算 row 的 conflict：传 binding + 自身 id + pending overlay（不含自身条目）三参数。
   * 注意：findConflict 返回的 conflictId 是 raw id（如 "tab.close"），需要查 meta 拿 description。
   */
  function computeConflict(
    id: string,
    binding: string,
  ): { conflictId: string; conflictLabel: string } | null {
    if (!binding) return null;
    // overlay 排除自身（findConflict 内部也会再排，但这里多一层防御保证语义清晰）
    const conflictId = findConflict(binding, id, pending);
    if (!conflictId) return null;
    const conflictMeta = getShortcutMeta(conflictId);
    return {
      conflictId,
      conflictLabel: conflictMeta?.description ?? conflictId,
    };
  }

  /**
   * 算 advisoryHints：仅 `tab.close` 行有；
   * - macOS：`Cmd+W 由系统优先处理` 提示
   * - HTTP 浏览器（!isTauriRuntime()）：`Ctrl+W 会被原生拦截` 提示
   *
   * 注意：判定基于 row id 而非具体 binding——即使用户改成别的组合键，行业语义仍是
   * "关闭 tab 对应的系统级争抢键"，故 advisory 只跟随 id 不跟随 binding。
   */
  function computeAdvisoryHints(id: string): string[] {
    if (id !== "tab.close") return [];
    const hints: string[] = [];
    if (isMac()) {
      hints.push("macOS 上 ⌘W 由系统优先处理，可能直接关闭窗口而非 tab；建议改用其他组合键");
    }
    if (!isTauriRuntime()) {
      hints.push("浏览器中 Ctrl+W / Cmd+W 会被浏览器原生拦截，无法被本应用接管");
    }
    return hints;
  }

  // ----- 录键 commit / reset 单条 -----------------------------------------

  function handleCommit(meta: ShortcutMeta, normalized: string) {
    // 清掉旧的 inline error（用户已经动了一次，重置 saveError）
    saveError = null;
    preSaveConflict = null;
    // 与 default 完全相同（normalized 比对）→ 等价于 reset，从 pending 删除
    const defaultNormalized = defaultBindings.get(meta.id);
    if (normalized === defaultNormalized) {
      // 但如果 committed 里有同 id 的 override，需要把"删 override"作为 pending 操作
      if (committed[meta.id] !== undefined) {
        pending = new Map(pending).set(meta.id, "__RESET__");
      } else {
        // commited 没有，pending 也只是录回了 default → 直接清 pending entry
        const next = new Map(pending);
        next.delete(meta.id);
        pending = next;
      }
      return;
    }
    pending = new Map(pending).set(meta.id, normalized);
  }

  function handleReset(meta: ShortcutMeta) {
    saveError = null;
    preSaveConflict = null;
    if (committed[meta.id] !== undefined) {
      // 已持久化的 override → 用 sentinel 标记 Save 时把 key 删掉
      pending = new Map(pending).set(meta.id, "__RESET__");
    } else {
      // 仅 pending 里录过 → 清掉 pending entry 即可
      const next = new Map(pending);
      next.delete(meta.id);
      pending = next;
    }
  }

  // ----- Save / 丢弃 / 重置全部 ------------------------------------------

  /**
   * Save：走 `persistOverrides`（IPC + applyOverrides registry rebuild）。
   * 写入前对每条 pending entry 再走一遍 findConflict（spec "保存路径串行冲突防御"）。
   *
   * pending 里 "__RESET__" sentinel 表示"删 override 回到 default"——构造最终 overrides
   * 时跳过该 id 即可（mergeOverrides 自动过滤幽灵 / 空串）。
   */
  async function handleSave() {
    if (saving || !hasPending) return;
    saveError = null;
    preSaveConflict = null;
    // 构造最终 overrides snapshot：committed 套 pending（"__RESET__" 删 key）
    const finalOverrides: Record<string, string> = { ...committed };
    for (const [id, raw] of pending) {
      if (raw === "__RESET__") {
        delete finalOverrides[id];
      } else {
        finalOverrides[id] = raw;
      }
    }
    // 防御：对 pending 里每条新 binding 再走一遍 findConflict
    // overlay 用 finalOverrides 视角（不含 __RESET__ sentinel）
    const overlayMap = new Map<string, string>();
    for (const [id, raw] of Object.entries(finalOverrides)) {
      overlayMap.set(id, raw);
    }
    for (const [id, raw] of pending) {
      if (raw === "__RESET__") continue;
      const conflictId = findConflict(raw, id, overlayMap);
      if (conflictId) {
        preSaveConflict = { id, conflictId };
        return;
      }
    }
    saving = true;
    try {
      await persistOverrides(finalOverrides);
      committed = { ...finalOverrides };
      pending = new Map();
    } catch (e) {
      saveError = e instanceof Error ? e.message : String(e);
    } finally {
      saving = false;
    }
  }

  function handleDiscard() {
    saveError = null;
    preSaveConflict = null;
    pending = new Map();
  }

  async function handleResetAll() {
    if (saving) return;
    saveError = null;
    preSaveConflict = null;
    saving = true;
    try {
      await persistOverrides({});
      committed = {};
      pending = new Map();
    } catch (e) {
      saveError = e instanceof Error ? e.message : String(e);
    } finally {
      saving = false;
    }
  }

  async function handleRetryBootstrap() {
    if (retrying) return;
    retrying = true;
    try {
      await retryBootstrap();
      // bootstrap 成功后 configLoadError 由 subscribe 自动清空
    } finally {
      retrying = false;
    }
  }

  // ----- 文案 --------------------------------------------------------------

  const CATEGORY_LABEL: Record<ShortcutCategory, string> = {
    global: "全局",
    tabs: "标签页",
    sidebar: "侧栏",
    search: "搜索",
    session: "会话",
  };

  const CATEGORY_DESC: Record<ShortcutCategory, string> = {
    global: "应用级唤起",
    tabs: "标签页与 pane 管理",
    sidebar: "侧栏与导航",
    search: "查找与定位",
    session: "会话内交互",
  };

  // category 渲染顺序：global → tabs → sidebar → search → session
  const CATEGORY_ORDER: ShortcutCategory[] = [
    "global",
    "tabs",
    "sidebar",
    "search",
    "session",
  ];
</script>

<div class="kbd-panel">
  {#if configLoadError}
    <div class="banner banner-error" role="alert">
      <span class="banner-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html ALERT_CIRCLE_SVG}</svg>
      </span>
      <span class="banner-text">无法加载快捷键自定义：{configLoadError}（已回落到内置默认值）</span>
      <SettingsButton variant="ghost" size="sm" disabled={retrying} onClick={handleRetryBootstrap}>
        {retrying ? "重试中…" : "重试"}
      </SettingsButton>
    </div>
  {/if}

  {#if saveError}
    <div class="banner banner-error" role="alert">
      <span class="banner-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html ALERT_CIRCLE_SVG}</svg>
      </span>
      <span class="banner-text">保存失败：{saveError}</span>
    </div>
  {/if}

  {#if preSaveConflict}
    <div class="banner banner-error" role="alert">
      <span class="banner-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html ALERT_CIRCLE_SVG}</svg>
      </span>
      <span class="banner-text">
        保存前自检：「{getShortcutMeta(preSaveConflict.id)?.description ?? preSaveConflict.id}」
        与「{getShortcutMeta(preSaveConflict.conflictId)?.description ?? preSaveConflict.conflictId}」
        存在串行冲突，请先解决后再保存
      </span>
    </div>
  {/if}

  {#if hasPending}
    <div class="pending-bar" role="status">
      <span class="pending-text">有 {pending.size} 项未保存改动</span>
      <div class="pending-actions">
        <SettingsButton variant="ghost" size="sm" disabled={saving} onClick={handleDiscard}>
          丢弃
        </SettingsButton>
        <SettingsButton variant="primary" size="sm" disabled={saving} onClick={handleSave}>
          {saving ? "保存中…" : "保存"}
        </SettingsButton>
      </div>
    </div>
  {/if}

  {#each CATEGORY_ORDER as category (category)}
    {@const items = grouped[category]}
    {#if items.length > 0}
      <section class="category">
        <header class="category-header">
          <h3 class="category-title">{CATEGORY_LABEL[category]}</h3>
          <p class="category-desc">{CATEGORY_DESC[category]}</p>
        </header>
        <div class="category-body">
          {#each items as meta (meta.id)}
            {@const currentBinding = effectiveBindings.get(meta.id) ?? ""}
            {@const defaultEffective = defaultBindings.get(meta.id) ?? ""}
            {@const conflict = computeConflict(meta.id, currentBinding)}
            {@const advisoryHints = computeAdvisoryHints(meta.id)}
            <ShortcutRow
              {meta}
              {currentBinding}
              {defaultEffective}
              {conflict}
              {advisoryHints}
              disabled={saving}
              onCommit={(b) => handleCommit(meta, b)}
              onReset={() => handleReset(meta)}
            />
          {/each}
        </div>
      </section>
    {/if}
  {/each}

  <footer class="panel-footer">
    <SettingsButton
      variant="ghost"
      size="sm"
      disabled={saving || (!hasCommittedOverrides && !hasPending)}
      onClick={handleResetAll}
      ariaLabel="重置全部为内置默认"
      title={hasCommittedOverrides || hasPending ? "重置全部为内置默认" : "已经全部是默认值"}
    >
      {#snippet icon()}
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html ROTATE_CCW_SVG}</svg>
      {/snippet}
      重置全部
    </SettingsButton>
  </footer>
</div>

<style>
  .kbd-panel {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  /* error / pending banners 复用 SettingsView 的 .banner 样式语义；这里给 panel scope 局部覆盖 */
  .banner {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 14px;
    border: 1px solid var(--color-border);
    border-radius: 8px;
    background: var(--color-surface-raised);
    color: var(--color-text-secondary);
    font-size: 13px;
    line-height: 1.5;
  }
  .banner-icon {
    flex-shrink: 0;
    display: inline-flex;
    width: 16px;
    height: 16px;
    margin-top: 1px;
  }
  .banner-icon :global(svg) {
    width: 16px;
    height: 16px;
  }
  .banner-text {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .banner-error {
    border-color: color-mix(in oklch, var(--color-danger-bright) 35%, var(--color-border));
    background: color-mix(in oklch, var(--color-danger-bright) 8%, var(--color-surface));
    color: var(--tool-result-error-text);
  }

  .pending-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    border: 1px solid color-mix(in oklch, var(--color-switch-on) 35%, var(--color-border));
    border-radius: 8px;
    background: color-mix(in oklch, var(--color-switch-on) 8%, var(--color-surface));
    color: var(--color-text);
    font-size: 13px;
    position: sticky;
    top: 0;
    z-index: 1;
  }
  .pending-text {
    flex: 1;
    min-width: 0;
  }
  .pending-actions {
    display: flex;
    gap: 8px;
  }

  .category {
    display: flex;
    flex-direction: column;
  }
  .category-header {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 0 4px 12px;
  }
  .category-title {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: var(--color-text);
    letter-spacing: -0.005em;
  }
  .category-desc {
    margin: 0;
    font-size: 13px;
    color: var(--color-text-secondary);
    line-height: 1.5;
  }
  .category-body {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--color-border);
    border-radius: 8px;
    background: var(--color-surface);
    overflow: hidden;
  }
  .category-body :global(> *:not(:last-child)) {
    border-bottom: 1px solid var(--color-border-subtle);
  }

  .panel-footer {
    display: flex;
    justify-content: flex-end;
    padding: 4px 4px 0;
  }
</style>
