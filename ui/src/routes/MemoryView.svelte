<script lang="ts">
  import { openPath } from "@tauri-apps/plugin-opener";
  import { getProjectMemory, readMemoryFile, type MemoryLayer, type ProjectMemory } from "../lib/api";
  import { renderMarkdown } from "../lib/render";

  interface Props {
    projectId: string;
  }

  let { projectId }: Props = $props();

  let memory: ProjectMemory | null = $state(null);
  let selectedFile: string | null = $state(null);
  let content = $state("");
  let filePath: string | null = $state(null);
  let loading = $state(true);
  let contentLoading = $state(false);
  let error: string | null = $state(null);
  let copyState: "idle" | "copied" | "error" = $state("idle");
  let memoryRequestId = 0;
  let fileRequestId = 0;

  const selectedLayer = $derived.by(() => {
    if (!memory) return null;
    return memory.layers.find((layer: MemoryLayer) => layer.file === selectedFile) ?? null;
  });
  const selectedPathLabel = $derived.by(() => {
    if (filePath) return filePath.split(/[\\/]/).slice(-3).join("/");
    return selectedFile ?? "";
  });
  const renderedContent = $derived(content ? renderMarkdown(content) : "");

  function kindLabel(kind: MemoryLayer["kind"]): string {
    if (kind === "index") return "index";
    if (kind === "entry") return "linked";
    return "file";
  }

  $effect(() => {
    if (!projectId) return;
    void loadMemory(projectId);
  });

  async function loadMemory(id: string) {
    const requestId = ++memoryRequestId;
    loading = true;
    error = null;
    content = "";
    try {
      const next = await getProjectMemory(id);
      if (requestId !== memoryRequestId || id !== projectId) return;
      memory = next;
      const nextFile = next.defaultFile ?? next.layers[0]?.file ?? null;
      selectedFile = nextFile;
      if (nextFile) await loadFile(id, nextFile);
    } catch (e) {
      if (requestId !== memoryRequestId || id !== projectId) return;
      error = e instanceof Error ? e.message : String(e);
      memory = null;
      selectedFile = null;
    } finally {
      if (requestId === memoryRequestId && id === projectId) loading = false;
    }
  }

  async function loadFile(id: string, file: string) {
    const requestId = ++fileRequestId;
    contentLoading = true;
    error = null;
    copyState = "idle";
    try {
      const result = await readMemoryFile(id, file);
      if (requestId !== fileRequestId || id !== projectId) return;
      content = result.content;
      filePath = result.filePath;
      selectedFile = result.file;
    } catch (e) {
      if (requestId !== fileRequestId || id !== projectId) return;
      error = e instanceof Error ? e.message : String(e);
      content = "";
    } finally {
      if (requestId === fileRequestId && id === projectId) contentLoading = false;
    }
  }

  function selectLayer(layer: MemoryLayer) {
    if (layer.file === selectedFile) return;
    void loadFile(projectId, layer.file);
  }

  function resolveMemoryHref(href: string): string | null {
    const clean = href.split("#", 1)[0];
    const file = decodeURIComponent(clean).split("/").pop() ?? "";
    if (!file.endsWith(".md")) return null;
    if (!memory?.layers.some((layer) => layer.file === file)) return null;
    return file;
  }

  function onMarkdownClick(e: MouseEvent) {
    const target = e.target;
    if (!(target instanceof Element)) return;
    const anchor = target.closest("a[href]") as HTMLAnchorElement | null;
    const href = anchor?.getAttribute("href");
    if (!href) return;
    const file = resolveMemoryHref(href);
    if (!file) return;
    e.preventDefault();
    if (file !== selectedFile) void loadFile(projectId, file);
  }

  async function openCurrentFile() {
    if (!filePath) return;
    try {
      await openPath(filePath);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function copyContent() {
    if (!content || contentLoading) return;
    try {
      await navigator.clipboard.writeText(content);
      copyState = "copied";
    } catch {
      copyState = "error";
    }
    window.setTimeout(() => {
      copyState = "idle";
    }, 1200);
  }
</script>

<div class="memory-view">
  {#if loading}
    <div class="memory-status">加载 Memory...</div>
  {:else if error && !memory}
    <div class="memory-status memory-error">{error}</div>
  {:else if !memory || memory.layers.length === 0}
    <div class="memory-status">当前项目没有 Memory。</div>
  {:else}
    <aside class="memory-layers">
      <div class="layers-header">
        <span>LAYERS</span>
        <span>{memory.count}</span>
      </div>
      <div class="layer-stack">
        {#each memory.layers as layer (layer.file)}
          <button
            class="layer-item"
            class:layer-active={layer.file === selectedFile}
            onclick={() => selectLayer(layer)}
          >
            <span class="layer-topline">
              <span class="layer-title">{layer.title}</span>
              <span class="layer-kind">{kindLabel(layer.kind)}</span>
            </span>
            <span class="layer-file">{layer.file}</span>
            {#if layer.hook}
              <span class="layer-hook">{layer.hook}</span>
            {/if}
          </button>
        {/each}
      </div>
    </aside>

    <section class="memory-content">
      <div class="memory-toolbar">
        <div class="current-file" title={filePath ?? selectedFile ?? ""}>
          <span class="current-file-name">{selectedFile}</span>
          <span class="current-file-path">{selectedPathLabel}</span>
        </div>
        <div class="toolbar-actions">
          <select
            class="file-select"
            value={selectedFile ?? ""}
            disabled={contentLoading}
            onchange={(e) => {
              const file = e.currentTarget.value;
              if (file) void loadFile(projectId, file);
            }}
            aria-label="选择 Memory 文件"
          >
            {#each memory.layers as layer (layer.file)}
              <option value={layer.file}>{layer.file}</option>
            {/each}
          </select>
          <button
            class="toolbar-button"
            disabled={!filePath || contentLoading}
            onclick={openCurrentFile}
          >
            Open
          </button>
          <button
            class="toolbar-button"
            disabled={!content || contentLoading}
            onclick={copyContent}
          >
            {copyState === "copied" ? "已复制" : copyState === "error" ? "复制失败" : "Copy"}
          </button>
        </div>
      </div>

      {#if error}
        <div class="content-status memory-error">{error}</div>
      {:else if contentLoading}
        <div class="content-status">加载文件...</div>
      {:else}
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_click_events_have_key_events -->
        <article class="markdown-body" onclick={onMarkdownClick}>
          {#if selectedLayer}
            <h1>{selectedLayer.title}</h1>
          {/if}
          {@html renderedContent}
        </article>
      {/if}
    </section>
  {/if}
</div>

<style>
  .memory-view {
    flex: 1;
    min-height: 0;
    display: flex;
    background: var(--color-surface);
    color: var(--color-text);
  }

  .memory-status,
  .content-status {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .memory-error {
    color: var(--color-error, #ef4444);
  }

  .memory-layers {
    width: 324px;
    flex-shrink: 0;
    overflow-y: auto;
    border-right: 1px solid var(--color-border);
    padding: 18px 12px;
    background: var(--color-surface-sidebar);
  }

  .layers-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
    padding: 0 8px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: var(--color-text-muted);
  }

  .layer-stack {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .layer-item {
    position: relative;
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 10px 9px 11px;
    border: 1px solid transparent;
    border-radius: 9px;
    background: transparent;
    color: var(--color-text);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background 0.12s ease-out, border-color 0.12s ease-out;
  }

  .layer-item:hover {
    background: var(--tool-item-hover-bg);
  }

  .layer-active {
    background: var(--color-surface-raised);
    border-color: var(--color-border-emphasis);
  }

  .layer-topline {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .layer-title {
    min-width: 0;
    font-size: 13px;
    font-weight: 600;
    line-height: 1.3;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .layer-kind {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 1;
    color: var(--color-text-muted);
    border: 1px solid var(--color-border);
    border-radius: 999px;
    padding: 3px 6px;
    background: var(--color-surface);
  }

  .layer-file,
  .layer-hook {
    font-size: 12px;
    line-height: 1.35;
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .layer-file {
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .memory-content {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .memory-toolbar {
    min-height: 52px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    padding: 8px 22px;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-surface);
  }

  .current-file {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .current-file-name {
    font-size: 13px;
    font-weight: 600;
    line-height: 1.25;
    color: var(--color-text);
  }

  .current-file-path {
    max-width: 46vw;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-muted);
  }

  .toolbar-actions {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .file-select,
  .toolbar-button {
    height: 30px;
    padding: 0 11px;
    border: 1px solid var(--color-border);
    border-radius: 7px;
    background: transparent;
    color: var(--color-text-secondary);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
    transition: background 0.12s ease-out, color 0.12s ease-out, border-color 0.12s ease-out;
  }

  .file-select {
    max-width: 220px;
  }

  .toolbar-button:hover:not(:disabled) {
    background: var(--color-surface-raised);
    color: var(--color-text);
    border-color: var(--color-border-emphasis);
  }

  .toolbar-button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .markdown-body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 42px 64px;
    font-size: 15px;
    line-height: 1.7;
  }

  .markdown-body > :global(*) {
    max-width: 72ch;
  }

  .markdown-body :global(h1) {
    margin: 0 0 20px;
    font-size: 24px;
    line-height: 1.2;
  }

  .markdown-body :global(h2),
  .markdown-body :global(h3) {
    margin: 24px 0 12px;
  }

  .markdown-body :global(p),
  .markdown-body :global(ul),
  .markdown-body :global(ol) {
    margin: 0 0 14px;
  }

  .markdown-body :global(pre) {
    overflow-x: auto;
    padding: 12px;
    border-radius: 8px;
    background: var(--code-bg, var(--color-surface-sidebar));
  }

  .markdown-body :global(code) {
    font-family: var(--font-mono);
  }
</style>
