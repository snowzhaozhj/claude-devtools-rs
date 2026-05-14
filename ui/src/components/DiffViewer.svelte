<script lang="ts">
  import { getLanguageFromPath, getFileName, shortenPath } from "../lib/toolHelpers";
  import { generateDiff, type DiffLine } from "../lib/diff";

  interface Props {
    fileName: string;
    oldString: string;
    newString: string;
  }

  let { fileName, oldString, newString }: Props = $props();

  // LRU 缓存 LCS 结果：file-change re-render 时同一 (oldString,newString) 不再重算。
  // 用 length 前缀 + \0 分隔，避免 oldString 内含分隔符碰撞。
  const diffCache = new Map<string, DiffLine[]>();
  const DIFF_CACHE_CAP = 64;
  function cachedDiff(o: string, n: string): DiffLine[] {
    const key = `${o.length}\0${n.length}\0${o}\0${n}`;
    const hit = diffCache.get(key);
    if (hit !== undefined) {
      diffCache.delete(key);
      diffCache.set(key, hit);
      return hit;
    }
    const result = generateDiff(o, n);
    if (diffCache.size >= DIFF_CACHE_CAP) {
      const first = diffCache.keys().next().value;
      if (first !== undefined) diffCache.delete(first);
    }
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
  </div>

  <!-- Diff lines -->
  <div class="diff-body">
    <div class="diff-body-inner">
      {#each diffLines as line}
        <div
          class="diff-line"
          class:diff-line-added={line.type === "added"}
          class:diff-line-removed={line.type === "removed"}
        >
          <span class="diff-gutter diff-gutter-old">{line.oldLineNumber ?? ""}</span>
          <span class="diff-gutter diff-gutter-new">{line.newLineNumber ?? ""}</span>
          <span class="diff-prefix">{line.type === "added" ? "+" : line.type === "removed" ? "-" : " "}</span>
          <span class="diff-content">{line.content || " "}</span>
        </div>
      {/each}
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

  .diff-body {
    max-height: 400px;
    overflow: auto;
    background: var(--code-bg);
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
