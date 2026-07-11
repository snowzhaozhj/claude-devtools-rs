/**
 * 输出内容按规模自适应展示的判定逻辑（纯函数，无 DOM / 无副作用）。
 *
 * 行为契约：
 * - `openspec/specs/session-display/spec.md::对话流输出自适应展示的规模阈值`
 * - `openspec/specs/session-display/spec.md::对话流文本输出按内容规模自适应展示`
 * - `openspec/specs/tool-viewer-routing/spec.md::工具查看器按内容规模自适应展示`
 * - `openspec/specs/tool-viewer-routing/spec.md::工具输出懒加载态的稳定分档`
 * - `openspec/specs/tool-viewer-routing/spec.md::首尾切片的渲染上限与切分安全`
 *
 * 两类路径：
 * - markdown prose（参与 Cmd+F 全文搜索）：只有 inline / bounded 两档，不切片。
 * - 行导向工具输出（不参与 Cmd+F）：inline / bounded / oversized 三档，oversized 走 top/tail 切片。
 */

/** 限高档边界：行数 >= 80 或 UTF-8 字节 >= 16 KiB 即进入 bounded。 */
export const BOUNDED_LINE_THRESHOLD = 80;
export const BOUNDED_BYTE_THRESHOLD = 16 * 1024;

/** 超大档边界：行数 >= 1000 或 UTF-8 字节 >= 256 KiB 即进入 oversized（仅工具输出）。 */
export const OVERSIZED_LINE_THRESHOLD = 1000;
export const OVERSIZED_BYTE_THRESHOLD = 256 * 1024;

/** top/tail 切片每侧渲染上限（实现 tuning，见 design D7）。 */
export const SLICE_MAX_LINES_PER_SIDE = 400;
export const SLICE_MAX_BYTES_PER_SIDE = 128 * 1024;

export type OutputTier = "inline" | "bounded" | "oversized";

/**
 * 内容的 UTF-8 字节长度。与后端裁剪层记录的 `outputBytes` 同度量，
 * 不用 UTF-16 码元数 / `string.length`（多字节内容会与 `outputBytes` 得不同档）。
 */
export function utf8ByteLength(text: string): number {
  // TextEncoder 在所有目标环境可用；一次性编码已加载的字符串，代价可接受。
  return new TextEncoder().encode(text).length;
}

/**
 * 按换行符计数行数：末尾单个换行符不额外计为一空行。
 * 空串视为 0 行。
 */
export function countLines(text: string): number {
  if (text.length === 0) return 0;
  let lines = 1;
  for (let i = 0; i < text.length; i++) {
    if (text.charCodeAt(i) === 10 /* \n */) lines++;
  }
  // 末尾单个换行符不计空行。
  if (text.charCodeAt(text.length - 1) === 10) lines--;
  return Math.max(lines, 1);
}

/**
 * 按行数 + 字节数分档，`>=` 即升档，任一维度达标即升。
 * `allowOversized=false`（prose 路径）时 oversized 降级为 bounded（不切片）。
 */
export function classifyBySize(lines: number, bytes: number, allowOversized: boolean): OutputTier {
  const oversized =
    lines >= OVERSIZED_LINE_THRESHOLD || bytes >= OVERSIZED_BYTE_THRESHOLD;
  if (oversized) return allowOversized ? "oversized" : "bounded";
  if (lines >= BOUNDED_LINE_THRESHOLD || bytes >= BOUNDED_BYTE_THRESHOLD) {
    return "bounded";
  }
  return "inline";
}

/** 从完整文本分档（已加载内容）。 */
export function classifyText(text: string, allowOversized: boolean): OutputTier {
  return classifyBySize(countLines(text), utf8ByteLength(text), allowOversized);
}

