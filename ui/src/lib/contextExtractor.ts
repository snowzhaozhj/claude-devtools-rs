/**
 * Context Panel 数据层：把后端 6 类 `ContextInjection` 序列化结果 narrow 成
 * 判别联合，保留所有字段（不再压扁成简陋的 `ContextEntry`）。
 *
 * 后端 serde 形态：`{ category: "claude-md" | ..., ...fields }`（internally-tagged）。
 *
 * spec：`openspec/specs/context-tracking/spec.md` + `session-display` Context
 * Panel Requirements。
 */

import type { SessionDetail } from "./api";

// ---------------------------------------------------------------------------
// 6 类 ContextInjection 类型定义（与后端 cdt-core::context.rs 对齐）
// ---------------------------------------------------------------------------

export type ClaudeMdScope = "enterprise" | "user" | "project" | "directory";

export interface ClaudeMdInjection {
  category: "claude-md";
  id: string;
  path: string;
  displayName: string;
  scope: ClaudeMdScope;
  estimatedTokens: number;
  firstSeenTurnIndex: number;
}

export interface MentionedFileInjection {
  category: "mentioned-file";
  id: string;
  path: string;
  displayName: string;
  estimatedTokens: number;
  firstSeenTurnIndex: number;
  firstSeenInGroup: string;
  exists: boolean;
}

export interface ToolTokenBreakdown {
  toolName: string;
  tokenCount: number;
  isError: boolean;
  toolUseId?: string;
}

export interface ToolOutputInjection {
  category: "tool-output";
  id: string;
  turnIndex: number;
  aiGroupId: string;
  estimatedTokens: number;
  toolCount: number;
  toolBreakdown: ToolTokenBreakdown[];
}

export type ThinkingTextKind = "thinking" | "text";

export interface ThinkingTextBreakdownItem {
  type: ThinkingTextKind;
  tokenCount: number;
}

export interface ThinkingTextInjection {
  category: "thinking-text";
  id: string;
  turnIndex: number;
  aiGroupId: string;
  estimatedTokens: number;
  breakdown: ThinkingTextBreakdownItem[];
}

export type TaskCoordinationKind = "send-message" | "task-tool" | "teammate-message";

export interface TaskCoordinationBreakdownItem {
  type: TaskCoordinationKind;
  tokenCount: number;
  label: string;
  toolName?: string;
}

export interface TaskCoordinationInjection {
  category: "task-coordination";
  id: string;
  turnIndex: number;
  aiGroupId: string;
  estimatedTokens: number;
  breakdown: TaskCoordinationBreakdownItem[];
}

export interface UserMessageInjection {
  category: "user-message";
  id: string;
  turnIndex: number;
  aiGroupId: string;
  estimatedTokens: number;
  textPreview: string;
}

export type ContextInjection =
  | ClaudeMdInjection
  | MentionedFileInjection
  | ToolOutputInjection
  | ThinkingTextInjection
  | TaskCoordinationInjection
  | UserMessageInjection;

// ---------------------------------------------------------------------------
// 分类颜色（Ranked 视图 / category chip 用）
// ---------------------------------------------------------------------------

export interface CategoryColor {
  bg: string;
  text: string;
  label: string;
}

export const CATEGORY_COLORS: Record<ContextInjection["category"], CategoryColor> = {
  "claude-md": { bg: "rgba(99, 102, 241, 0.15)", text: "#818cf8", label: "CLAUDE.md" },
  "mentioned-file": { bg: "rgba(52, 211, 153, 0.15)", text: "#34d399", label: "File" },
  "tool-output": { bg: "rgba(251, 191, 36, 0.15)", text: "#fbbf24", label: "Tool" },
  "thinking-text": { bg: "rgba(167, 139, 250, 0.15)", text: "#a78bfa", label: "Thinking" },
  "task-coordination": { bg: "rgba(251, 146, 60, 0.15)", text: "#fb923c", label: "Team" },
  "user-message": { bg: "rgba(96, 165, 250, 0.15)", text: "#60a5fa", label: "User" },
};

