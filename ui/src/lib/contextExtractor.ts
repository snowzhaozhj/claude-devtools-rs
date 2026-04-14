import type { SessionDetail } from "./api";

export type ContextCategory = "user" | "claudemd" | "tools" | "system" | "thinking" | "task";

export interface ContextEntry {
  category: ContextCategory;
  label: string;
  preview: string;
  estimatedTokens: number;
}

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
        label: String(inj.displayName ?? inj.path ?? "CLAUDE.md"),
        preview: truncatePreview(String(inj.path ?? "").replace(/^\/Users\/[^/]+/, "~")),
        estimatedTokens: Number(inj.estimatedTokens ?? 0),
      };
    case "mentioned-file":
      return {
        category: "claudemd",
        label: String(inj.displayName ?? inj.path ?? "file"),
        preview: truncatePreview(String(inj.path ?? "").replace(/^\/Users\/[^/]+/, "~")),
        estimatedTokens: Number(inj.estimatedTokens ?? 0),
      };
    case "tool-output": {
      const breakdown = (inj.toolBreakdown ?? []) as Array<Record<string, unknown>>;
      const names = breakdown.map(b => String(b.toolName ?? "")).filter(Boolean).join(", ");
      return {
        category: "tools",
        label: names || `${inj.toolCount ?? 0} tools`,
        preview: `${inj.toolCount ?? 0} tool calls`,
        estimatedTokens: Number(inj.estimatedTokens ?? 0),
      };
    }
    case "thinking-text":
      return {
        category: "thinking",
        label: "Thinking + Text",
        preview: `Turn ${inj.turnIndex ?? "?"}`,
        estimatedTokens: Number(inj.estimatedTokens ?? 0),
      };
    case "task-coordination":
      return {
        category: "task",
        label: "Task Coordination",
        preview: `Turn ${inj.turnIndex ?? "?"}`,
        estimatedTokens: Number(inj.estimatedTokens ?? 0),
      };
    case "user-message":
      return {
        category: "user",
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