/**
 * 工具输出懒加载态的稳定分档（见 spec `工具输出懒加载态的稳定分档`）。
 *
 * 规模信号优先级：已加载真实内容 > `outputBytes` > 未知。
 * - 已加载：按真实内容分档。
 * - 未加载但有 `outputBytes`：按字节维度分档；行数未知，为避免"字节短但行数超大"
 *   误判为 inline（会以不限高方式渲染巨量行），未加载态**下限锁定为 bounded**，
 *   即字节不足 bounded 时仍以 bounded 占位（`ready=false`），加载后再校正。
 * - 未加载且无 `outputBytes`（老后端 / 解析层未填）：fetch-first，bounded 占位。
 *
 * 裁剪空值（`omitted=true` 的空占位）不被当作 0 字节判入 inline。
 *
 * `ready=false` 表示外层应以稳定的 bounded 占位高度渲染、并触发懒加载；
 * `ready=true` 表示 `tier` 为最终档位。外层 viewport 几何在 ready 翻转前后
 * SHALL 由调用方保持 bounded 占位与 bounded 最终态一致，避免几何跳变。
 */
export interface ToolOutputSizing {
  ready: boolean;
  tier: OutputTier;
}

export function sizingForToolOutput(opts: {
  loadedText: string | null;
  outputBytes: number | undefined;
  omitted: boolean;
  allowOversized: boolean;
}): ToolOutputSizing {
  const { loadedText, outputBytes, omitted, allowOversized } = opts;

  // 已加载真实内容：最高优先级，直接按真实内容分档。
  if (loadedText !== null) {
    return { ready: true, tier: classifyText(loadedText, allowOversized) };
  }

  // 未加载。裁剪空值不判 0；未知维度（行数）不允许 inline。
  if (outputBytes !== undefined && outputBytes >= OVERSIZED_BYTE_THRESHOLD) {
    // 仅字节维度即可确定 oversized。
    return { ready: false, tier: allowOversized ? "oversized" : "bounded" };
  }
  // 字节不足 oversized、或字节未知：行数未知，锁定 bounded 占位，待加载校正。
  // omitted 但内容尚未到达 → 触发懒加载（ready=false）。
  void omitted;
  return { ready: false, tier: "bounded" };
}

/** top/tail 切片结果：省略量已按"总量 − 首尾实渲量"精确计算。 */
export interface SlicedOutput {
  head: string;
  tail: string;
  omittedLines: number;
  omittedBytes: number;
}

/**
 * 对行导向文本做 top/tail 切片（见 spec `首尾切片的渲染上限与切分安全`）。
 *
 * - 每侧最多 `SLICE_MAX_LINES_PER_SIDE` 行且 `SLICE_MAX_BYTES_PER_SIDE` 字节，任一先达即停。
 * - 重叠规避：总行数 <= 两侧行上限之和时返回 `null`（调用方退回限高预览完整渲染）。
 * - 切分点落在行边界；对无换行超大单行按字节上限 + Unicode 码点边界截取（不拆码点）。
 * - `omittedLines` / `omittedBytes` = 总量 − 首尾实渲量。
 */
export function sliceHeadTail(text: string): SlicedOutput | null {
  const totalBytes = utf8ByteLength(text);
  const lines = text.split("\n");
  const totalLines = countLines(text);

  // 单行（或近单行）超大：按字节切码点安全的首尾片段。
  if (lines.length <= 2) {
    if (totalBytes <= SLICE_MAX_BYTES_PER_SIDE * 2) return null;
    const head = takeBytesFromStart(text, SLICE_MAX_BYTES_PER_SIDE);
    const tail = takeBytesFromEnd(text, SLICE_MAX_BYTES_PER_SIDE);
    const shownBytes = utf8ByteLength(head) + utf8ByteLength(tail);
    return {
      head,
      tail,
      omittedLines: 0,
      omittedBytes: Math.max(totalBytes - shownBytes, 0),
    };
  }

  // 多行：按行取首尾，每侧受行 + 字节双上限约束。
  const head = takeLines(lines, SLICE_MAX_LINES_PER_SIDE, SLICE_MAX_BYTES_PER_SIDE, false);
  const tail = takeLines(lines, SLICE_MAX_LINES_PER_SIDE, SLICE_MAX_BYTES_PER_SIDE, true);

  // 重叠规避：首尾行数之和覆盖全部行则不切片。
  if (head.lineCount + tail.lineCount >= lines.length) return null;

  const shownBytes = utf8ByteLength(head.text) + utf8ByteLength(tail.text);
  return {
    head: head.text,
    tail: tail.text,
    omittedLines: Math.max(totalLines - head.lineCount - tail.lineCount, 0),
    omittedBytes: Math.max(totalBytes - shownBytes, 0),
  };
}

