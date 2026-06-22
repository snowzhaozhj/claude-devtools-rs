import type { SessionDetail, Chunk, AIChunk, UserChunk, SystemChunk, CompactChunk, ToolExecution, SubagentProcess } from "../api";
import type { ExportOptions } from "./types";
import { projectSessionDetail } from "./projection";
import { buildDisplayItems, type DisplayItem } from "../displayItemBuilder";
import { userChunkToMarkdown, toolExecToMarkdown } from "../contextMenu/markdown";
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

  const { items, lastOutput } = buildDisplayItems(chunk);

  for (const item of items) {
    const rendered = renderDisplayItem(item, options);
    if (rendered) parts.push(rendered);
  }

  for (const step of chunk.semanticSteps) {
    if (step.kind === "interruption") {
      parts.push(`*[interrupted]* ${step.text}\n`);
    }
  }

  if (lastOutput) {
    const cleaned = cleanDisplayText(lastOutput.text);
    if (cleaned) parts.push(`${cleaned}\n`);
  }

  parts.push("---\n");
  return parts.join("\n");
}

function renderDisplayItem(item: DisplayItem, options: ExportOptions): string {
  switch (item.type) {
    case "thinking":
      if (!options.includeThinking) return "";
      return `> [thinking] ${item.text}\n`;
    case "output":
      {
        const cleaned = cleanDisplayText(item.text);
        return cleaned ? `${cleaned}\n` : "";
      }
    case "tool":
      return renderToolExecution(item.execution);
    case "subagent":
      return renderSubagent(item.process);
    case "user_message":
      return `*[user]* ${cleanDisplayText(item.text)}\n`;
    case "slash":
    case "teammate_message":
    case "teammate_spawn":
    case "workflow":
      return "";
  }
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
