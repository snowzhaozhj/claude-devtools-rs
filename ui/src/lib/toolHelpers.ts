import type { ToolExecution, ToolOutput } from "./api";

/**
 * 清洗文本：移除 JSONL 中的元数据标签。
 * - `<command-name>...</command-name>`, `<command-message>...</command-message>`
 * - `<command-args>...</command-args>`
 * - `<system-reminder>...</system-reminder>` (含多行内容)
 * - `<local-command-caveat>...</local-command-caveat>`
 * - `<local-command-stdout>...</local-command-stdout>`
 * - 其他类似的 XML 包装标签
 */
export function cleanDisplayText(text: string): string {
  if (!text) return "";
  let s = text;

  // 1. 移除多行块标签（连同内容一起删除）
  s = s.replace(/<system-reminder>[\s\S]*?<\/system-reminder>/g, "");
  s = s.replace(/<local-command-caveat>[\s\S]*?<\/local-command-caveat>/g, "");

  // 2. 提取 command-message 内容（如果有），这是用户实际输入
  const cmdMsgMatch = s.match(/<command-message>([\s\S]*?)<\/command-message>/);

  // 3. 移除 local-command-stdout 连同内容（这是命令输出，不是用户输入）
  s = s.replace(/<local-command-stdout>[\s\S]*?<\/local-command-stdout>/g, "");

  // 4. 移除其他自定义 XML 标签（只去标签，保留内容）
  s = s.replace(/<\/?(?:command-name|command-message|command-args)[^>]*>/g, "");

  // 5. 移除 ANSI 转义序列（真实 ESC 字符 \x1b）
  // eslint-disable-next-line no-control-regex
  s = s.replace(/\x1b\[[0-9;]*m/g, "");

  // 6. 移除残留的 ANSI 码文本形式（[1m, [22m, [0m 等）
  s = s.replace(/\[(\d+;)*\d*m/g, "");

  // 7. 如果清洗后为空但有 command-message，用它
  s = s.trim();
  if (!s && cmdMsgMatch) {
    s = cmdMsgMatch[1].trim();
  }

  // 8. 去除连续的重复行（如 "/model\nmodel" → "/model"）
  const lines = s.split("\n").map(l => l.trim()).filter(Boolean);
  if (lines.length === 2 && lines[0].startsWith("/") && lines[0].slice(1) === lines[1]) {
    s = lines[0];
  }

  return s.trim();
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