// ---------------------------------------------------------------------------
// 解析：把后端 raw `unknown[]` narrow 成判别联合
// ---------------------------------------------------------------------------

/**
 * 把后端序列化结果 narrow 为 `ContextInjection`。返回 null 表示 category 未知
 * （忽略该条，不阻断渲染）。不做信息丢失变换。
 */
export function parseInjection(raw: unknown): ContextInjection | null {
  if (!raw || typeof raw !== "object") return null;
  const obj = raw as Record<string, unknown>;
  switch (obj.category) {
    case "claude-md":
    case "mentioned-file":
    case "tool-output":
    case "thinking-text":
    case "task-coordination":
    case "user-message":
      return obj as unknown as ContextInjection;
    default:
      return null;
  }
}

export function parseInjections(raw: unknown[] | undefined | null): ContextInjection[] {
  if (!Array.isArray(raw)) return [];
  const out: ContextInjection[] = [];
  for (const item of raw) {
    const inj = parseInjection(item);
    if (inj) out.push(inj);
  }
  return out;
}

// ---------------------------------------------------------------------------
// Phase 过滤：按 selectedPhase（null = Latest）从 SessionDetail 取对应 injections
// ---------------------------------------------------------------------------

/**
 * 按 selectedPhase 取目标 phase 的 injections。
 * - `selectedPhase == null` → latest phase（取 `injectionsByPhase[最大 phaseNumber]`；
 *   fallback 到 `contextInjections`；再 fallback 到空数组）
 * - `selectedPhase = N` → 取 `injectionsByPhase[String(N)]`；缺失则空数组
 *
 * 老后端不返回 `injectionsByPhase` 时，selectedPhase 必须为 null（Phase Selector
 * 不显示，UI 上不会触发非 null 路径）。
 */
export function selectActivePhaseInjections(
  detail: SessionDetail,
  selectedPhase: number | null,
): ContextInjection[] {
  const byPhase = detail.injectionsByPhase;
  if (selectedPhase !== null) {
    const key = String(selectedPhase);
    return parseInjections((byPhase?.[key] ?? []) as unknown[]);
  }
  // Latest fallback 链
  if (byPhase) {
    const phaseNumbers = Object.keys(byPhase)
      .map((k) => Number(k))
      .filter((n) => Number.isFinite(n));
    if (phaseNumbers.length > 0) {
      const latest = Math.max(...phaseNumbers);
      return parseInjections(byPhase[String(latest)] as unknown[]);
    }
  }
  return parseInjections(detail.contextInjections);
}

// ---------------------------------------------------------------------------
// CLAUDE.md scope 分组：4 scope → 3 组（Global / Project / Directory）
// ---------------------------------------------------------------------------

export interface ClaudeMdGroups {
  global: ClaudeMdInjection[];
  project: ClaudeMdInjection[];
  directory: ClaudeMdInjection[];
}

export function groupClaudeMdByScope(injections: ContextInjection[]): ClaudeMdGroups {
  const out: ClaudeMdGroups = { global: [], project: [], directory: [] };
  for (const inj of injections) {
    if (inj.category !== "claude-md") continue;
    if (inj.scope === "enterprise" || inj.scope === "user") out.global.push(inj);
    else if (inj.scope === "project") out.project.push(inj);
    else if (inj.scope === "directory") out.directory.push(inj);
  }
  return out;
}

// ---------------------------------------------------------------------------
// Token 总和 / 格式化
// ---------------------------------------------------------------------------

export function sumTokens(injections: ContextInjection[]): number {
  let sum = 0;
  for (const inj of injections) sum += inj.estimatedTokens;
  return sum;
}

export function formatTokens(n: number): string {
  if (n >= 1e6) return (n / 1e6).toFixed(1) + "M";
  if (n >= 1e3) return (n / 1e3).toFixed(1) + "k";
  return String(n);
}
