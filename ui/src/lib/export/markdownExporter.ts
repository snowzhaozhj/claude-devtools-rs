import type { SessionDetail, Chunk, AIChunk, UserChunk, SystemChunk, CompactChunk, SemanticStep, ToolExecution, SubagentProcess } from "../api";
import type { ExportOptions } from "./types";
import { projectSessionDetail } from "./projection";
import { userChunkToMarkdown, aiChunkToMarkdown, toolExecToMarkdown } from "../contextMenu/markdown";
import { cleanDisplayText } from "../toolHelpers";

export function exportAsMarkdown(detail: SessionDetail, options: ExportOptions): string {
  const projected = projectSessionDetail(detail, options);
  const parts: string[] = [];

  parts.push(buildHeader(projected.title, projected.sessionId));
  parts.push(buildMetadataTable(detail));
  parts.push("---\n");

  let turnIndex = 0;
  for (const chunk of projected.chunks) {
    turnIndex++;
    parts.push(renderChunk(chunk, turnIndex, options));
  }

  return parts.join("\n");
}

function buildHeader(title: string | null | undefined, sessionId: string): string {
  const displayTitle = title || `Session ${sessionId.slice(0, 8)}`;
  return `# ${displayTitle}\n`;
}

function buildMetadataTable(detail: SessionDetail): string {
  const rows: [string, string][] = [];

  rows.push(["Session ID", detail.sessionId]);
  if (detail.metadata.cwd) rows.push(["工作目录", detail.metadata.cwd]);
  rows.push(["消息数", String(detail.metrics.message_count)]);
  rows.push(["进行中", detail.isOngoing ? "是" : "否"]);

  if (detail.metadata.last_modified) {
    const d = new Date(detail.metadata.last_modified);
    rows.push(["最后修改", d.toISOString()]);
  }

  let table = "| 字段 | 值 |\n|------|-----|\n";
  for (const [k, v] of rows) {
    table += `| ${k} | ${v} |\n`;
  }
  return table;
}

function renderChunk(chunk: Chunk, index: number, options: ExportOptions): string {
  switch (chunk.kind) {
    case "user":
      return renderUserChunk(chunk as UserChunk, index);
    case "ai":
      return renderAIChunk(chunk as AIChunk, index, options);
    case "system":
      return renderSystemChunk(chunk as SystemChunk, index);
    case "compact":
      return renderCompactChunk(chunk as CompactChunk, index);
    default:
      return "";
  }
}

function renderUserChunk(chunk: UserChunk, index: number): string {
  const content = userChunkToMarkdown(chunk);
  return `## Turn ${index} — User\n\n${content}\n\n---\n`;
}

function renderAIChunk(chunk: AIChunk, index: number, options: ExportOptions): string {
  const parts: string[] = [];
  parts.push(`## Turn ${index} — Assistant\n`);

  for (const step of chunk.semanticSteps) {
    parts.push(renderSemanticStep(step, options));
  }

  for (const exec of chunk.toolExecutions) {
    parts.push(renderToolExecution(exec));
  }

  if (options.includeSubagents && chunk.subagents.length > 0) {
    for (const sub of chunk.subagents) {
      parts.push(renderSubagent(sub));
    }
  }

  parts.push("---\n");
  return parts.join("\n");
}

function renderSemanticStep(step: SemanticStep, options: ExportOptions): string {
  if (step.kind === "thinking") {
    if (!options.includeThinking) return "";
    return `> [thinking] ${step.text}\n`;
  }
  if (step.kind === "text") {
    const cleaned = cleanDisplayText(step.text);
    return cleaned ? `${cleaned}\n` : "";
  }
  if (step.kind === "interruption") {
    return `*[interrupted]* ${step.text}\n`;
  }
  return "";
}

function renderToolExecution(exec: ToolExecution): string {
  const md = toolExecToMarkdown(exec);
  return `### Tool: ${exec.toolName}\n\n${md}\n`;
}

function renderSubagent(sub: SubagentProcess): string {
  const desc = sub.description || sub.rootTaskDescription || "subagent";
  const type = sub.subagentType ? ` (${sub.subagentType})` : "";
  const duration = sub.durationMs ? ` — ${Math.round(sub.durationMs / 1000)}s` : "";
  return `### Subagent: ${desc}${type}${duration}\n\n`;
}

function renderSystemChunk(chunk: SystemChunk, index: number): string {
  const text = cleanDisplayText(chunk.contentText);
  return `## Turn ${index} — System\n\n*${text}*\n\n---\n`;
}

function renderCompactChunk(chunk: CompactChunk, index: number): string {
  return `## Turn ${index} — Context Compacted\n\n*[Context compacted]*\n\n---\n`;
}
