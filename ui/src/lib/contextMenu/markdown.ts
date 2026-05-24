/*
 * Chunk → Markdown 反序 helper（Task 5 / design.md::D8）。
 *
 * spec: openspec/specs/session-display/spec.md
 *   ::Requirement 消息 chunk 右键菜单（"复制为 Markdown" / "复制纯文本"）
 *
 * 设计原则（design.md::D8 + D8 取舍）：
 * - 前端直取 raw text 字段，**不**反向 HTML（不引入 turndown ~30 KB），**不**
 *   新增 IPC（chunk 数据已含全部所需字段）
 * - UserChunk.content 经 cleanDisplayText 清洗（去 XML noise 标签）即为原始 markdown
 * - AIChunk.semanticSteps 中 kind="text" 步骤的 text 字段就是原始 markdown
 *   （来自 assistant response，未经 marked 渲染）
 * - 工具块按 Bash / File 类别构造 markdown 段，含 fenced code block
 * - chunkToPlainText：在 markdown 之上 strip 格式，输出"用户最终阅读内容"
 */

import type { Chunk, UserChunk, AIChunk, SystemChunk, CompactChunk, ContentBlock, ToolExecution } from "../api";
import { cleanDisplayText } from "../toolHelpers";

// ---------------------------------------------------------------------------
// UserChunk → Markdown
// ---------------------------------------------------------------------------

/**
 * 从 UserChunk 提取用户输入 markdown。
 *
 * UserChunk.content 形态：
 * - `string`：纯文本（典型路径，~99% 真实数据）
 * - `ContentBlock[]`：含 image / text 混排（罕见，typical "image + 文字"）
 *
 * 处理：
 * - string → cleanDisplayText 清洗 XML noise
 * - ContentBlock[] → 仅取 type==="text" 块拼接（image block 跳过——markdown
 *   无法表达 base64 image data，且 image block 已在 IPC payload 瘦身路径中
 *   置 dataOmitted=true）
 */
export function userChunkToMarkdown(chunk: UserChunk): string {
  return contentToMarkdown(chunk.content);
}

// ---------------------------------------------------------------------------
// AIChunk → Markdown
// ---------------------------------------------------------------------------

/**
 * 从 AIChunk 提取 AI 回复 markdown。
 *
 * 按 design.md::D8 取 `chunk.semanticSteps` 中 kind="text" 步骤的 `text` 字段
 * 拼接（用 `\n\n` 分隔）。这是 AI 原始回复的最干净来源——`responses` 内
 * `content` 字段是 ContentBlock[] 形态，提取 text 还要绕一层。
 *
 * 不包含 thinking / tool_execution / subagent_spawn / interruption 步骤——
 * "复制为 Markdown" 的语义是用户预期的"AI 答复正文"。
 */
export function aiChunkToMarkdown(chunk: AIChunk): string {
  const textParts: string[] = [];
  for (const step of chunk.semanticSteps) {
    if (step.kind === "text") {
      const cleaned = cleanDisplayText(step.text);
      if (cleaned) textParts.push(cleaned);
    }
  }
  return textParts.join("\n\n");
}

// ---------------------------------------------------------------------------
// ToolExecution → Markdown
// ---------------------------------------------------------------------------

/**
 * 从 ToolExecution 构造工具调用 markdown 段。
 *
 * 按工具名分发：
 * - Bash → `$ {command}` 行 + output fenced block
 * - Read → 文件路径 heading + content fenced block（含 lang hint）
 * - Edit / Write → 文件路径 heading + diff fenced block（已含 +/- 前缀走 diff lang）
 * - 其它 / Default → input JSON + output fenced block
 *
 * output 缺失（`output.kind === "missing"`）时仅含 input 部分，避免空 fence
 * 干扰阅读。
 */
export function toolExecToMarkdown(exec: ToolExecution): string {
  const name = exec.toolName;
  const input = (exec.input ?? {}) as Record<string, unknown>;

  if (name === "Bash") {
    const command = String(input.command ?? "");
    const output = extractOutputText(exec);
    const outFence = output ? `\n\n\`\`\`\n${output}\n\`\`\`` : "";
    return `\`\`\`bash\n$ ${command}\n\`\`\`${outFence}`;
  }

  if (name === "Read") {
    const path = String(input.file_path ?? input.filePath ?? "");
    const output = extractOutputText(exec);
    const lang = guessLangFromPath(path);
    const head = path ? `**${path}**\n\n` : "";
    if (!output) return head.trimEnd();
    return `${head}\`\`\`${lang}\n${output}\n\`\`\``;
  }

  if (name === "Edit" || name === "Write") {
    const path = String(input.file_path ?? input.filePath ?? "");
    const head = path ? `**${path}**\n\n` : "";
    if (name === "Write") {
      // Write：input.content 是新文件全文
      const content = String(input.content ?? "");
      const lang = guessLangFromPath(path);
      if (!content) return head.trimEnd();
      return `${head}\`\`\`${lang}\n${content}\n\`\`\``;
    }
    // Edit：output 通常含 diff（After + Before）；优先用 output 文本作 diff fence
    const output = extractOutputText(exec);
    if (!output) {
      // fallback：input.old_string / new_string 拼成简单 diff
      const oldStr = String(input.old_string ?? "");
      const newStr = String(input.new_string ?? "");
      if (!oldStr && !newStr) return head.trimEnd();
      return `${head}\`\`\`diff\n- ${oldStr.replace(/\n/g, "\n- ")}\n+ ${newStr.replace(/\n/g, "\n+ ")}\n\`\`\``;
    }
    return `${head}\`\`\`diff\n${output}\n\`\`\``;
  }

  // 其它工具：input JSON + output（如有）
  const inputJson = safeJsonStringify(input);
  const output = extractOutputText(exec);
  const inputFence = `\`\`\`json\n${inputJson}\n\`\`\``;
  if (!output) return `**${name}**\n\n${inputFence}`;
  return `**${name}**\n\n${inputFence}\n\n\`\`\`\n${output}\n\`\`\``;
}

