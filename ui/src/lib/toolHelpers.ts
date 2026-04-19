import type { ToolExecution, ToolOutput } from "./api";

// ---------------------------------------------------------------------------
// 内容清洗（移植自原版 contentSanitizer.ts）
// ---------------------------------------------------------------------------

/** 完全移除的噪声标签 */
const NOISE_TAG_PATTERNS = [
  /<local-command-caveat>[\s\S]*?<\/local-command-caveat>/gi,
  /<system-reminder>[\s\S]*?<\/system-reminder>/gi,
  /<task-notification>[\s\S]*?<\/task-notification>/gi,
];

/** task 通知尾部指令 */
const TASK_OUTPUT_INSTRUCTION_PATTERN =
  / ?Read the output file to retrieve the result: [^\s]+/g;

function isCommandContent(content: string): boolean {
  return content.startsWith("<command-name>") || content.startsWith("<command-message>");
}

function isCommandOutputContent(content: string): boolean {
  return content.startsWith("<local-command-stdout>") || content.startsWith("<local-command-stderr>");
}

/** 提取 <local-command-stdout/stderr> 内容 */
function extractCommandOutput(content: string): string | null {
  const match = /<local-command-stdout>([\s\S]*?)<\/local-command-stdout>/i.exec(content);
  if (match) return match[1].trim();
  const matchErr = /<local-command-stderr>([\s\S]*?)<\/local-command-stderr>/i.exec(content);
  if (matchErr) return matchErr[1].trim();
  return null;
}

/** 提取 slash 命令为可读格式，如 "/model sonnet" */
function extractCommandDisplay(content: string): string | null {
  const nameMatch = /<command-name>\/([^<]+)<\/command-name>/.exec(content);
  const argsMatch = /<command-args>([^<]*)<\/command-args>/.exec(content);
  if (nameMatch) {
    const name = `/${nameMatch[1].trim()}`;
    const args = argsMatch?.[1]?.trim();
    return args ? `${name} ${args}` : name;
  }
  return null;
}

export interface SlashInfo {
  name: string;
  message?: string;
  args?: string;
}

/**
 * 后台任务通知卡片数据。移植自原版 `contentSanitizer.ts::TaskNotification`。
 * 用户消息含 `<task-notification>` XML 时，文本被 `cleanDisplayText` 清空，
 * 但卡片信息仍要单独渲染（对齐原版 `UserChatGroup.tsx::taskNotifications`）。
 */
export interface TaskNotification {
  taskId: string;
  status: string;
  summary: string;
  outputFile: string;
}

/** 从 user message content 抽取所有 `<task-notification>` 卡片信息。 */
export function parseTaskNotifications(content: string | unknown[]): TaskNotification[] {
  let raw = "";
  if (typeof content === "string") {
    raw = content;
  } else if (Array.isArray(content)) {
    for (const b of content) {
      if (b && typeof b === "object" && "type" in b) {
        const x = b as Record<string, unknown>;
        if (x.type === "text" && typeof x.text === "string") raw += x.text;
      }
    }
  }
  const out: TaskNotification[] = [];
  const re = /<task-notification>([\s\S]*?)<\/task-notification>/gi;
  let m: RegExpExecArray | null;
  while ((m = re.exec(raw)) !== null) {
    const block = m[1];
    out.push({
      taskId: /<task-id>([^<]*)<\/task-id>/.exec(block)?.[1] ?? "",
      status: /<status>([^<]*)<\/status>/.exec(block)?.[1] ?? "",
      summary: /<summary>([\s\S]*?)<\/summary>/.exec(block)?.[1]?.trim() ?? "",
      outputFile: /<output-file>([^<]*)<\/output-file>/.exec(block)?.[1] ?? "",
    });
  }
  return out;
}

/** 从 command XML 标签提取 slash 信息 */
export function extractSlashInfo(content: string): SlashInfo | null {
  const nameMatch = /<command-name>\/([^<]+)<\/command-name>/.exec(content);
  if (!nameMatch) return null;
  const name = nameMatch[1].trim();
  const messageMatch = /<command-message>([^<]*)<\/command-message>/.exec(content);
  const argsMatch = /<command-args>([^<]*)<\/command-args>/.exec(content);
  return {
    name,
    message: messageMatch?.[1]?.trim() ?? undefined,
    args: argsMatch?.[1]?.trim() ?? undefined,
  };
}

