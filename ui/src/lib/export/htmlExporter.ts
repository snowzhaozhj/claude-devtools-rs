import type { SessionDetail, Chunk, AIChunk, UserChunk, SystemChunk, CompactChunk, SemanticStep, ToolExecution, SubagentProcess } from "../api";
import type { ExportOptions } from "./types";
import { projectSessionDetail } from "./projection";
import { buildHtmlShell, escapeHtml } from "./htmlTemplate";
import { renderMarkdown } from "../render";
import { cleanDisplayText } from "../toolHelpers";

export function exportAsHtml(detail: SessionDetail, options: ExportOptions): string {
  const projected = projectSessionDetail(detail, options);
  const title = projected.title || `Session ${projected.sessionId.slice(0, 8)}`;

  const tocItems: string[] = [];
  const bodyParts: string[] = [];

  bodyParts.push(buildMetadataSection(detail));

  let turnIndex = 0;
  for (const chunk of projected.chunks) {
    turnIndex++;
    const { label, html } = renderChunkHtml(chunk, turnIndex, options);
    tocItems.push(label);
    bodyParts.push(html);
  }

  return buildHtmlShell(title, bodyParts.join("\n"), tocItems);
}

function buildMetadataSection(detail: SessionDetail): string {
  const rows: [string, string][] = [];
  rows.push(["Session ID", detail.sessionId]);
  if (detail.metadata.cwd) rows.push(["工作目录", detail.metadata.cwd]);
  rows.push(["消息数", String(detail.metrics.message_count)]);
  rows.push(["状态", detail.isOngoing ? "进行中" : "已完成"]);
  if (detail.metadata.last_modified) {
    rows.push(["最后修改", new Date(detail.metadata.last_modified).toLocaleString()]);
  }

  const rowsHtml = rows
    .map(([k, v]) => `<tr><th>${escapeHtml(k)}</th><td>${escapeHtml(v)}</td></tr>`)
    .join("\n");

  return `<table class="metadata-table">\n${rowsHtml}\n</table>`;
}

function renderChunkHtml(
  chunk: Chunk,
  index: number,
  options: ExportOptions,
): { label: string; html: string } {
  switch (chunk.kind) {
    case "user":
      return renderUserHtml(chunk as UserChunk, index);
    case "ai":
      return renderAIHtml(chunk as AIChunk, index, options);
    case "system":
      return renderSystemHtml(chunk as SystemChunk, index);
    case "compact":
      return renderCompactHtml(index);
    default:
      return { label: `Turn ${index}`, html: "" };
  }
}

function renderUserHtml(chunk: UserChunk, index: number): { label: string; html: string } {
  const content = extractUserContent(chunk);
  const rendered = renderMarkdownSafe(content);
  const html = `<div class="turn" id="turn-${index}">
  <div class="turn-header">Turn ${index} — User</div>
  <div class="turn-content">${rendered}</div>
</div>`;
  return { label: `${index}. User`, html };
}

function renderAIHtml(
  chunk: AIChunk,
  index: number,
  options: ExportOptions,
): { label: string; html: string } {
  const parts: string[] = [];

  for (const step of chunk.semanticSteps) {
    const stepHtml = renderStepHtml(step, options);
    if (stepHtml) parts.push(stepHtml);
  }

  for (const exec of chunk.toolExecutions) {
    parts.push(renderToolHtml(exec));
  }

  if (options.includeSubagents) {
    for (const sub of chunk.subagents) {
      parts.push(renderSubagentHtml(sub));
    }
  }

  const html = `<div class="turn" id="turn-${index}">
  <div class="turn-header">Turn ${index} — Assistant</div>
  <div class="turn-content">${parts.join("\n")}</div>
</div>`;
  return { label: `${index}. Assistant`, html };
}

function renderStepHtml(step: SemanticStep, options: ExportOptions): string {
  if (step.kind === "thinking") {
    if (!options.includeThinking) return "";
    const escaped = escapeHtml(step.text);
    return `<div class="thinking">
  <div class="thinking-header">💭 Thinking...</div>
  <div class="thinking-content">${escaped}</div>
</div>`;
  }
  if (step.kind === "text") {
    const cleaned = cleanDisplayText(step.text);
    if (!cleaned) return "";
    return renderMarkdownSafe(cleaned);
  }
  if (step.kind === "interruption") {
    return `<p><em>[interrupted]</em> ${escapeHtml(step.text)}</p>`;
  }
  return "";
}

function renderToolHtml(exec: ToolExecution): string {
  const name = escapeHtml(exec.toolName);
  const inputStr = exec.input ? JSON.stringify(exec.input, null, 2) : "";
  const outputStr = extractToolOutput(exec);

  const contentParts: string[] = [];
  if (inputStr) {
    contentParts.push(`<pre><code>${escapeHtml(inputStr)}</code></pre>`);
  }
  if (outputStr) {
    contentParts.push(`<pre><code>${escapeHtml(outputStr)}</code></pre>`);
  }

  return `<div class="tool-block">
  <div class="tool-header">${name}</div>
  <div class="tool-content">${contentParts.join("\n")}</div>
</div>`;
}

function renderSubagentHtml(sub: SubagentProcess): string {
  const desc = escapeHtml(sub.description || sub.rootTaskDescription || "subagent");
  const type = sub.subagentType ? ` (${escapeHtml(sub.subagentType)})` : "";
  const duration = sub.durationMs ? ` — ${Math.round(sub.durationMs / 1000)}s` : "";
  return `<div class="subagent">
  <div class="subagent-header">🤖 ${desc}${type}${duration}</div>
</div>`;
}

function renderSystemHtml(chunk: SystemChunk, index: number): { label: string; html: string } {
  const text = escapeHtml(cleanDisplayText(chunk.contentText));
  const html = `<div class="turn" id="turn-${index}">
  <div class="turn-header">Turn ${index} — System</div>
  <div class="turn-content"><em>${text}</em></div>
</div>`;
  return { label: `${index}. System`, html };
}

function renderCompactHtml(index: number): { label: string; html: string } {
  const html = `<div class="turn" id="turn-${index}">
  <div class="turn-header">Turn ${index} — Context Compacted</div>
  <div class="turn-content"><em>[Context compacted]</em></div>
</div>`;
  return { label: `${index}. Compact`, html };
}

function extractUserContent(chunk: UserChunk): string {
  if (typeof chunk.content === "string") return cleanDisplayText(chunk.content);
  if (Array.isArray(chunk.content)) {
    return chunk.content
      .filter((b) => b && b.type === "text" && typeof b.text === "string")
      .map((b) => cleanDisplayText((b as { text: string }).text))
      .join("\n\n");
  }
  return "";
}

function extractToolOutput(exec: ToolExecution): string {
  const out = exec.output;
  if (!out) return "";
  if (out.kind === "text") return out.text ?? "";
  if (out.kind === "structured") {
    try {
      return JSON.stringify(out.value, null, 2);
    } catch {
      return String(out.value);
    }
  }
  return "";
}

function renderMarkdownSafe(md: string): string {
  try {
    return renderMarkdown(md);
  } catch {
    return `<p>${escapeHtml(md)}</p>`;
  }
}
