<script lang="ts">
  /**
   * 路径节点。`path` 是绝对路径（用于 home 替换）；`estimatedTokens` 用于显示。
   * ClaudeMd / MentionedFile injection 都能 fit（皆有 path + estimatedTokens 字段）。
   */
  export interface TreeEntry {
    path: string;
    estimatedTokens: number;
  }

  interface Props {
    entries: TreeEntry[];
  }

  let { entries }: Props = $props();

  // ── 目录树构建 ──

  interface TreeNode {
    name: string;
    path: string;
    isFile: boolean;
    tokens: number;
    children: Map<string, TreeNode>;
  }

  function buildTree(items: TreeEntry[]): TreeNode {
    const root: TreeNode = { name: "", path: "", isFile: false, tokens: 0, children: new Map() };

    for (const item of items) {
      const p = (item.path ?? "").replace(/^\/Users\/[^/]+\/?/, "");
      const parts = p.split("/").filter(Boolean);
      let current = root;

      for (let i = 0; i < parts.length; i++) {
        const part = parts[i];
        const isLast = i === parts.length - 1;

        if (!current.children.has(part)) {
          current.children.set(part, {
            name: part,
            path: isLast ? item.path ?? "" : "",
            isFile: isLast,
            tokens: isLast ? item.estimatedTokens : 0,
            children: new Map(),
          });
        } else if (isLast) {
          const node = current.children.get(part)!;
          node.isFile = true;
          node.tokens = item.estimatedTokens;
          node.path = item.path ?? "";
        }
        current = current.children.get(part)!;
      }
    }

    return root;
  }

  function sortedChildren(node: TreeNode): TreeNode[] {
    return Array.from(node.children.values()).sort((a, b) => {
      if (a.isFile && !b.isFile) return -1;
      if (!a.isFile && b.isFile) return 1;
      return a.name.localeCompare(b.name);
    });
  }

  function fk(n: number): string {
    if (n >= 1e6) return (n / 1e6).toFixed(1) + "M";
    if (n >= 1e3) return (n / 1e3).toFixed(1) + "k";
    return String(n);
  }

  const tree = $derived(buildTree(entries));

  // 折叠状态：path → collapsed
  let collapsed: Set<string> = $state(new Set());

  function toggleDir(path: string) {
    const n = new Set(collapsed);
    if (n.has(path)) n.delete(path); else n.add(path);
    collapsed = n;
  }
</script>

{#snippet treeNode(node: TreeNode, depth: number, parentPath: string)}
  {@const children = sortedChildren(node)}
  {@const nodePath = parentPath ? `${parentPath}/${node.name}` : node.name}

  {#if node.isFile}
    <div class="dt-file" class:dt-nested={depth > 0} style:--dt-depth={String(depth)}>
      <span class="dt-file-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" />
          <path d="M14 2v4a2 2 0 0 0 2 2h4" />
        </svg>
      </span>
      <span class="dt-file-name" title={node.path || node.name}>{node.name}</span>
      {#if node.tokens > 0}
        <span class="dt-tokens">~{fk(node.tokens)}</span>
      {/if}
    </div>
  {:else if node.name}
    <button
      type="button"
      class="dt-dir"
      class:dt-dir-expanded={!collapsed.has(nodePath)}
      class:dt-nested={depth > 0}
      style:--dt-depth={String(depth)}
      onclick={() => toggleDir(nodePath)}
      aria-expanded={!collapsed.has(nodePath)}
    >
      <span class="dt-chevron" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="m9 18 6-6-6-6" />
        </svg>
      </span>
      <span class="dt-dir-name">{node.name}</span>
      <span class="dt-dir-count">{children.length}</span>
    </button>
  {/if}

  {#if !node.name || !collapsed.has(nodePath)}
    {#each children as child}
      {@render treeNode(child, node.name ? depth + 1 : depth, nodePath)}
    {/each}
  {/if}
{/snippet}

<div class="directory-tree">
  {@render treeNode(tree, 0, "")}
</div>

<style>
  .directory-tree {
    min-width: 0;
    font-size: 12px;
    font-family: var(--font-mono);
    line-height: 1.35;
  }

  .dt-file,
  .dt-dir {
    position: relative;
    display: grid;
    grid-template-columns: 14px minmax(0, 1fr) auto;
    align-items: center;
    gap: 6px;
    width: 100%;
    min-width: 0;
    padding: 3px 6px 3px calc(var(--dt-depth, 0) * 14px + 4px);
    border-radius: 5px;
  }

  .dt-file {
    color: var(--color-text-secondary);
  }

  .dt-file::before,
  .dt-dir::before {
    content: "";
    position: absolute;
    left: calc(var(--dt-depth, 0) * 14px - 6px);
    top: 0;
    bottom: 0;
    display: none;
    border-left: 1px solid var(--color-border-subtle, var(--color-border));
  }

  .dt-nested::before {
    display: block;
  }

  .dt-file:hover,
  .dt-dir:hover {
    background: var(--tool-item-hover-bg);
  }

  .dt-file-icon,
  .dt-chevron {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    color: var(--color-text-muted);
  }

  .dt-file-icon svg,
  .dt-chevron svg {
    width: 13px;
    height: 13px;
  }

  .dt-file-name {
    min-width: 0;
    overflow: hidden;
    color: var(--color-text-secondary);
    font-weight: 400;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dt-tokens {
    color: var(--color-text-muted);
    font-size: 10px;
    font-variant-numeric: tabular-nums;
  }

  .dt-dir {
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .dt-dir:focus-visible {
    outline: 2px solid var(--color-accent-blue);
    outline-offset: 1px;
  }

  .dt-chevron {
    transition: transform 0.15s ease, color 0.1s;
  }

  .dt-dir-expanded .dt-chevron {
    color: var(--color-text-secondary);
    transform: rotate(90deg);
  }

  .dt-dir-name {
    min-width: 0;
    overflow: hidden;
    color: var(--color-text-muted);
    font-weight: 500;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dt-dir-expanded .dt-dir-name {
    color: var(--color-text-secondary);
  }

  .dt-dir-count {
    border-radius: 4px;
    background: var(--color-surface-overlay, var(--badge-neutral-bg));
    color: var(--color-text-muted);
    font-size: 10px;
    line-height: 1;
    padding: 2px 4px;
    font-variant-numeric: tabular-nums;
  }
</style>
