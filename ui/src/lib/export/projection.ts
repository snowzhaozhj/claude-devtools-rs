import type { SessionDetail, Chunk, AIChunk, SemanticStep, ToolExecution, SubagentProcess } from "../api";
import type { ExportOptions } from "./types";

export interface ProjectedSessionDetail {
  sessionId: string;
  projectId: string;
  chunks: Chunk[];
  metrics: SessionDetail["metrics"];
  metadata: SessionDetail["metadata"];
  isOngoing: boolean;
  title?: string | null;
}

export function projectSessionDetail(
  detail: SessionDetail,
  options: ExportOptions,
): ProjectedSessionDetail {
  const chunks = detail.chunks.map((chunk) => projectChunk(chunk, options));
  return {
    sessionId: detail.sessionId,
    projectId: detail.projectId,
    chunks,
    metrics: detail.metrics,
    metadata: detail.metadata,
    isOngoing: detail.isOngoing,
    title: detail.title,
  };
}

function projectChunk(chunk: Chunk, options: ExportOptions): Chunk {
  if (chunk.kind !== "ai") return chunk;

  const ai = chunk as AIChunk;
  const semanticSteps = projectSemanticSteps(ai.semanticSteps, options);
  const toolExecutions = projectToolExecutions(ai.toolExecutions, options);
  const subagents = projectSubagents(ai.subagents, options);

  return { ...ai, semanticSteps, toolExecutions, subagents };
}

function projectSemanticSteps(steps: SemanticStep[], options: ExportOptions): SemanticStep[] {
  if (options.includeThinking) return steps;
  return steps.filter((s) => s.kind !== "thinking");
}

function projectToolExecutions(execs: ToolExecution[], options: ExportOptions): ToolExecution[] {
  if (options.toolOutputMode === "full") return execs;
  if (options.toolOutputMode === "name-only") {
    return execs.map((e) => ({
      ...e,
      input: {},
      output: { kind: "missing" as const },
    }));
  }
  return execs.map((e) => truncateToolExecution(e, options.toolOutputMaxLength));
}

function truncateToolExecution(exec: ToolExecution, maxLen: number): ToolExecution {
  const output = exec.output;
  if (!output || output.kind === "missing") return exec;

  if (output.kind === "text") {
    if (!output.text || output.text.length <= maxLen) return exec;
    return {
      ...exec,
      output: { kind: "text", text: output.text.slice(0, maxLen) + "... (truncated)" },
    };
  }

  if (output.kind === "structured") {
    const str = JSON.stringify(output.value);
    if (str.length <= maxLen) return exec;
    return {
      ...exec,
      output: { kind: "text", text: str.slice(0, maxLen) + "... (truncated)" },
    };
  }

  return exec;
}

function projectSubagents(procs: SubagentProcess[], options: ExportOptions): SubagentProcess[] {
  if (options.includeSubagents) return procs;
  return [];
}
