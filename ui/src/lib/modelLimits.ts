import { parseModelString } from "./modelParser";

const MODEL_CONTEXT_LIMITS: Record<string, number> = {
  opus: 200_000,
  sonnet: 200_000,
  haiku: 200_000,
};

export function getModelContextLimit(model: string | null | undefined): number | null {
  const info = parseModelString(model);
  if (!info) return null;
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

export function getLastAssistantUsage(
  chunks: Array<{ kind: string; responses?: Array<{ usage: { input_tokens: number } | null; model: string | null }> }>,
): ContextWindowUsage | null {
  for (let i = chunks.length - 1; i >= 0; i--) {
    const chunk = chunks[i];
    if (chunk.kind !== "ai" || !chunk.responses) continue;
    for (let j = chunk.responses.length - 1; j >= 0; j--) {
      const resp = chunk.responses[j];
      if (resp.usage && resp.usage.input_tokens > 0) {
        const limit = getModelContextLimit(resp.model);
        if (!limit) return null;
        return {
          inputTokens: resp.usage.input_tokens,
          contextLimit: limit,
          ratio: resp.usage.input_tokens / limit,
          model: resp.model!,
        };
      }
    }
  }
  return null;
}
