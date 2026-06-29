import type { SessionDetail, Chunk, AIChunk, UserChunk, SystemChunk, CompactChunk, ToolExecution, SubagentProcess, WorkflowItem, TeammateMessage, SlashCommand } from "../api";
import type { ExportOptions } from "./types";
import { projectSessionDetail } from "./projection";
import { buildDisplayItems, buildDisplayItemsFromChunks, type DisplayItem } from "../displayItemBuilder";
import { userChunkToMarkdown, toolExecToMarkdown } from "../contextMenu/markdown";
import { cleanDisplayText } from "../toolHelpers";

/** 单次导出贯穿的渲染上下文：workflow 关联 + runId 去重。 */
interface RenderCtx {
  options: ExportOptions;
  workflowMap: Map<string, WorkflowItem>;
  seenWorkflowIds: Set<string>;
}

export function exportAsMarkdown(detail: SessionDetail, options: ExportOptions): string {
  const projected = projectSessionDetail(detail, options);
  const ctx: RenderCtx = {
    options,
    workflowMap: new Map(projected.workflowItems.map((w) => [w.runId, w])),
    seenWorkflowIds: new Set(),
  };
  const parts: string[] = [];

  parts.push(buildHeader(projected.title, projected.sessionId));
  parts.push(buildMetadataTable(detail));
  parts.push("---\n");

  let turnIndex = 0;
  for (const chunk of projected.chunks) {
    turnIndex++;
    parts.push(renderChunk(chunk, turnIndex, ctx));
  }

  return parts.join("\n");
}

function buildHeader(title: string | null | undefined, sessionId: string): string {
  const displayTitle = title || `Session ${sessionId}`;
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

function renderChunk(chunk: Chunk, index: number, ctx: RenderCtx): string {
  switch (chunk.kind) {
    case "user":
      return renderUserChunk(chunk as UserChunk, index);
    case "ai":
      return renderAIChunk(chunk as AIChunk, index, ctx);
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

function renderAIChunk(chunk: AIChunk, index: number, ctx: RenderCtx): string {
  const parts: string[] = [];
  parts.push(`## Turn ${index} — Assistant\n`);

  const { items, lastOutput } = buildDisplayItems(chunk);

  for (const item of items) {
    const rendered = renderDisplayItem(item, ctx);
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

function renderDisplayItem(item: DisplayItem, ctx: RenderCtx): string {
  switch (item.type) {
    case "thinking":
      if (!ctx.options.includeThinking) return "";
      return `> [thinking] ${item.text}\n`;
    case "output":
      {
        const cleaned = cleanDisplayText(item.text);
        return cleaned ? `${cleaned}\n` : "";
      }
    case "tool":
      return renderToolOrWorkflow(item.execution, ctx);
    case "subagent":
      return renderSubagent(item.process, ctx);
    case "user_message":
      return `*[user]* ${cleanDisplayText(item.text)}\n`;
    case "slash":
      return renderSlash(item.slash);
    case "teammate_message":
      return renderTeammateMessage(item.teammateMessage);
    case "teammate_spawn":
      return `*[teammate spawned]* ${item.name}\n`;
    case "workflow":
      // buildDisplayItems 不产 workflow item（workflow 经 tool.workflowRunId 关联，
      // 见 renderToolOrWorkflow）。此 case 仅为类型完整保留。
      return "";
  }
}

function renderToolOrWorkflow(exec: ToolExecution, ctx: RenderCtx): string {
  // workflow 关联：带 workflowRunId 且命中 workflowItems 的 tool 渲染为 workflow 摘要
  // 替代普通 tool；同一 runId 单次导出只渲染一次，后续命中跳过（codex F4）。
  const runId = exec.workflowRunId;
  if (runId) {
    const wf = ctx.workflowMap.get(runId);
    if (wf) {
      if (ctx.seenWorkflowIds.has(runId)) return "";
      ctx.seenWorkflowIds.add(runId);
      return renderWorkflow(wf);
    }
  }
  const md = toolExecToMarkdown(exec);
  return `### Tool: ${exec.toolName}\n\n${md}\n`;
}

function renderWorkflow(wf: WorkflowItem): string {
  const name = wf.name ?? wf.runId;
  const phases = wf.phases?.length ?? 0;
  const agents = wf.agents ?? [];
  const parts: string[] = [];
  parts.push(`### Workflow: ${name} — ${wf.status}`);
  const meta: string[] = [`${phases} phase${phases === 1 ? "" : "s"}`, `${agents.length} agent${agents.length === 1 ? "" : "s"}`];
  if (wf.totalTokens) meta.push(`${wf.totalTokens.toLocaleString()} tokens`);
  if (wf.durationMs) meta.push(`${Math.round(wf.durationMs / 1000)}s`);
  parts.push(`\n*${meta.join(" · ")}*\n`);
  for (const a of agents) {
    const aMeta: string[] = [a.state];
    if (a.tokens) aMeta.push(`${a.tokens.toLocaleString()} tk`);
    if (a.durationMs) aMeta.push(`${Math.round(a.durationMs / 1000)}s`);
    parts.push(`- ${a.label} (${aMeta.join(", ")})`);
  }
  return parts.join("\n") + "\n";
}

function renderSlash(slash: SlashCommand): string {
  const parts: string[] = [`### Slash: /${slash.name}`];
  const arg = slash.args ?? slash.message;
  if (arg) parts.push(`\n\`${arg}\``);
  if (slash.instructions) parts.push(`\n> ${slash.instructions.replace(/\n/g, "\n> ")}`);
  return parts.join("") + "\n";
}

function renderTeammateMessage(tm: TeammateMessage): string {
  const body = cleanDisplayText(tm.body);
  return `### Teammate: ${tm.teammateId}\n\n${body}\n`;
}

function renderSubagent(sub: SubagentProcess, ctx: RenderCtx): string {
  const desc = sub.description || sub.rootTaskDescription || "subagent";
  const type = sub.subagentType ? ` (${sub.subagentType})` : "";
  const duration = sub.durationMs ? ` — ${Math.round(sub.durationMs / 1000)}s` : "";
  const parts: string[] = [`### Subagent: ${desc}${type}${duration}\n`];

  // 内部对话递归渲染：messages 已在 projection 阶段按导出选项 project（含 thinking
  // 过滤 / 工具详略 / includeSubagents 去重，见 projection.ts::projectSubagents）。
  if (sub.messages && sub.messages.length > 0) {
    const inner = buildDisplayItemsFromChunks(sub.messages);
    for (const item of inner) {
      const rendered = renderDisplayItem(item, ctx);
      if (rendered) parts.push(rendered);
    }
  } else if (sub.messagesOmitted) {
    parts.push("*[内部对话已省略：超出导出上限]*\n");
  }

  return parts.join("\n");
}

function renderSystemChunk(chunk: SystemChunk, index: number): string {
  const text = cleanDisplayText(chunk.contentText);
  return `## Turn ${index} — System\n\n*${text}*\n\n---\n`;
}

function renderCompactChunk(chunk: CompactChunk, index: number): string {
  return `## Turn ${index} — Context Compacted\n\n*[Context compacted]*\n\n---\n`;
}
