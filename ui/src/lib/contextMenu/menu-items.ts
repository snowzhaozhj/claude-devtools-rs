/*
 * menu-items 函数库（Task 4 / design.md::D6）。
 *
 * spec: openspec/specs/frontend-context-menu/spec.md
 *   ::Requirement menu-items 函数库
 *
 * 按 surface 拆 factory 函数，每个返回 `ContextMenuItem[]` 给 use:contextMenu
 * 的 provider 消费。所有 factory 共享 `MenuItemContext` 上下文（含 sessionId
 * / projectId / settings / dispatch / selectionText），让 item.action 自包含
 * 不需要在 surface 组件里写 closure 引用 IPC。
 *
 * 硬约束：
 * - factory **不**直接读 DOM（含 window.getSelection / document.activeElement）
 * - factory **不**直接 import api.ts（IPC 调用走 ctx.dispatch 间接调用便于测试）
 * - factory 是**纯函数**：相同 (input, ctx) → 相同输出
 * - 调用方在 oncontextmenu 触发瞬间预读 selectionText 后通过 ctx 传入
 *   （详 spec menu-items 函数库 Requirement 第一段）
 *
 * separator 自动插入（design.md::D-V3 + D6）：
 * - 相邻 item kind 切换处插 `{ separator: true }`
 * - factory 内部 trim 首尾孤立 separator
 * - 两段顺序：① copy → ② external
 */

import type { ContextMenuItem } from "./types";
import type { UserChunk, AIChunk, ToolExecution } from "../api";
import { userChunkToMarkdown, aiChunkToMarkdown, toolExecToMarkdown, chunkToPlainText } from "./markdown";
import { truncatePath } from "./pathLabel";

// ---------------------------------------------------------------------------
// 共享类型
// ---------------------------------------------------------------------------

/** Settings.searchEngine 形态（与后端 D3 internally tagged enum 对齐 IPC 序列化值） */
export type SearchEngineSetting =
  | { type: "google" }
  | { type: "bing" }
  | { type: "duck_duck_go" }
  | { type: "custom"; urlTemplate: string };

/** Settings.externalEditor 形态（与后端 D3 enum snake_case 对齐） */
export type ExternalEditorSetting =
  | "system"
  | "vs_code"
  | "cursor"
  | "zed"
  | "sublime";

/** Settings.terminalApp 形态（统一并集 enum；后端 D3 跨平台不匹配 fallback） */
export type TerminalAppSetting = string;

/**
 * factory 共享上下文。所有运行时浏览器状态（典型 selection）SHALL 通过本接口
 * 字段传入，**不**让 factory 内部读 DOM。
 */
export interface MenuItemContext {
  /** 当前 session id */
  sessionId: string;
  /** 当前 project id */
  projectId: string;
  /** Settings 三字段快照（typically 来自 getConfig 的 general 段） */
  settings: {
    externalEditor: ExternalEditorSetting;
    searchEngine: SearchEngineSetting;
    terminalApp: TerminalAppSetting;
  };
  /** 当前选区文本快照——调用方在 oncontextmenu 触发瞬间预读 */
  selectionText: string;
  /** IPC dispatch 闭包，封装真 IPC 调用便于 vitest mock */
  dispatch: MenuItemDispatch;
}

/** dispatch 闭包接口——封装 5 个 IPC 调用 */
export interface MenuItemDispatch {
  /** clipboard 写入纯文本 */
  copyToClipboard: (text: string) => Promise<void>;
  /** 调 open_in_editor IPC 打开文件（含可选行号 / 列号） */
  openInEditor: (path: string, line?: number, column?: number) => Promise<void>;
  /** 调 open_in_terminal IPC cd 到目录 */
  openInTerminal: (path: string) => Promise<void>;
  /** 调 plugin:opener|reveal_item_in_dir 在 Finder/Explorer 显示 */
  revealInDir: (path: string) => Promise<void>;
  /** 调 plugin:opener|open_url 浏览器打开 URL */
  openUrl: (url: string) => Promise<void>;
}

// ---------------------------------------------------------------------------
// factory：UserMessage chunk
// ---------------------------------------------------------------------------

export function buildUserMessageItems(chunk: UserChunk, ctx: MenuItemContext): ContextMenuItem[] {
  const items: RawItem[] = [];
  appendSelectionCopyIfAny(items, ctx);

  items.push(copyItem("复制纯文本", () => chunkToPlainText(chunk), ctx, "⌘C"));
  items.push(copyItem("复制为 Markdown", () => userChunkToMarkdown(chunk), ctx));

  return finalizeWithSeparators(items);
}

// ---------------------------------------------------------------------------
// factory：AssistantMessage chunk
// ---------------------------------------------------------------------------

export function buildAssistantMessageItems(chunk: AIChunk, ctx: MenuItemContext): ContextMenuItem[] {
  const items: RawItem[] = [];
  appendSelectionCopyIfAny(items, ctx);

  items.push(copyItem("复制纯文本", () => chunkToPlainText(chunk), ctx, "⌘C"));
  items.push(copyItem("复制为 Markdown", () => aiChunkToMarkdown(chunk), ctx));

  return finalizeWithSeparators(items);
}

