/**
 * Display Item Builder — 从 AIChunk 构建统一的 DisplayItem 列表。
 *
 * 对齐原版 TS 的 displayItemBuilder.ts + displaySummary.ts 管道：
 * AIChunk → buildDisplayItems(chunk) → DisplayItem[]
 *        → buildSummary(items) → header summary 字符串
 */

import type {
  AIChunk,
  Chunk,
  ToolExecution,
  SubagentProcess,
  SlashCommand,
  TeammateMessage,
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

export interface TeammateMessageDisplayItem {
  type: "teammate_message";
  teammateMessage: TeammateMessage;
}

export interface TeammateSpawnDisplayItem {
  type: "teammate_spawn";
  /** 队友名（如 "member-1"），用于 badge 文字。 */
  name: string;
  /** 队友色（teamColors 调色板键）。 */
  color: string | null;
  /** 来源 tool_use_id，用于稳定 key。 */
  toolUseId: string;
  /** 来源 tool 名（如 "Agent"），UI 不展示但保留供调试。 */
  toolName: string;
}

export type DisplayItem =
  | ThinkingItem
  | ToolItem
  | OutputItem
  | SubagentItem
  | SlashItem
  | TeammateMessageDisplayItem
  | TeammateSpawnDisplayItem;

// ---------------------------------------------------------------------------
// buildDisplayItems
// ---------------------------------------------------------------------------

/**
 * 从 AIChunk 的 semanticSteps + toolExecutions + subagents + slashCommands +
 * teammateMessages 构建统一 DisplayItem 列表。
 *
 * 排序策略（对齐原版 `displayItemBuilder.ts::sortDisplayItemsChronologically`）：
 * - slash 命令排最前（与 AI turn 整体绑定，不参与时序排序）
 * - 其余 items 全部按 timestamp 稳定排序穿插：thinking / text / tool /
 *   subagent / teammate_message / teammate_spawn 各自携带 timestamp，按 ISO
 *   字符串排序即可（同 ts 保留 push 顺序）
 * - 最后一个 text step 被识别为 last output 并跳过（由外部始终可见地渲染）
 *
 * teammate_spawn 替换：tool_execution 的 ToolExecution 检测到 `teammateSpawn`
 * 字段非空时，转化为 `teammate_spawn` DisplayItem（极简单行渲染替代 tool item），
 * 对齐原版 `LinkedToolItem.tsx::isTeammateSpawned`。
 *
 * teammate reply 的 `replyToToolUseId` 仅作为 chip 文本展示（哪条 SendMessage
 * 触发的回信），**不**决定渲染位置——位置由 timestamp 决定。这样即使没有
 * SendMessage 配对（teammate 主动发言），卡片也能按时序自然穿插，不会全部
 * 堆在 turn 末尾。
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

  // 收集已关联 subagent 的 Task `tool_use_id`，用于跳过重复的 Task 工具调用
  // 渲染（对齐原版 displayItemBuilder.ts 的 `taskIdsWithSubagents` 过滤）。
  const taskIdsWithSubagents = new Set<string>();
  for (const sub of chunk.subagents) {
    if (sub.parentTaskId) taskIdsWithSubagents.add(sub.parentTaskId);
  }

  // 累积"待排序池"：[(timestamp, item)]——最后整体稳定排序穿插。
  const pool: Array<{ ts: string; order: number; item: DisplayItem }> = [];
  let order = 0;
  let lastOutput: OutputItem | null = null;

  // semanticSteps 顺序构建（thinking / text / tool / subagent）
  for (let i = 0; i < chunk.semanticSteps.length; i++) {
    const step = chunk.semanticSteps[i];

    switch (step.kind) {
      case "thinking":
        pool.push({
          ts: step.timestamp,
          order: order++,
          item: { type: "thinking", text: step.text, timestamp: step.timestamp },
        });
        break;

      case "text":
        if (i === lastTextIndex) {
          // 记录 last output，不放入 items
          lastOutput = { type: "output", text: step.text, timestamp: step.timestamp };
        } else {
          pool.push({
            ts: step.timestamp,
            order: order++,
            item: { type: "output", text: step.text, timestamp: step.timestamp },
          });
        }
        break;

      case "tool_execution": {
        const exec = execMap.get(step.toolUseId);
        if (!exec) break;
        // 跳过已关联 subagent 的 Task tool_execution：subagent 卡片
        // 本身就是该 Task 的可视代表，再渲染一次 Tool item 会重复。
        if (exec.toolName === "Task" && taskIdsWithSubagents.has(exec.toolUseId)) {
          break;
        }
        // teammate_spawn 检测：tool_result.toolUseResult.status === "teammate_spawned"
        // 时由后端预算落到 exec.teammateSpawn，这里把整条 tool item 替换成
        // 极简 teammate_spawn DisplayItem（对齐原版 LinkedToolItem.tsx）。
        if (exec.teammateSpawn) {
          pool.push({
            ts: step.timestamp,
            order: order++,
            item: {
              type: "teammate_spawn",
              name: exec.teammateSpawn.name,
              color: exec.teammateSpawn.color ?? null,
              toolUseId: exec.toolUseId,
              toolName: exec.toolName,
            },
          });
        } else {
          pool.push({
            ts: step.timestamp,
            order: order++,
            item: { type: "tool", execution: exec },
          });
        }
        break;
      }

      case "subagent_spawn": {
        const sub = subMap.get(step.placeholderId);
        if (sub) {
          pool.push({
            ts: step.timestamp,
            order: order++,
            item: { type: "subagent", process: sub },
          });
        }
        break;
      }
    }
  }

  // teammate messages 按 timestamp 加入待排序池
  for (const tm of chunk.teammateMessages ?? []) {
    pool.push({
      ts: tm.timestamp,
      order: order++,
      item: { type: "teammate_message", teammateMessage: tm },
    });
  }

  // 稳定排序：先 timestamp 升序，同 ts 保留 push 顺序
  pool.sort((a, b) => {
    if (a.ts < b.ts) return -1;
    if (a.ts > b.ts) return 1;
    return a.order - b.order;
  });

  const items: DisplayItem[] = [];

  // slash 命令排最前（不参与时序排序）
  for (const slash of chunk.slashCommands ?? []) {
    items.push({ type: "slash", slash });
  }
  for (const entry of pool) {
    items.push(entry.item);
  }

  return { items, lastOutput };
}

// ---------------------------------------------------------------------------
// buildDisplayItemsCached — 按内容指纹 memo
//
// SessionDetail.svelte 在 `{#each detail.chunks}` 循环里对每个 AIChunk 调一次
// buildDisplayItems。Svelte 5 的 reactivity 会在 effect / derived 变化时重算
// 整个 each block；file-change 触发的 detail 替换更让所有 chunk 重算一遍——
// 即使其中绝大多数 chunk 内容完全没变（一次 JSONL 追加通常只动最后 1-2 个 chunk）。
//
// 用"内容指纹"作 key memo 结果：
//   - 相同 (firstResponseUuid, semanticSteps.len, toolExecutions.len, lastStepTs,
//     teammateMessages.len, slashCommands.len) 命中复用
//   - 大会话（数百 chunk）file-change 时只会有最后 1-2 个 chunk 重新 build
//
// 缓存上限 500 条，超出按 FIFO 淘汰最早写入项（Map 迭代顺序）。
// ---------------------------------------------------------------------------

const DISPLAY_ITEMS_CACHE_LIMIT = 500;
const displayItemsCache = new Map<
  string,
  { items: DisplayItem[]; lastOutput: OutputItem | null }
>();

function chunkDigest(chunk: AIChunk): string {
  const firstUuid = chunk.responses[0]?.uuid ?? chunk.timestamp;
  const stepsLen = chunk.semanticSteps.length;
  const lastStepTs = chunk.semanticSteps[stepsLen - 1]?.timestamp ?? "";
  const toolsLen = chunk.toolExecutions.length;
  const teamLen = chunk.teammateMessages?.length ?? 0;
  const slashLen = chunk.slashCommands?.length ?? 0;
  return `${firstUuid}|${stepsLen}|${lastStepTs}|${toolsLen}|${teamLen}|${slashLen}`;
}

export function buildDisplayItemsCached(chunk: AIChunk): {
  items: DisplayItem[];
  lastOutput: OutputItem | null;
} {
  const key = chunkDigest(chunk);
  const hit = displayItemsCache.get(key);
  if (hit) return hit;
  const result = buildDisplayItems(chunk);
  displayItemsCache.set(key, result);
  if (displayItemsCache.size > DISPLAY_ITEMS_CACHE_LIMIT) {
    const firstKey = displayItemsCache.keys().next().value;
    if (firstKey !== undefined) displayItemsCache.delete(firstKey);
  }
  return result;
}

/** 仅供测试：清理 memo 缓存。 */
export function _resetDisplayItemsCacheForTest(): void {
  displayItemsCache.clear();
}

// ---------------------------------------------------------------------------
// buildSummary
// ---------------------------------------------------------------------------

/**
 * 从 subagent 的 `messages: Chunk[]` 串联构建 DisplayItem 流。
 *
 * 对齐原版 `aiGroupEnhancer.ts::buildDisplayItemsFromMessages`：把 subagent session
 * 里所有 AI chunk 的 DisplayItem 平铺串接；user / system / compact chunk 忽略
 * （subagent 内部的 user 消息通常是 tool_result，已由 ExecutionTrace 的 tool item 覆盖）。
 */
export function buildDisplayItemsFromChunks(chunks: Chunk[]): DisplayItem[] {
  const out: DisplayItem[] = [];
  for (const c of chunks) {
    if (c.kind !== "ai") continue;
    const { items, lastOutput } = buildDisplayItems(c);
    out.push(...items);
    if (lastOutput) out.push(lastOutput);
  }
  return out;
}

/**
 * 统计 DisplayItem 列表中各类型数量，生成 header summary 字符串。
 *
 * 对齐原版 `claude-devtools/src/renderer/utils/displaySummary.ts`：
 * - team 成员（`process.team` 非空的 subagent）按 unique `memberName` 计入
 *   `teammates`，**不**计入 `subagents`（避免一个队友同时出现两个统计）
 * - `teammate_message` 单独计为 "N teammate messages"
 * - 拼接顺序：thinking → tool calls → messages → teammates → subagents → slashes → teammate messages
 */
export function buildSummary(items: DisplayItem[]): string {
  let tools = 0;
  let slashes = 0;
  let messages = 0;
  let subagents = 0;
  let thinkings = 0;
  let teammateMessages = 0;
  const teammateNames = new Set<string>();

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
        // team 成员单独计 teammate；非 team subagent 才走 subagent 计数
        if (item.process.team) {
          teammateNames.add(item.process.team.memberName);
        } else {
          subagents++;
        }
        break;
      case "thinking":
        thinkings++;
        break;
      case "teammate_message":
        teammateMessages++;
        break;
      case "teammate_spawn":
        // teammate spawn 卡片按 unique name 计入 teammates（与 SubagentItem
        // 含 team 字段的统计同语义）。
        teammateNames.add(item.name);
        break;
    }
  }

  const parts: string[] = [];
  if (thinkings > 0) parts.push(`${thinkings} thinking`);
  if (tools > 0) parts.push(`${tools} tool call${tools > 1 ? "s" : ""}`);
  if (messages > 0) parts.push(`${messages} message${messages > 1 ? "s" : ""}`);
  if (teammateNames.size > 0) {
    parts.push(`${teammateNames.size} teammate${teammateNames.size > 1 ? "s" : ""}`);
  }
  if (subagents > 0) parts.push(`${subagents} subagent${subagents > 1 ? "s" : ""}`);
  if (slashes > 0) parts.push(`${slashes} slash${slashes > 1 ? "es" : ""}`);
  if (teammateMessages > 0) {
    parts.push(`${teammateMessages} teammate message${teammateMessages > 1 ? "s" : ""}`);
  }
  return parts.join(", ");
}
