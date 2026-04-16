/**
 * Display Item Builder — 从 AIChunk 构建统一的 DisplayItem 列表。
 *
 * 对齐原版 TS 的 displayItemBuilder.ts + displaySummary.ts 管道：
 * AIChunk → buildDisplayItems(chunk) → DisplayItem[]
 *        → buildSummary(items) → header summary 字符串
 */

import type {
  AIChunk,
  ToolExecution,
  SubagentProcess,
  SlashCommand,
} from "./api";

// ---------------------------------------------------------------------------
// DisplayItem 类型
// ---------------------------------------------------------------------------

export interface ThinkingItem {
  type: "thinking";
  text: string;
  timestamp: string;
}

export interface ToolItem {
  type: "tool";
  execution: ToolExecution;
}

export interface OutputItem {
  type: "output";
  text: string;
  timestamp: string;
}

export interface SubagentItem {
  type: "subagent";
  process: SubagentProcess;
}

export interface SlashItem {
  type: "slash";
  slash: SlashCommand;
}

export type DisplayItem =
  | ThinkingItem
  | ToolItem
  | OutputItem
  | SubagentItem
  | SlashItem;

// ---------------------------------------------------------------------------
// buildDisplayItems
// ---------------------------------------------------------------------------

/**
 * 从 AIChunk 的 semanticSteps + toolExecutions + subagents + slashCommands
 * 构建统一 DisplayItem 列表。
 *
 * - slash 命令排最前
 * - 按 semanticSteps 出现顺序排列
 * - 最后一个 text step 被识别为 last output 并跳过（由外部始终可见地渲染）
 * - tool_execution step 无匹配 ToolExecution 则跳过
 * - subagent_spawn step 无匹配 Process 则跳过
 */
export function buildDisplayItems(chunk: AIChunk): {
  items: DisplayItem[];
  lastOutput: OutputItem | null;
} {
  // 找到最后一个 text step 的索引（last output）
  let lastTextIndex = -1;
  for (let i = chunk.semanticSteps.length - 1; i >= 0; i--) {
    if (chunk.semanticSteps[i].kind === "text") {
      lastTextIndex = i;
      break;
    }
  }

  // 构建 toolExecution 查找表
  const execMap = new Map<string, ToolExecution>();
  for (const exec of chunk.toolExecutions) {
    execMap.set(exec.toolUseId, exec);
  }

  // 构建 subagent 查找表（placeholderId = sessionId）
  const subMap = new Map<string, SubagentProcess>();
  for (const sub of chunk.subagents) {
    subMap.set(sub.sessionId, sub);
  }

  const items: DisplayItem[] = [];

  // slash 命令排最前
  for (const slash of chunk.slashCommands ?? []) {
    items.push({ type: "slash", slash });
  }

  // 按 semanticSteps 顺序构建
  let lastOutput: OutputItem | null = null;

  for (let i = 0; i < chunk.semanticSteps.length; i++) {
    const step = chunk.semanticSteps[i];

    switch (step.kind) {
      case "thinking":
        items.push({ type: "thinking", text: step.text, timestamp: step.timestamp });
        break;

      case "text":
        if (i === lastTextIndex) {
          // 记录 last output，不放入 items
          lastOutput = { type: "output", text: step.text, timestamp: step.timestamp };
        } else {
          items.push({ type: "output", text: step.text, timestamp: step.timestamp });
        }
        break;

      case "tool_execution": {
        const exec = execMap.get(step.toolUseId);
        if (exec) {
          items.push({ type: "tool", execution: exec });
        }
        break;
      }

      case "subagent_spawn": {
        const sub = subMap.get(step.placeholderId);
        if (sub) {
          items.push({ type: "subagent", process: sub });
        }
        break;
      }
    }
  }

  return { items, lastOutput };
}

// ---------------------------------------------------------------------------
// buildSummary
// ---------------------------------------------------------------------------

/**
 * 统计 DisplayItem 列表中各类型数量，生成 header summary 字符串。
 * 顺序：tool → slash → message → subagent → thinking
 */
export function buildSummary(items: DisplayItem[]): string {
  let tools = 0;
  let slashes = 0;
  let messages = 0;
  let subagents = 0;
  let thinkings = 0;

  for (const item of items) {
    switch (item.type) {
      case "tool":
        tools++;
        break;
      case "slash":
        slashes++;
        break;
      case "output":
        messages++;
        break;
      case "subagent":
        subagents++;
        break;
      case "thinking":
        thinkings++;
        break;
    }
  }

  const parts: string[] = [];
  if (tools > 0) parts.push(`${tools} tool call${tools > 1 ? "s" : ""}`);
  if (slashes > 0) parts.push(`${slashes} slash${slashes > 1 ? "es" : ""}`);
  if (messages > 0) parts.push(`${messages} message${messages > 1 ? "s" : ""}`);
  if (subagents > 0) parts.push(`${subagents} subagent${subagents > 1 ? "s" : ""}`);
  if (thinkings > 0) parts.push(`${thinkings} thinking`);
  return parts.join(", ");
}