// ---------------------------------------------------------------------------
// factory：BashTool 块
// ---------------------------------------------------------------------------

export function buildBashToolItems(exec: ToolExecution, ctx: MenuItemContext): ContextMenuItem[] {
  const items: RawItem[] = [];
  appendSelectionCopyIfAny(items, ctx);

  const input = (exec.input ?? {}) as Record<string, unknown>;
  const command = String(input.command ?? "");
  const cwd = String(input.cwd ?? "");
  const outputText = extractOutputForBash(exec);

  // 复制类
  items.push(copyItem(
    "复制命令",
    () => command,
    ctx,
    undefined,
    "copy",
    !command,
  ));
  items.push(copyItem(
    "复制完整 Markdown",
    () => toolExecToMarkdown(exec),
    ctx,
  ));
  if (outputText) {
    items.push(copyItem(
      exec.isError ? "复制 stderr" : "复制 stdout",
      () => outputText,
      ctx,
    ));
  }

  // 外部类：在终端打开 cwd
  if (cwd) {
    items.push(externalItem(
      "在终端打开",
      () => ctx.dispatch.openInTerminal(cwd),
    ));
  }

  // 错误时提供"在浏览器搜索错误信息"
  if (exec.isError && outputText) {
    items.push(externalItem(
      "在浏览器搜索错误",
      () => ctx.dispatch.openUrl(buildSearchUrl(firstLine(outputText), ctx.settings.searchEngine)),
    ));
  }

  return finalizeWithSeparators(items);
}

// ---------------------------------------------------------------------------
// factory：File 工具块（Read / Edit / Write 共用）
// ---------------------------------------------------------------------------

export function buildFileToolItems(exec: ToolExecution, ctx: MenuItemContext): ContextMenuItem[] {
  const items: RawItem[] = [];
  appendSelectionCopyIfAny(items, ctx);

  const input = (exec.input ?? {}) as Record<string, unknown>;
  const path = String(input.file_path ?? input.filePath ?? "");

  // 复制类
  items.push(copyItem(
    "复制路径",
    () => path,
    ctx,
    undefined,
    "copy",
    !path,
  ));
  items.push(copyItem(
    "复制完整 Markdown",
    () => toolExecToMarkdown(exec),
    ctx,
  ));
  if (exec.toolName === "Edit" || exec.toolName === "Write") {
    items.push(copyItem(
      "复制 Diff",
      () => extractDiffText(exec),
      ctx,
    ));
  }

  // 外部类
  if (path) {
    items.push(externalItem(
      "在编辑器打开",
      () => ctx.dispatch.openInEditor(path),
      makePathLabel("在编辑器打开", path),
    ));
    items.push(externalItem(
      "在 Finder 中显示",
      () => ctx.dispatch.revealInDir(path),
    ));
    items.push(externalItem(
      "在终端打开父目录",
      () => ctx.dispatch.openInTerminal(parentDir(path)),
    ));
  }

  return finalizeWithSeparators(items);
}

// ---------------------------------------------------------------------------
// factory：Worktree chip
// ---------------------------------------------------------------------------

export function buildWorktreeChipItems(
  worktree: { path: string; name: string },
  ctx: MenuItemContext,
): ContextMenuItem[] {
  const items: RawItem[] = [];
  appendSelectionCopyIfAny(items, ctx);

  items.push(copyItem("复制路径", () => worktree.path, ctx, undefined, "copy", !worktree.path));

  if (worktree.path) {
    items.push(externalItem(
      "在编辑器打开",
      () => ctx.dispatch.openInEditor(worktree.path),
      makePathLabel("在编辑器打开", worktree.path),
    ));
    items.push(externalItem(
      "在终端打开",
      () => ctx.dispatch.openInTerminal(worktree.path),
    ));
    items.push(externalItem(
      "在 Finder 中显示",
      () => ctx.dispatch.revealInDir(worktree.path),
    ));
  }

  return finalizeWithSeparators(items);
}

// ---------------------------------------------------------------------------
// factory：Project card
// ---------------------------------------------------------------------------

export function buildProjectCardItems(
  project: { path: string; name: string },
  ctx: MenuItemContext,
): ContextMenuItem[] {
  const items: RawItem[] = [];
  appendSelectionCopyIfAny(items, ctx);

  items.push(copyItem("复制路径", () => project.path, ctx, undefined, "copy", !project.path));
  items.push(copyItem("复制项目名", () => project.name, ctx, undefined, "copy", !project.name));

  if (project.path) {
    items.push(externalItem(
      "在编辑器打开",
      () => ctx.dispatch.openInEditor(project.path),
      makePathLabel("在编辑器打开", project.path),
    ));
    items.push(externalItem(
      "在终端打开",
      () => ctx.dispatch.openInTerminal(project.path),
    ));
  }

  return finalizeWithSeparators(items);
}

// ---------------------------------------------------------------------------
// factory：选区菜单（Layer 2 window-level handler 调用）
// ---------------------------------------------------------------------------

