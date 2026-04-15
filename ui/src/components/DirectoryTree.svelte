<script lang="ts">
  import type { ContextEntry } from "../lib/contextExtractor";

  interface Props {
    entries: ContextEntry[];
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

  function buildTree(items: ContextEntry[]): TreeNode {
    const root: TreeNode = { name: "", path: "", isFile: false, tokens: 0, children: new Map() };

    for (const item of items) {
      const p = (item.path ?? "").replace(/^\/Users\/[^/]+/, "~");
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
    <div class="dt-file" style:padding-left="{depth * 12}px">
      <span class="dt-file-name">{node.name}</span>
      {#if node.tokens > 0}
        <span class="dt-tokens">~{fk(node.tokens)}</span>
      {/if}
    </div>
  {:else if node.name}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="dt-dir" style:padding-left="{depth * 12}px" onclick={() => toggleDir(nodePath)}>
      <span class="dt-chevron" class:dt-chevron-open={!collapsed.has(nodePath)}>▸</span>
      <span class="dt-dir-name">{node.name}/</span>
    </div>
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
    font-size: 12px;
    font-family: var(--font-mono);
  }

  .dt-file {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 2px 0;
  }

  .dt-file-name {
    color: var(--color-text);
  }

  .dt-tokens {
    font-size: 10px;
    color: var(--color-text-muted);
  }

  .dt-dir {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 0;
    cursor: pointer;
  }

  .dt-dir:hover {
    opacity: 0.8;
  }

  .dt-chevron {
    font-size: 9px;
    color: var(--color-text-muted);
    width: 10px;
    transition: transform 0.15s ease;
  }

  .dt-chevron-open {
    transform: rotate(90deg);
  }

  .dt-dir-name {
    color: var(--color-text-muted);
  }
</style>
