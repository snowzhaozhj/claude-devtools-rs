/*
 * ContextMenuItem 类型定义（Phase 2 扩展）。
 *
 * spec: openspec/specs/frontend-context-menu/spec.md
 *   ::Requirement ContextMenuItem 类型扩展
 *
 * Phase 1 旧版本定义在 `components/AppContextMenu.svelte` module script，
 * Phase 2 抽离到本独立模块以便：
 * - menu-items.ts 等 factory 模块直接 import type 不绕道 .svelte
 * - 单测对比类型定义稳定锚点
 * - 旧调用点（TabBar / Sidebar）通过 `lib/contextMenu.svelte.ts` 的 re-export
 *   保持引用路径不变（向后兼容）
 *
 * 全部 Phase 2 新字段为 optional，Phase 1 已落地的 Sidebar / Tab 右键菜单
 * 无需改动即兼容（详 spec ContextMenuItem 类型扩展 Requirement 第一段）。
 */

export interface ContextMenuItem {
  // ---- Phase 1 既有字段 ----

  /** separator=true 时其它字段忽略 */
  separator?: boolean;
  /** 显示文本；有 pathLabel 时被覆盖 */
  label?: string;
  /** 可选 lucide SVG path 字符串，作为菜单项前导图标渲染 */
  icon?: string;
  /** 不可用 item，渲染为 opacity 0.45 + aria-disabled，键盘可经过但 Enter no-op */
  disabled?: boolean;
  /** 危险动作（destructive），文字色染 --color-danger。Phase 2 暂不引入 */
  danger?: boolean;
  /** 触发动作；disabled / separator / 含 submenu 的 item 不调用 */
  action?: () => void;
  /** action 触发后短暂展示 feedback label，再关菜单（典型"已复制!"600ms） */
  feedback?: { label: string; durationMs?: number };

  // ---- Phase 2 新增字段 ----

  /**
   * 右侧灰色快捷键 hint 文本，如 "⌘C"。仅 display 不绑定真实快捷键。
   * 含 submenu 时该字段被忽略（chevron 与 shortcut hint 互斥）。
   * 视觉规格：`--color-text-muted` + `var(--font-mono)` `11px` `400`。
   */
  shortcut?: string;

  /**
   * 二级菜单数组。非空时 AppContextMenu 渲染 `›` chevron 指示器，且：
   * - `action` 与 `shortcut` 字段被忽略（点击/hover 200ms 后弹 submenu）
   * - submenu 渲染深度上限 2（Phase 2 仅用一层）
   */
  submenu?: ContextMenuItem[];

  /**
   * 语义分类，用于 factory 内部 separator 自动插入逻辑：相邻 item kind
   * 不同时插入 `{ separator: true }`。AppContextMenu 渲染层 SHALL **不**
   * 消费此字段（无视觉变化），保证 kind 是纯语义标记不影响渲染契约。
   */
  kind?: "copy" | "external";

  /**
   * 路径类 label 的中段截断形态（D-V6）。CSS 原生不支持中段 ellipsis，
   * 由 factory 预处理生成 `{ short, full }`：
   * - `short`：截断后显示文本（典型 `~/Rustro…/menu-items.ts`，总长 ≤ 50）
   * - `full`：完整路径，渲染层用作 `title` tooltip
   *
   * 有 pathLabel 时覆盖 `label` 渲染（AppContextMenu 优先读 pathLabel.short）。
   */
  pathLabel?: { short: string; full: string };
}