/** 结构化行数组的首尾切片索引结果。 */
export interface SliceIndices {
  headCount: number;
  tailCount: number;
  omittedLines: number;
  omittedBytes: number;
}

/**
 * 对结构化行数组（Read 的 `{num,text}[]` / Diff 的 `DiffLine[]` 等）计算
 * 首尾切片索引：调用方按 `slice(0, headCount)` / `slice(-tailCount)` 取行。
 * 预算与 `sliceHeadTail` 多行路径一致（每侧行 + 字节双上限，首行必取）；
 * 重叠规避同规则（首尾覆盖全部行时返回 `null`，退回限高预览）。
 *
 * `lineByteLengths` 为每行 UTF-8 字节数（不含换行）；字节核算内部按
 * 每行 +1 近似换行符，省略量 = 总量 − 首尾实渲量。
 */
export function sliceLineIndices(lineByteLengths: number[]): SliceIndices | null {
  const n = lineByteLengths.length;
  const take = (fromEnd: boolean): { count: number; bytes: number } => {
    let count = 0;
    let bytes = 0;
    for (let i = 0; i < n; i++) {
      const b = lineByteLengths[fromEnd ? n - 1 - i : i] + 1;
      if (count >= SLICE_MAX_LINES_PER_SIDE || (count > 0 && bytes + b > SLICE_MAX_BYTES_PER_SIDE)) {
        break;
      }
      count++;
      bytes += b;
    }
    return { count, bytes };
  };
  const head = take(false);
  const tail = take(true);
  if (head.count + tail.count >= n) return null;
  let totalBytes = 0;
  for (const b of lineByteLengths) totalBytes += b + 1;
  return {
    headCount: head.count,
    tailCount: tail.count,
    omittedLines: n - head.count - tail.count,
    omittedBytes: Math.max(totalBytes - head.bytes - tail.bytes, 0),
  };
}

function takeLines(
  lines: string[],
  maxLines: number,
  maxBytes: number,
  fromEnd: boolean,
): { text: string; lineCount: number } {
  const picked: string[] = [];
  let bytes = 0;
  const source = fromEnd ? [...lines].reverse() : lines;
  for (const line of source) {
    const lineBytes = utf8ByteLength(line) + 1; // +1 近似换行符
    if (picked.length >= maxLines || (picked.length > 0 && bytes + lineBytes > maxBytes)) {
      break;
    }
    picked.push(line);
    bytes += lineBytes;
  }
  const ordered = fromEnd ? picked.reverse() : picked;
  return { text: ordered.join("\n"), lineCount: ordered.length };
}

/** 从字符串开头取不超过 maxBytes 的 UTF-8 前缀，切点落在码点边界。 */
function takeBytesFromStart(text: string, maxBytes: number): string {
  let bytes = 0;
  let end = 0;
  for (const ch of text) {
    const chBytes = utf8ByteLength(ch);
    if (bytes + chBytes > maxBytes) break;
    bytes += chBytes;
    end += ch.length;
  }
  return text.slice(0, end);
}

/** 从字符串结尾取不超过 maxBytes 的 UTF-8 后缀，切点落在码点边界。 */
function takeBytesFromEnd(text: string, maxBytes: number): string {
  const chars = Array.from(text);
  let bytes = 0;
  let start = chars.length;
  for (let i = chars.length - 1; i >= 0; i--) {
    const chBytes = utf8ByteLength(chars[i]);
    if (bytes + chBytes > maxBytes) break;
    bytes += chBytes;
    start = i;
  }
  return chars.slice(start).join("");
}
