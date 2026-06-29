import type { SessionDetail, Chunk, AIChunk, UserChunk, SystemChunk, CompactChunk, ToolExecution, SubagentProcess, WorkflowItem, TeammateMessage, SlashCommand } from "../api";
import type { ExportOptions } from "./types";
import { projectSessionDetail } from "./projection";
import { buildDisplayItems, buildDisplayItemsFromChunks, type DisplayItem } from "../displayItemBuilder";
import { buildHtmlShell, escapeHtml } from "./htmlTemplate";
import { renderMarkdown } from "../render";
import { cleanDisplayText } from "../toolHelpers";

/** 单次导出贯穿的渲染上下文：workflow 关联 + runId 去重。 */
interface RenderCtx {
  options: ExportOptions;
  workflowMap: Map<string, WorkflowItem>;
  seenWorkflowIds: Set<string>;
}

export function exportAsHtml(detail: SessionDetail, options: ExportOptions): string {
  const projected = projectSessionDetail(detail, options);
  const title = projected.title || `Session ${projected.sessionId}`;
  const ctx: RenderCtx = {
    options,
    workflowMap: new Map(projected.workflowItems.map((w) => [w.runId, w])),
    seenWorkflowIds: new Set(),
  };

  const tocItems: string[] = [];
  const bodyParts: string[] = [];

  bodyParts.push(buildMetadataSection(detail));

  let turnIndex = 0;
  for (const chunk of projected.chunks) {
    turnIndex++;
    const { label, html } = renderChunkHtml(chunk, turnIndex, ctx);
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
  ctx: RenderCtx,
): { label: string; html: string } {
  switch (chunk.kind) {
    case "user":
      return renderUserHtml(chunk as UserChunk, index);
    case "ai":
      return renderAIHtml(chunk as AIChunk, index, ctx);
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
  ctx: RenderCtx,
): { label: string; html: string } {
  const parts: string[] = [];

  const { items, lastOutput } = buildDisplayItems(chunk);

  for (const item of items) {
    const rendered = renderDisplayItemHtml(item, ctx);
    if (rendered) parts.push(rendered);
  }

  for (const step of chunk.semanticSteps) {
    if (step.kind === "interruption") {
      parts.push(`<p><em>[interrupted]</em> ${escapeHtml(step.text)}</p>`);
    }
  }

  if (lastOutput) {
    const cleaned = cleanDisplayText(lastOutput.text);
    if (cleaned) parts.push(renderMarkdownSafe(cleaned));
  }

  const html = `<div class="turn" id="turn-${index}">
  <div class="turn-header">Turn ${index} — Assistant</div>
  <div class="turn-content">${parts.join("\n")}</div>
</div>`;
  return { label: `${index}. Assistant`, html };
}

function renderDisplayItemHtml(item: DisplayItem, ctx: RenderCtx): string {
  switch (item.type) {
    case "thinking":
      if (!ctx.options.includeThinking) return "";
      return `<div class="thinking">
  <div class="thinking-header">💭 Thinking...</div>
  <div class="thinking-content">${escapeHtml(item.text)}</div>
</div>`;
    case "output":
      {
        const cleaned = cleanDisplayText(item.text);
        if (!cleaned) return "";
        return renderMarkdownSafe(cleaned);
      }
    case "tool":
      return renderToolOrWorkflowHtml(item.execution, ctx);
    case "subagent":
      return renderSubagentHtml(item.process, ctx);
    case "user_message":
      return `<p><em>[user]</em> ${escapeHtml(item.text)}</p>`;
    case "slash":
      return renderSlashHtml(item.slash);
    case "teammate_message":
      return renderTeammateMessageHtml(item.teammateMessage);
    case "teammate_spawn":
      return `<p class="teammate-spawn"><em>[teammate spawned]</em> ${escapeHtml(item.name)}</p>`;
    case "workflow":
      // buildDisplayItems 不产 workflow item（workflow 经 tool.workflowRunId 关联）。
      return "";
  }
}

function renderToolOrWorkflowHtml(exec: ToolExecution, ctx: RenderCtx): string {
  const runId = exec.workflowRunId;
  if (runId) {
    const wf = ctx.workflowMap.get(runId);
    if (wf) {
      if (ctx.seenWorkflowIds.has(runId)) return "";
      ctx.seenWorkflowIds.add(runId);
      return renderWorkflowHtml(wf);
    }
  }
  return renderToolHtml(exec);
}

function renderWorkflowHtml(wf: WorkflowItem): string {
  const name = escapeHtml(wf.name ?? wf.runId);
  const phases = wf.phases?.length ?? 0;
  const agents = wf.agents ?? [];
  const meta: string[] = [`${phases} phase${phases === 1 ? "" : "s"}`, `${agents.length} agent${agents.length === 1 ? "" : "s"}`];
  if (wf.totalTokens) meta.push(`${wf.totalTokens.toLocaleString()} tokens`);
  if (wf.durationMs) meta.push(`${Math.round(wf.durationMs / 1000)}s`);
  const agentRows = agents
    .map((a) => {
      const aMeta: string[] = [escapeHtml(a.state)];
      if (a.tokens) aMeta.push(`${a.tokens.toLocaleString()} tk`);
      if (a.durationMs) aMeta.push(`${Math.round(a.durationMs / 1000)}s`);
      return `<li>${escapeHtml(a.label)} (${aMeta.join(", ")})</li>`;
    })
    .join("\n");
  return `<div class="workflow-block">
  <div class="workflow-header">⚙ ${name} — ${escapeHtml(wf.status)}</div>
  <div class="workflow-meta">${escapeHtml(meta.join(" · "))}</div>
  ${agentRows ? `<ul class="workflow-agents">${agentRows}</ul>` : ""}
</div>`;
}

function renderSlashHtml(slash: SlashCommand): string {
  const arg = slash.args ?? slash.message;
  const argHtml = arg ? `<code>${escapeHtml(arg)}</code>` : "";
  const instr = slash.instructions ? renderMarkdownSafe(slash.instructions) : "";
  return `<div class="slash-block">
  <div class="slash-header">/${escapeHtml(slash.name)} ${argHtml}</div>
  ${instr ? `<div class="slash-instructions">${instr}</div>` : ""}
</div>`;
}

function renderTeammateMessageHtml(tm: TeammateMessage): string {
  const body = renderMarkdownSafe(cleanDisplayText(tm.body));
  return `<div class="teammate-message">
  <div class="teammate-header">👥 ${escapeHtml(tm.teammateId)}</div>
  <div class="teammate-body">${body}</div>
</div>`;
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

function renderSubagentHtml(sub: SubagentProcess, ctx: RenderCtx): string {
  const desc = escapeHtml(sub.description || sub.rootTaskDescription || "subagent");
  const type = sub.subagentType ? ` (${escapeHtml(sub.subagentType)})` : "";
  const duration = sub.durationMs ? ` — ${Math.round(sub.durationMs / 1000)}s` : "";

  // 内部对话递归渲染：messages 已在 projection 阶段按导出选项 project。
  let inner = "";
  if (sub.messages && sub.messages.length > 0) {
    const items = buildDisplayItemsFromChunks(sub.messages);
    inner = items
      .map((item) => renderDisplayItemHtml(item, ctx))
      .filter((s) => s)
      .join("\n");
  } else if (sub.messagesOmitted) {
    inner = `<p class="subagent-omitted"><em>[内部对话已省略：超出导出上限]</em></p>`;
  }

  return `<div class="subagent">
  <div class="subagent-header">🤖 ${desc}${type}${duration}</div>
  ${inner ? `<div class="subagent-body">${inner}</div>` : ""}
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