/**
 * 清洗 JSONL 原始内容为可显示文本。
 * 逻辑与原版 `sanitizeDisplayContent` 对齐，额外处理 ANSI 转义码。
 */
export function cleanDisplayText(text: string): string {
  if (!text) return "";

  // 命令输出 → 直接返回内容
  if (isCommandOutputContent(text)) {
    const output = extractCommandOutput(text);
    if (output) return stripAnsi(output);
  }

  // slash 命令 → 返回 "/name args" 格式
  if (isCommandContent(text)) {
    const display = extractCommandDisplay(text);
    if (display) return display;
  }

  // 通用清洗
  let s = text;
  for (const p of NOISE_TAG_PATTERNS) {
    s = s.replace(p, "");
  }
  s = s
    .replace(/<command-name>[\s\S]*?<\/command-name>/gi, "")
    .replace(/<command-message>[\s\S]*?<\/command-message>/gi, "")
    .replace(/<command-args>[\s\S]*?<\/command-args>/gi, "")
    .replace(/<local-command-stdout>[\s\S]*?<\/local-command-stdout>/gi, "")
    .replace(/<local-command-stderr>[\s\S]*?<\/local-command-stderr>/gi, "");
  s = s.replace(TASK_OUTPUT_INSTRUCTION_PATTERN, "");

  return stripAnsi(s).trim();
}

/** 移除 ANSI 转义序列 */
function stripAnsi(s: string): string {
  // eslint-disable-next-line no-control-regex
  s = s.replace(/\x1b\[[0-9;]*m/g, "");
  s = s.replace(/\[(\d+;)*\d*m/g, "");
  return s;
}

/** 根据工具名和 input 生成摘要文本 */
export function getToolSummary(
  toolName: string,
  input: unknown
): string {
  const i = input as Record<string, unknown> | null;
  if (!i) return "";

  if (
    ["Read", "Edit", "Write", "read_file", "edit_file", "write_file"].includes(
      toolName
    )
  ) {
    return shortenPath(String(i.file_path ?? i.filePath ?? ""));
  }
  if (["Bash", "bash"].includes(toolName)) {
    const c = String(i.command ?? "");
    return c.length > 60 ? c.slice(0, 60) + "…" : c;
  }
  if (["Grep", "grep"].includes(toolName)) return String(i.pattern ?? "");
  if (["Glob", "glob"].includes(toolName)) return String(i.pattern ?? "");
  if (toolName === "Agent")
    return String(i.description ?? "").slice(0, 50);
  return "";
}

/** 判断工具状态 */
export function getToolStatus(
  exec: ToolExecution
): "ok" | "error" | "pending" | "orphaned" {
  if (exec.isError) return "error";
  if (exec.output.kind === "missing") return "pending";
  return "ok";
}

/** 将 ToolOutput 转为文本 */
export function toolOutputText(output: ToolOutput): string {
  if (output.kind === "text") return output.text;
  if (output.kind === "structured")
    return JSON.stringify(output.value, null, 2);
  return "";
}

/** 路径缩短：/Users/xxx → ~ */
export function shortenPath(p: string): string {
  return p.replace(/^\/Users\/[^/]+/, "~");
}

/** 截断文本 */
export function truncate(
  text: string,
  max: number
): { text: string; truncated: boolean } {
  if (text.length <= max) return { text, truncated: false };
  return { text: text.slice(0, max), truncated: true };
}

/** 文件扩展名 → 语言 */
const EXT_LANG: Record<string, string> = {
  ts: "typescript",
  tsx: "typescript",
  js: "javascript",
  jsx: "javascript",
  rs: "rust",
  py: "python",
  go: "go",
  rb: "ruby",
  java: "java",
  kt: "kotlin",
  c: "c",
  cpp: "cpp",
  h: "c",
  hpp: "cpp",
  cs: "csharp",
  swift: "swift",
  sh: "bash",
  bash: "bash",
  zsh: "bash",
  json: "json",
  yaml: "yaml",
  yml: "yaml",
  toml: "toml",
  xml: "xml",
  html: "html",
  css: "css",
  scss: "scss",
  sql: "sql",
  md: "markdown",
  svelte: "html",
  vue: "html",
};

export function getLanguageFromPath(filePath: string): string {
  const ext = filePath.split(".").pop()?.toLowerCase() ?? "";
  return EXT_LANG[ext] ?? "text";
}

/** 文件名提取 */
export function getFileName(filePath: string): string {
  return filePath.split("/").pop() ?? filePath;
}

