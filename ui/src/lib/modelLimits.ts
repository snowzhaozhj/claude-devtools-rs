import { parseModelString } from "./modelParser";

const MODEL_CONTEXT_LIMITS: Record<string, number> = {
  opus: 1_000_000,
  sonnet: 200_000,
  haiku: 200_000,
};

export function getModelContextLimit(model: string | null | undefined): number | null {
  if (!model) return null;
  const normalized = model.toLowerCase().trim();
  if (/\b1m\b|\[1m\]|-1m$/.test(normalized)) return 1_000_000;
  const info = parseModelString(model);
  if (!info) {
    if (normalized === "opus") return 1_000_000;
    if (normalized === "sonnet" || normalized === "haiku") return 200_000;
    return null;
  }
  return MODEL_CONTEXT_LIMITS[info.family] ?? null;
}

export interface ContextWindowUsage {
  inputTokens: number;
  contextLimit: number;
  ratio: number;
  model: string;
}

export type UsageLevel = "low" | "medium" | "high";

export function getUsageLevel(ratio: number): UsageLevel {
  if (ratio >= 0.8) return "high";
  if (ratio >= 0.5) return "medium";
  return "low";
}

interface UsageFields {
  input_tokens: number;
  cache_read_input_tokens?: number;
  cache_creation_input_tokens?: number;
}

function totalInputTokens(usage: UsageFields): number {
  return usage.input_tokens
    + (usage.cache_read_input_tokens ?? 0)
    + (usage.cache_creation_input_tokens ?? 0);
}

export function getLastAssistantUsage(
  chunks: Array<{ kind: string; responses?: Array<{ usage: UsageFields | null; model: string | null }> }>,
): ContextWindowUsage | null {
  for (let i = chunks.length - 1; i >= 0; i--) {
    const chunk = chunks[i];
    if (chunk.kind !== "ai" || !chunk.responses) continue;
    for (let j = chunk.responses.length - 1; j >= 0; j--) {
      const resp = chunk.responses[j];
      if (!resp.usage) continue;
      const total = totalInputTokens(resp.usage);
      if (total > 0) {
        let limit = getModelContextLimit(resp.model);
        if (!limit) return null;
        if (total > limit) limit = 1_000_000;
        return {
          inputTokens: total,
          contextLimit: limit,
          ratio: total / limit,
          model: resp.model!,
        };
      }
    }
  }
  return null;
}
