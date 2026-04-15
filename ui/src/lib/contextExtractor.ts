import type { SessionDetail } from "./api";

export type ContextCategory = "user" | "claudemd" | "tools" | "system" | "thinking" | "task";

export interface ContextEntry {
  category: ContextCategory;
  categoryKey: string; // 原始 kebab-case，用于颜色映射
  label: string;
  preview: string;
  path?: string; // claude-md / mentioned-file 用
  estimatedTokens: number;
}

export interface CategoryColor {
  bg: string;
  text: string;
  label: string;
}

export const CATEGORY_COLORS: Record<string, CategoryColor> = {
  "claude-md":         { bg: "rgba(99, 102, 241, 0.15)",  text: "#818cf8", label: "CLAUDE.md" },
  "mentioned-file":    { bg: "rgba(52, 211, 153, 0.15)",  text: "#34d399", label: "File" },
  "tool-output":       { bg: "rgba(251, 191, 36, 0.15)",  text: "#fbbf24", label: "Tool" },
  "thinking-text":     { bg: "rgba(167, 139, 250, 0.15)", text: "#a78bfa", label: "Thinking" },
  "task-coordination": { bg: "rgba(251, 146, 60, 0.15)",  text: "#fb923c", label: "Team" },
  "user-message":      { bg: "rgba(96, 165, 250, 0.15)",  text: "#60a5fa", label: "User" },
};

function truncatePreview(text: string, max = 120): string {
  if (text.length <= max) return text;
  return text.slice(0, max) + "…";
}

/** 后端 ContextInjection serde 格式: { category: "claude-md", ...fields } */
function injectionToEntry(inj: Record<string, unknown>): ContextEntry | null {
  const cat = String(inj.category ?? "");

  switch (cat) {
    case "claude-md":
      return {
        category: "claudemd",
        categoryKey: "claude-md",
        label: String(inj.displayName ?? inj.path ?? "CLAUDE.md"),
        preview: truncatePreview(String(inj.path ?? "").replace(/^\/Users\/[^/]+/, "~")),
        path: String(inj.path ?? ""),
        estimatedTokens: Number(inj.estimatedTokens ?? 0),
      };
    case "mentioned-file":
      return {
        category: "claudemd",
        categoryKey: "mentioned-file",
        label: String(inj.displayName ?? inj.path ?? "file"),
        preview: truncatePreview(String(inj.path ?? "").replace(/^\/Users\/[^/]+/, "~")),
        path: String(inj.path ?? ""),
        estimatedTokens: Number(inj.estimatedTokens ?? 0),
      };
    case "tool-output": {
      const breakdown = (inj.toolBreakdown ?? []) as Array<Record<string, unknown>>;
      const names = breakdown.map(b => String(b.toolName ?? "")).filter(Boolean).join(", ");
      return {
        category: "tools",
        categoryKey: "tool-output",
        label: names || `${inj.toolCount ?? 0} tools`,
        preview: `${inj.toolCount ?? 0} tool calls`,
        estimatedTokens: Number(inj.estimatedTokens ?? 0),
      };
    }
    case "thinking-text":
      return {
        category: "thinking",
        categoryKey: "thinking-text",
        label: "Thinking + Text",
        preview: `Turn ${inj.turnIndex ?? "?"}`,
        estimatedTokens: Number(inj.estimatedTokens ?? 0),
      };
    case "task-coordination":
      return {
        category: "task",
        categoryKey: "task-coordination",
        label: "Task Coordination",
        preview: `Turn ${inj.turnIndex ?? "?"}`,
        estimatedTokens: Number(inj.estimatedTokens ?? 0),
      };
    case "user-message":
      return {
        category: "user",
        categoryKey: "user-message",
        label: "User",
        preview: truncatePreview(String(inj.textPreview ?? "")),
        estimatedTokens: Number(inj.estimatedTokens ?? 0),
      };
    default:
      return null;
  }
}

export function extractContext(detail: SessionDetail): ContextEntry[] {
  const raw = detail.contextInjections;
  if (!Array.isArray(raw)) return [];
  const entries: ContextEntry[] = [];
  for (const item of raw) {
    const entry = injectionToEntry(item as Record<string, unknown>);
    if (entry) entries.push(entry);
  }
  return entries;
}

export function groupByCategory(entries: ContextEntry[]): Map<ContextCategory, ContextEntry[]> {
  const map = new Map<ContextCategory, ContextEntry[]>();
  for (const entry of entries) {
    const list = map.get(entry.category) || [];
    list.push(entry);
    map.set(entry.category, list);
  }
  return map;
}

const CATEGORY_LABELS: Record<ContextCategory, string> = {
  user: "User Messages",
  claudemd: "CLAUDE.md Files",
  tools: "Tool Outputs",
  system: "System",
  thinking: "Thinking + Text",
  task: "Task Coordination",
};

export function categoryLabel(cat: ContextCategory): string {
  return CATEGORY_LABELS[cat];
}
