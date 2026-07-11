<script lang="ts">
  import { getLanguageFromPath, getFileName, shortenPath } from "../lib/toolHelpers";
  import { generateDiff, type DiffLine } from "../lib/diff";
  import { highlightCode } from "../lib/render";
  import { ByteCappedCache } from "../lib/byteCappedCache";
  import CopyButton from "../lib/components/CopyButton.svelte";
  import { formatBytes } from "../lib/formatters";
  import { adaptiveScrollViewport } from "../lib/adaptiveViewport";
  import { classifyBySize, utf8ByteLength, sliceLineIndices } from "../lib/outputSizing";

  interface Props {
    fileName: string;
    oldString: string;
    newString: string;
  }

  let { fileName, oldString, newString }: Props = $props();

  // LRU 缓存 LCS 结果：file-change re-render 时同一 (oldString,newString) 不再重算。
  // 用 length 前缀 + \0 分隔，避免 oldString 内含分隔符碰撞。
  // key 含 old+new 完整内容，单条可达数 MB → count + byte 双闸门（见 byteCappedCache）。
  const diffCache = new ByteCappedCache<DiffLine[]>({
    maxEntries: 64,
    maxBytes: 4 * 1024 * 1024,
    sizeOf: (key, lines) => {
      let v = key.length;
      for (const l of lines) v += l.content.length + 16; // +16：type + 两个行号的粗略开销
      return v;
    },
  });

  function cachedDiff(o: string, n: string): DiffLine[] {
    const key = `${o.length}\0${n.length}\0${o}\0${n}`;
    const hit = diffCache.get(key);
    if (hit !== undefined) return hit;
    const result = generateDiff(o, n);
    diffCache.set(key, result);
    return result;
  }

  const diffLines = $derived(cachedDiff(oldString, newString));
  const stats = $derived.by(() => {
    let added = 0, removed = 0;
    for (const l of diffLines) {
      if (l.type === "added") added++;
      else if (l.type === "removed") removed++;
    }
    return { added, removed };
  });
  const language = $derived(getLanguageFromPath(fileName));
  const shortName = $derived(getFileName(fileName));

  // 三档分级：内容面 = old/new 差异内容（工具输入），不依赖 output
  // （spec tool-viewer-routing::编辑型工具无输出时按差异内容分档）。
  // 复制全文 = 完整差异文本（+/-/空格前缀逐行拼接）。diff 行导向，允许切片。
  const diffText = $derived(
    diffLines
      .map((l) => `${l.type === "added" ? "+" : l.type === "removed" ? "-" : " "}${l.content}`)
      .join("\n")
  );
  const totalLines = $derived(diffLines.length);
  const totalBytes = $derived(utf8ByteLength(diffText));
  const tier = $derived(classifyBySize(totalLines, totalBytes, true));
  const sliceIdx = $derived(
    tier === "oversized" ? sliceLineIndices(diffLines.map((l) => utf8ByteLength(l.content))) : null
  );
  const effectiveTier = $derived(tier === "oversized" && sliceIdx === null ? "bounded" : tier);
  const headDiffLines = $derived(sliceIdx ? diffLines.slice(0, sliceIdx.headCount) : diffLines);
  const tailDiffLines = $derived(sliceIdx ? diffLines.slice(diffLines.length - sliceIdx.tailCount) : []);
  const scent = $derived(`${totalLines} 行 · ${formatBytes(totalBytes)} · 预览`);
</script>

