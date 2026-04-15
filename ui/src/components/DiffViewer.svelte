<script lang="ts">
  import { getLanguageFromPath, getFileName, shortenPath } from "../lib/toolHelpers";

  interface Props {
    fileName: string;
    oldString: string;
    newString: string;
  }

  let { fileName, oldString, newString }: Props = $props();

  // ── LCS diff 算法 ──

  interface DiffLine {
    type: "added" | "removed" | "context";
    content: string;
    oldNum: number | null;
    newNum: number | null;
  }

  function computeLCS(a: string[], b: string[]): number[][] {
    const m = a.length, n = b.length;
    const dp: number[][] = Array.from({ length: m + 1 }, () => new Array(n + 1).fill(0));
    for (let i = 1; i <= m; i++) {
      for (let j = 1; j <= n; j++) {
        dp[i][j] = a[i - 1] === b[j - 1]
          ? dp[i - 1][j - 1] + 1
          : Math.max(dp[i - 1][j], dp[i][j - 1]);
      }
    }
    return dp;
  }

  function generateDiff(oldText: string, newText: string): DiffLine[] {
    const oldLines = oldText.split("\n");
    const newLines = newText.split("\n");
    const dp = computeLCS(oldLines, newLines);
    const result: DiffLine[] = [];

    let i = oldLines.length, j = newLines.length;
    let oldNum = i, newNum = j;

    while (i > 0 || j > 0) {
      if (i > 0 && j > 0 && oldLines[i - 1] === newLines[j - 1]) {
        result.push({ type: "context", content: oldLines[i - 1], oldNum: oldNum--, newNum: newNum-- });
        i--; j--;
      } else if (j > 0 && (i === 0 || dp[i][j - 1] >= dp[i - 1][j])) {
        result.push({ type: "added", content: newLines[j - 1], oldNum: null, newNum: newNum-- });
        j--;
      } else {
        result.push({ type: "removed", content: oldLines[i - 1], oldNum: oldNum--, newNum: null });
        i--;
      }
    }

    return result.reverse();
  }

  const diffLines = $derived(generateDiff(oldString, newString));
  const stats = $derived(() => {
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
    {#if stats().added > 0}
      <span class="diff-stat diff-stat-added">+{stats().added}</span>
    {/if}
    {#if stats().removed > 0}
      <span class="diff-stat diff-stat-removed">-{stats().removed}</span>
    {/if}
  </div>

  <!-- Diff lines -->
  <div class="diff-body">
    {#each diffLines as line}
      <div
        class="diff-line"
        class:diff-line-added={line.type === "added"}
        class:diff-line-removed={line.type === "removed"}
      >
        <span class="diff-gutter diff-gutter-old">{line.oldNum ?? ""}</span>
        <span class="diff-gutter diff-gutter-new">{line.newNum ?? ""}</span>
        <span class="diff-prefix">{line.type === "added" ? "+" : line.type === "removed" ? "-" : " "}</span>
        <span class="diff-content">{line.content || " "}</span>
      </div>
    {/each}
  </div>
</div>

<style>
  .diff-viewer {
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
    width: 40px;
    min-width: 40px;
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
    overflow-x: auto;
    padding-right: 8px;
  }

  .diff-line-added .diff-content { color: var(--diff-added-text); }
  .diff-line-removed .diff-content { color: var(--diff-removed-text); }
</style>