export function buildSelectionItems(selectionText: string, ctx: MenuItemContext): ContextMenuItem[] {
  const text = selectionText ?? "";
  if (!text) return [];

  const items: RawItem[] = [];
  items.push(copyItem("复制", () => text, ctx, "⌘C"));
  items.push(copyItem(
    "复制为引用 Markdown",
    () => quoteAsMarkdown(text),
    ctx,
  ));
  items.push(externalItem(
    "在浏览器搜索",
    () => ctx.dispatch.openUrl(buildSearchUrl(text, ctx.settings.searchEngine)),
  ));

  return finalizeWithSeparators(items);
}

// ---------------------------------------------------------------------------
// 内部：原始 item 构造 helpers + finalizeWithSeparators
// ---------------------------------------------------------------------------

/**
 * 内部 raw item 类型——含 `kind` 必填字段，便于 finalizeWithSeparators 按
 * kind 切换插 separator。导出后的 ContextMenuItem.kind 字段 optional。
 */
type RawItem = ContextMenuItem & { kind: NonNullable<ContextMenuItem["kind"]> };

function copyItem(
  label: string,
  textFn: () => string,
  ctx: MenuItemContext,
  shortcut?: string,
  kind: "copy" = "copy",
  disabled = false,
): RawItem {
  return {
    label,
    kind,
    shortcut,
    disabled,
    feedback: { label: "已复制!" },
    action: () => {
      const text = textFn();
      if (!text) return;
      void ctx.dispatch.copyToClipboard(text);
    },
  };
}

function externalItem(
  label: string,
  fn: () => Promise<unknown> | void,
  pathLabel?: { short: string; full: string },
): RawItem {
  const item: RawItem = {
    label,
    kind: "external",
    action: () => {
      void fn();
    },
  };
  if (pathLabel) item.pathLabel = pathLabel;
  return item;
}

/**
 * separator 自动插入：相邻 item kind 切换处插 `{ separator: true }`，
 * trim 首尾孤立 separator。
 *
 * 输入是 `RawItem[]`（kind 必填），输出是 `ContextMenuItem[]`（含 separator 项）。
 */
function finalizeWithSeparators(rawItems: RawItem[]): ContextMenuItem[] {
  if (rawItems.length === 0) return [];

  // 按 kind 分组，保留输入顺序
  const out: ContextMenuItem[] = [];
  let lastKind: RawItem["kind"] | null = null;
  for (const item of rawItems) {
    if (lastKind !== null && item.kind !== lastKind) {
      out.push({ separator: true });
    }
    out.push(item);
    lastKind = item.kind;
  }

  // trim 首尾孤立 separator（防御）
  while (out.length > 0 && out[0].separator) out.shift();
  while (out.length > 0 && out[out.length - 1].separator) out.pop();

  return out;
}

/** 有选区时在首段首项前插入"复制选中文本"item（spec menu-items 函数库 Scenario "有选区时融合"复制选中文本""） */
function appendSelectionCopyIfAny(items: RawItem[], ctx: MenuItemContext): void {
  if (!ctx.selectionText) return;
  items.push({
    label: "复制选中文本",
    kind: "copy",
    shortcut: "⌘C",
    feedback: { label: "已复制!" },
    action: () => {
      void ctx.dispatch.copyToClipboard(ctx.selectionText);
    },
  });
}

/** 把"在编辑器打开"等 label 与 path 拼装成 pathLabel（中段截断 short / 完整 full） */
function makePathLabel(prefix: string, path: string): { short: string; full: string } {
  const truncated = truncatePath(path);
  return {
    short: `${prefix} ${truncated.short}`,
    full: `${prefix} ${truncated.full}`,
  };
}

// ---------------------------------------------------------------------------
// 内部：URL / path / 文本辅助
// ---------------------------------------------------------------------------

/** 用 settings.searchEngine 拼搜索 URL（Custom 必含 {query}） */
function buildSearchUrl(query: string, engine: SearchEngineSetting): string {
  const q = encodeURIComponent(query);
  switch (engine.type) {
    case "google":
      return `https://www.google.com/search?q=${q}`;
    case "bing":
      return `https://www.bing.com/search?q=${q}`;
    case "duck_duck_go":
      return `https://duckduckgo.com/?q=${q}`;
    case "custom":
      return engine.urlTemplate.replace("{query}", q);
  }
}

function quoteAsMarkdown(text: string): string {
  return text
    .split("\n")
    .map((line) => `> ${line}`)
    .join("\n");
}

function extractOutputForBash(exec: ToolExecution): string {
  if (!exec.output) return "";
  if (exec.output.kind === "text") return exec.output.text ?? "";
  return "";
}

function extractDiffText(exec: ToolExecution): string {
  // Edit / Write 的 output 通常是文本 diff；missing / structured 时返回空
  if (!exec.output) return "";
  if (exec.output.kind === "text") return exec.output.text ?? "";
  return "";
}

function firstLine(s: string): string {
  const idx = s.indexOf("\n");
  return idx === -1 ? s : s.slice(0, idx);
}

function parentDir(path: string): string {
  // POSIX / Windows 通用：找最后一个 / 或 \
  const idx = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  if (idx <= 0) return path;
  return path.slice(0, idx);
}