<div class="diff-viewer">
  <!-- Header -->
  <div class="diff-header">
    <svg class="diff-icon" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/>
    </svg>
    <span class="diff-filename" title={fileName}>{shortenPath(fileName)}</span>
    <span class="diff-lang-tag">{language}</span>
    {#if stats.added > 0}
      <span class="diff-stat diff-stat-added">+{stats.added}</span>
    {/if}
    {#if stats.removed > 0}
      <span class="diff-stat diff-stat-removed">-{stats.removed}</span>
    {/if}
    {#if effectiveTier !== "inline"}
      <span class="diff-scent">{scent}</span>
    {/if}
    <span class="diff-header-spacer"></span>
    <CopyButton text={diffText} ariaLabel="复制完整差异" />
  </div>

  {#snippet diffRows(rows: DiffLine[])}
    {#each rows as line}
      <div
        class="diff-line"
        class:diff-line-added={line.type === "added"}
        class:diff-line-removed={line.type === "removed"}
      >
        <span class="diff-gutter diff-gutter-old">{line.oldLineNumber ?? ""}</span>
        <span class="diff-gutter diff-gutter-new">{line.newLineNumber ?? ""}</span>
        <span class="diff-prefix">{line.type === "added" ? "+" : line.type === "removed" ? "-" : " "}</span>
        <span class="diff-content">{@html line.content ? highlightCode(line.content, language) : " "}</span>
      </div>
    {/each}
  {/snippet}

  <!-- Diff lines -->
  <div
    class="diff-body"
    class:bounded={effectiveTier !== "inline"}
    {@attach adaptiveScrollViewport(() => `Diff ${shortName}（${scent}，可滚动）`)}
  >
    <div class="diff-body-inner">
      {#if effectiveTier === "oversized" && sliceIdx}
        {@render diffRows(headDiffLines)}
        <div class="diff-seam" role="separator">
          已省略 {sliceIdx.omittedLines} 行 · {formatBytes(sliceIdx.omittedBytes)}
        </div>
        {@render diffRows(tailDiffLines)}
      {:else}
        {@render diffRows(diffLines)}
      {/if}
    </div>
  </div>
</div>

<style>
  .diff-viewer {
    min-width: 0;
    border: 1px solid var(--code-border);
    border-radius: 6px;
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .diff-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    background: var(--code-header-bg);
    border-bottom: 1px solid var(--code-border);
  }

  .diff-icon {
    flex-shrink: 0;
    color: var(--color-text-muted);
  }

  .diff-filename {
    color: var(--code-filename);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .diff-lang-tag {
    font-size: 10px;
    color: var(--color-text-muted);
    background: var(--badge-neutral-bg);
    padding: 1px 6px;
    border-radius: 3px;
    flex-shrink: 0;
  }

  .diff-stat {
    font-size: 11px;
    font-weight: 600;
    flex-shrink: 0;
  }

  .diff-stat-added { color: var(--diff-added-text); }
  .diff-stat-removed { color: var(--diff-removed-text); }

  /* 信息气味：总行数 · 总字节数 · 预览（mono metadata，中性色）。 */
  .diff-scent {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-secondary);
    white-space: nowrap;
  }

  .diff-header-spacer {
    flex: 1 1 auto;
  }

  .diff-body {
    overflow: auto;
    scrollbar-gutter: stable;
    background: var(--code-bg);
  }

  /* bounded / oversized：响应式限高（共享 token），inline 不限高。 */
  .diff-body.bounded {
    max-block-size: var(--ao-preview-max-block);
  }

  .diff-body:focus-visible {
    outline: 2px solid var(--color-accent-blue, #3b82f6);
    outline-offset: -2px;
  }

  /* 省略接缝：中性文字 + 细分隔线，显式标注省略量（不用渐隐遮罩）。 */
  .diff-seam {
    font-size: 11px;
    color: var(--color-text-secondary);
    padding: 4px 12px 4px 74px;
    border-block: 1px dashed var(--color-border);
    white-space: normal;
  }

  .diff-body-inner {
    display: inline-block;
    min-width: 100%;
  }

  .diff-line {
    display: flex;
    line-height: 1.5;
    min-height: 20px;
  }

  .diff-line-added {
    background: var(--diff-added-bg);
  }

  .diff-line-removed {
    background: var(--diff-removed-bg);
  }

  .diff-gutter {
    width: 34px;
    min-width: 34px;
    text-align: right;
    padding-right: 6px;
    color: var(--code-line-number);
    user-select: none;
    flex-shrink: 0;
  }

  .diff-prefix {
    width: 20px;
    min-width: 20px;
    text-align: center;
    flex-shrink: 0;
    user-select: none;
  }

  .diff-line-added .diff-prefix { color: var(--diff-added-text); font-weight: 700; }
  .diff-line-removed .diff-prefix { color: var(--diff-removed-text); font-weight: 700; }

  .diff-content {
    flex: 1;
    white-space: pre;
    padding-right: 8px;
  }

  /* hljs token 颜色 + .hljs 容器透明背景统一在 app.css 全局规则里。
     行 +/- 由 .diff-line-* 的背景区分，token 颜色继承自全局 .hljs-* */
</style>
