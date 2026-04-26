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

/**
 * 估算文本 token 数。原版 `tokenFormatting.ts::estimateTokens` 同款启发式：
 * 按 ~4 字符 / token 算，足够用于 UI 展示。
 */
export function estimateTokens(text: string | null | undefined): number {
  if (!text) return 0;
  return Math.ceil(text.length / 4);
}

/** 估算任意 content（字符串 / 对象 / 数组）的 token 数，对象/数组先 JSON 序列化。 */
export function estimateContentTokens(content: unknown): number {
  if (content == null) return 0;
  if (typeof content === "string") return estimateTokens(content);
  return estimateTokens(JSON.stringify(content));
}

/**
 * 估算单个工具调用占用的上下文 token 总和：input（Claude 生成）+ output
 * （Claude 读回）。移植自原版 `getToolContextTokens`——把 Tool row 上原版
 * 显示的 "~N tokens" 槽位补回 Rust 版。
 */
export function getToolContextTokens(exec: ToolExecution): number {
  return getToolInputTokens(exec) + getToolOutputTokens(exec);
}

/** 单独估算 tool 的 input（Claude 生成的参数）token 数。 */
export function getToolInputTokens(exec: ToolExecution): number {
  return estimateContentTokens(exec.input);
}

/** 单独估算 tool 的 output（工具回写）token 数。
 *  优先级：outputBytes（OMIT 层填的原始字节长度，懒加载前后稳定）→ 直接读 output.text/value（HTTP / 老后端 / 回滚路径）→ 0。
 *  见 change `tool-output-omit-preserve-size`：BaseItem 头部 token 数因此在懒加载前后保持一致。 */
export function getToolOutputTokens(exec: ToolExecution): number {
  if (exec.outputBytes != null) return Math.ceil(exec.outputBytes / 4);
  if (exec.output && exec.output.kind === "text") return estimateTokens(exec.output.text);
  if (exec.output && exec.output.kind === "structured") return estimateContentTokens(exec.output.value);
  return 0;
}

/** 移除 ANSI 转义序列 */
function stripAnsi(s: string): string {
  // eslint-disable-next-line no-control-regex
  s = s.replace(/\x1b\[[0-9;]*m/g, "");
  s = s.replace(/\[(\d+;)*\d*m/g, "");
  return s;
}

/**
 * 根据工具名和 input 生成 BaseItem header 上的人类可读摘要文本。
 * 移植自原版 `src/renderer/utils/toolRendering/toolSummaryHelpers.ts::getToolSummary`，
 * 风格：filename + 关键参数（行数 / 行号范围 / 模式 / 主机名等），保持简短。
 */
export function getToolSummary(toolName: string, input: unknown): string {
  const i = input as Record<string, unknown> | null;
  if (!i) return "";

  switch (toolName) {
    case "Edit":
    case "edit_file": {
      const filePath = readString(i.file_path ?? i.filePath);
      if (!filePath) return "Edit";
      const fileName = getFileName(filePath);
      const oldString = readString(i.old_string ?? i.oldString);
      const newString = readString(i.new_string ?? i.newString);
      if (oldString && newString) {
        const oldLines = oldString.split("\n").length;
        const newLines = newString.split("\n").length;
        if (oldLines === newLines) {
          return `${fileName} - ${oldLines} ${pluralize("line", oldLines)}`;
        }
        return `${fileName} - ${oldLines} → ${newLines} lines`;
      }
      return fileName;
    }

    case "Read":
    case "read_file": {
      const filePath = readString(i.file_path ?? i.filePath);
      if (!filePath) return "Read";
      const fileName = getFileName(filePath);
      const limit = readNumber(i.limit);
      const offset = readNumber(i.offset);
      if (limit) {
        const start = offset ?? 1;
        return `${fileName} - lines ${start}-${start + limit - 1}`;
      }
      return fileName;
    }

    case "Write":
    case "write_file": {
      const filePath = readString(i.file_path ?? i.filePath);
      if (!filePath) return "Write";
      const fileName = getFileName(filePath);
      const content = readString(i.content);
      if (content) {
        const lineCount = content.split("\n").length;
        return `${fileName} - ${lineCount} ${pluralize("line", lineCount)}`;
      }
      return fileName;
    }

    case "Bash":
    case "bash": {
      const description = readString(i.description);
      if (description) return truncatePlain(description, 50);
      const command = readString(i.command);
      if (command) return truncatePlain(command, 50);
      return "Bash";
    }

    case "Grep":
    case "grep": {
      const pattern = readString(i.pattern);
      if (!pattern) return "Grep";
      const patternStr = `"${truncatePlain(pattern, 30)}"`;
      const glob = readString(i.glob);
      const path = readString(i.path);
      if (glob) return `${patternStr} in ${glob}`;
      if (path) return `${patternStr} in ${getFileName(path)}`;
      return patternStr;
    }

    case "Glob":
    case "glob": {
      const pattern = readString(i.pattern);
      if (!pattern) return "Glob";
      const patternStr = `"${truncatePlain(pattern, 30)}"`;
      const path = readString(i.path);
      if (path) return `${patternStr} in ${getFileName(path)}`;
      return patternStr;
    }

    case "Task":
    case "Agent": {
      const prompt = readString(i.prompt);
      const subagentType = readString(i.subagent_type ?? i.subagentType);
      const description = readString(i.description);
      const desc = description ?? prompt;
      const typeStr = subagentType ? `${subagentType} - ` : "";
      if (desc) return `${typeStr}${truncatePlain(desc, 40)}`;
      return subagentType ?? "Task";
    }

    case "WebFetch": {
      const url = readString(i.url);
      if (!url) return "WebFetch";
      try {
        const u = new URL(url);
        return truncatePlain(u.hostname + u.pathname, 50);
      } catch {
        return truncatePlain(url, 50);
      }
    }

    case "WebSearch": {
      const query = readString(i.query);
      return query ? `"${truncatePlain(query, 40)}"` : "WebSearch";
    }

    case "TodoWrite": {
      const todos = i.todos;
      if (Array.isArray(todos)) {
        return `${todos.length} ${pluralize("item", todos.length)}`;
      }
      return "TodoWrite";
    }

    case "NotebookEdit": {
      const notebookPath = readString(i.notebook_path ?? i.notebookPath);
      const editMode = readString(i.edit_mode ?? i.editMode);
      if (notebookPath) {
        const fileName = getFileName(notebookPath);
        return editMode ? `${editMode} - ${fileName}` : fileName;
      }
      return "NotebookEdit";
    }

    default: {
      // 未知工具：尝试常见字段
      const nameField =
        readString(i.name) ??
        readString(i.path) ??
        readString(i.file) ??
        readString(i.query) ??
        readString(i.command);
      if (nameField) return truncatePlain(nameField, 50);
      return "";
    }
  }
}

function readString(v: unknown): string | undefined {
  if (typeof v !== "string") return undefined;
  const s = v.trim();
  return s.length > 0 ? s : undefined;
}

function readNumber(v: unknown): number | undefined {
  return typeof v === "number" && Number.isFinite(v) ? v : undefined;
}

function pluralize(word: string, n: number): string {
  return n === 1 ? word : `${word}s`;
}

function truncatePlain(s: string, max: number): string {
  return s.length <= max ? s : `${s.slice(0, max)}…`;
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