// ---------------------------------------------------------------------------
// chunk → 纯文本（strip markdown）
// ---------------------------------------------------------------------------

/**
 * 提取 chunk 纯文本（strip markdown 格式：# / ** / * / ` / fences / link）。
 *
 * 用最小 regex 手写，不引入 `remove-markdown` 库（30+ KB ROI 不合理）：
 * - 复杂 nested bold/italic / footnote / table 等不完美——可接受，用户可
 *   选"复制为 Markdown"获取精确格式
 * - 优先剔除常用噪声：fence、link 取 text、heading hash、加粗星号、行内 code
 */
export function chunkToPlainText(chunk: Chunk): string {
  const md = chunkToMarkdownDispatch(chunk);
  return stripMarkdownFormatting(md);
}

// ---------------------------------------------------------------------------
// 内部 helpers
// ---------------------------------------------------------------------------

function chunkToMarkdownDispatch(chunk: Chunk): string {
  switch (chunk.kind) {
    case "user":
      return userChunkToMarkdown(chunk as UserChunk);
    case "ai":
      return aiChunkToMarkdown(chunk as AIChunk);
    case "system":
      return cleanDisplayText((chunk as SystemChunk).contentText);
    case "compact":
      return cleanDisplayText((chunk as CompactChunk).summaryText);
    default:
      return "";
  }
}

function contentToMarkdown(content: string | ContentBlock[]): string {
  if (typeof content === "string") {
    return cleanDisplayText(content);
  }
  if (!Array.isArray(content)) return "";
  const parts: string[] = [];
  for (const block of content) {
    if (block && block.type === "text" && typeof block.text === "string") {
      const cleaned = cleanDisplayText(block.text);
      if (cleaned) parts.push(cleaned);
    }
    // image / tool_use / tool_result 等其它块跳过——markdown 无法表达 base64
    // image，且 user chunk 极少携带这类 block
  }
  return parts.join("\n\n");
}

function extractOutputText(exec: ToolExecution): string {
  const out = exec.output;
  if (!out) return "";
  if (out.kind === "text") return out.text ?? "";
  if (out.kind === "structured") {
    return safeJsonStringify(out.value);
  }
  // missing：output 已被 OMIT 裁剪或本就缺失，返回空
  return "";
}

function safeJsonStringify(v: unknown): string {
  try {
    return JSON.stringify(v, null, 2);
  } catch {
    return String(v);
  }
}

function guessLangFromPath(path: string): string {
  const m = /\.([a-zA-Z0-9]+)$/.exec(path);
  if (!m) return "";
  const ext = m[1].toLowerCase();
  // 仅常用 alias 化（其余直接返回 ext，highlight.js 兜底处理）
  const alias: Record<string, string> = {
    ts: "typescript",
    tsx: "tsx",
    js: "javascript",
    jsx: "jsx",
    rs: "rust",
    py: "python",
    rb: "ruby",
    sh: "bash",
    md: "markdown",
    yml: "yaml",
  };
  return alias[ext] ?? ext;
}

/**
 * 最小 markdown strip：
 * - fenced code block ``` ... ``` → 内部代码原样保留（去 fence 行）
 * - inline code `x` → x
 * - heading `# x` → `x`
 * - bold/italic `**x**` `*x*` `_x_` → `x`
 * - link `[text](url)` → `text`
 * - image `![alt](url)` → `alt`
 * - blockquote `> x` → `x`
 * - list marker `- ` / `* ` / `1. ` → 去掉
 */
function stripMarkdownFormatting(md: string): string {
  let s = md;
  // 1. fenced block：去掉首尾 fence 行（``` 或 ```lang），保留内部内容
  s = s.replace(/```[a-zA-Z0-9]*\n([\s\S]*?)\n```/g, "$1");
  s = s.replace(/```([\s\S]*?)```/g, "$1"); // 兜底无换行的小 fence
  // 2. image: ![alt](url) → alt
  s = s.replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1");
  // 3. link: [text](url) → text
  s = s.replace(/\[([^\]]+)\]\([^)]*\)/g, "$1");
  // 4. heading hash
  s = s.replace(/^#{1,6}[ \t]+/gm, "");
  // 5. blockquote
  s = s.replace(/^>[ \t]+/gm, "");
  // 6. list marker（用 [ \t] 而非 \s——\s 在 /m 模式下会贪婪吃前导换行
  // 把 list 之间的空行连带剥掉，破坏段落分隔）
  s = s.replace(/^[ \t]*[-*+][ \t]+/gm, "");
  s = s.replace(/^[ \t]*\d+\.[ \t]+/gm, "");
  // 7. bold / italic / strikethrough（顺序：double 在 single 之前）
  s = s.replace(/\*\*([^*]+)\*\*/g, "$1");
  s = s.replace(/__([^_]+)__/g, "$1");
  s = s.replace(/\*([^*\n]+)\*/g, "$1");
  s = s.replace(/(?<!\w)_([^_\n]+)_(?!\w)/g, "$1");
  s = s.replace(/~~([^~]+)~~/g, "$1");
  // 8. inline code（last——避免 strip 上面修饰符时误吃 backtick）
  s = s.replace(/`([^`]+)`/g, "$1");
  return s;
}
