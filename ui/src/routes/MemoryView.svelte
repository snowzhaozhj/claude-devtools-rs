<script lang="ts">
  import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
  import { getProjectMemory, readMemoryFile, type MemoryLayer, type ProjectMemory } from "../lib/api";
  import { renderMarkdown } from "../lib/render";
  import { splitFrontmatter, type MemoryFrontmatter } from "../lib/memoryFrontmatter";
  import Skeleton from "../components/Skeleton.svelte";
  import SkeletonList from "../components/SkeletonList.svelte";

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
  let openMenuOpen = $state(false);
  let openMenuRoot: HTMLDivElement | null = $state(null);
  let openActionState: "idle" | "ok" | "error" = $state("idle");
  let openActionMessage = $state("");
  let memoryRequestId = 0;
  let fileRequestId = 0;

  const selectedLayer = $derived.by(() => {
    if (!memory) return null;
    return memory.layers.find((layer: MemoryLayer) => layer.file === selectedFile) ?? null;
  });
  const split = $derived(content ? splitFrontmatter(content) : { frontmatter: null, body: "" });
  const frontmatter: MemoryFrontmatter | null = $derived(split.frontmatter);
  const renderedBody = $derived(split.body ? renderMarkdown(split.body) : "");
  const metadataEntries = $derived(frontmatter ? Object.entries(frontmatter.metadata) : []);

  $effect(() => {
    if (!projectId) return;
    void loadMemory(projectId);
  });

  $effect(() => {
    if (!openMenuOpen) return;
    const onPointerDown = (e: PointerEvent) => {
      if (openMenuRoot && !openMenuRoot.contains(e.target as Node)) {
        openMenuOpen = false;
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") openMenuOpen = false;
    };
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKey);
    };
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
    openActionState = "idle";
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

  function flashOpenAction(state: "ok" | "error", message: string) {
    openActionState = state;
    openActionMessage = message;
    window.setTimeout(() => {
      openActionState = "idle";
      openActionMessage = "";
    }, 1600);
  }

  async function openWithDefault() {
    openMenuOpen = false;
    if (!filePath) return;
    try {
      await openPath(filePath);
      flashOpenAction("ok", "已用默认应用打开");
    } catch (e) {
      flashOpenAction("error", e instanceof Error ? e.message : String(e));
    }
  }

  async function revealInFinder() {
    openMenuOpen = false;
    if (!filePath) return;
    try {
      await revealItemInDir(filePath);
      flashOpenAction("ok", "已在文件管理器中显示");
    } catch (e) {
      flashOpenAction("error", e instanceof Error ? e.message : String(e));
    }
  }

  async function copyPath() {
    openMenuOpen = false;
    if (!filePath) return;
    try {
      await navigator.clipboard.writeText(filePath);
      flashOpenAction("ok", "路径已复制");
    } catch (e) {
      flashOpenAction("error", e instanceof Error ? e.message : String(e));
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

  function kindLabel(kind: MemoryLayer["kind"]): string {
    if (kind === "index") return "index";
    if (kind === "entry") return "linked";
    return "file";
  }
</script>

<div class="memory-view">
  {#if loading && !memory}
    <aside class="memory-layers">
      <div class="layers-header">
        <span>LAYERS</span>
      </div>
      <SkeletonList count={5} rowHeight={56} gap={6} padding="6px" label="正在加载 Memory 层" />
    </aside>
    <section class="memory-content">
      <!-- 用真 toolbar 结构（背景 / 边框 / 高度）占位，避免加载完成切到主分支时
           layout 突然多出 52px toolbar 造成的 markdown 区域跳动。 -->

      <div class="memory-toolbar memory-toolbar-skeleton" aria-hidden="true">
        <div class="current-file">
          <Skeleton variant="text" height={14} width="120px" />
          <Skeleton variant="text" height={11} width="200px" />
        </div>
        <div class="toolbar-actions">
          <Skeleton variant="row" height={30} width="60px" />
          <Skeleton variant="row" height={30} width="60px" />
        </div>
      </div>
      <div class="memory-content-skeleton" role="status" aria-busy="true" aria-label="正在加载 Memory 文件">
        <Skeleton variant="text" height={28} width="40%" />
        <Skeleton variant="text" height={14} width="92%" />
        <Skeleton variant="text" height={14} width="88%" />
        <Skeleton variant="text" height={14} width="76%" />
      </div>
    </section>
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
          <span class="current-file-name" data-testid="memory-current-file">{selectedFile}</span>
          {#if filePath}
            <span class="current-file-path">{filePath}</span>
          {/if}
        </div>
        <div class="toolbar-actions">
          {#if openActionState !== "idle"}
            <span class="action-flash" class:action-flash-error={openActionState === "error"}>
              {openActionMessage}
            </span>
          {/if}
          <button
            class="toolbar-button"
            disabled={!content || contentLoading}
            onclick={copyContent}
          >
            {copyState === "copied" ? "已复制" : copyState === "error" ? "复制失败" : "Copy"}
          </button>
          <div class="open-menu" bind:this={openMenuRoot}>
            <button
              class="toolbar-button"
              disabled={!filePath || contentLoading}
              aria-haspopup="menu"
              aria-expanded={openMenuOpen}
              onclick={(e) => {
                e.stopPropagation();
                openMenuOpen = !openMenuOpen;
              }}
            >
              Open in… <span class="chevron">▾</span>
            </button>
            {#if openMenuOpen}
              <div class="open-menu-list" role="menu">
                <button class="open-menu-item" role="menuitem" onclick={revealInFinder}>
                  <span class="item-label">在文件管理器中显示</span>
                </button>
                <button class="open-menu-item" role="menuitem" onclick={openWithDefault}>
                  <span class="item-label">用默认应用打开</span>
                </button>
                <button
                  class="open-menu-item open-menu-item-divider"
                  role="menuitem"
                  onclick={copyPath}
                >
                  <span class="item-label">复制路径</span>
                </button>
              </div>
            {/if}
          </div>
        </div>
      </div>

      {#if error}
        <div class="content-status memory-error">{error}</div>
      {:else if contentLoading && !content}
        <div class="memory-content-skeleton" role="status" aria-busy="true" aria-label="正在加载文件内容">
          <Skeleton variant="text" height={28} width="40%" />
          <Skeleton variant="text" height={14} width="92%" />
          <Skeleton variant="text" height={14} width="88%" />
          <Skeleton variant="text" height={14} width="76%" />
          <Skeleton variant="text" height={14} width="84%" />
        </div>
      {:else}
        <div class="content-scroll">
          <div class="content-inner">
            {#if frontmatter}
              <div class="frontmatter-card">
                {#if frontmatter.name}
                  <div class="fm-row">
                    <span class="fm-label">name</span>
                    <span class="fm-value fm-mono">{frontmatter.name}</span>
                  </div>
                {/if}
                {#if frontmatter.description}
                  <div class="fm-row">
                    <span class="fm-label">description</span>
                    <span class="fm-value fm-desc">{frontmatter.description}</span>
                  </div>
                {/if}
                {#if metadataEntries.length > 0}
                  <div class="fm-meta">
                    {#each metadataEntries as [key, value] (key)}
                      <span class="fm-chip">
                        <span class="fm-label fm-label-inline">{key}</span>
                        <span class="fm-mono fm-chip-value">{value}</span>
                      </span>
                    {/each}
                  </div>
                {/if}
              </div>
            {/if}
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_click_events_have_key_events -->
            <article class="markdown-body" onclick={onMarkdownClick}>
              {#if selectedLayer && !frontmatter}
                <h1 class="layer-heading">{selectedLayer.title}</h1>
              {/if}
              {@html renderedBody}
            </article>
          </div>
        </div>
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
    min-height: 48px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    padding: 6px 18px;
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
    font-size: 12px;
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

  .toolbar-button {
    height: 28px;
    padding: 0 10px;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: transparent;
    color: var(--color-text-secondary);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    transition: background 0.12s ease-out, color 0.12s ease-out, border-color 0.12s ease-out;
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

  .chevron {
    font-size: 10px;
    line-height: 1;
    color: var(--color-text-muted);
  }

  .action-flash {
    font-size: 11px;
    color: var(--color-text-muted);
  }

  .action-flash-error {
    color: var(--color-error);
  }

  .open-menu {
    position: relative;
    display: inline-block;
  }

  .open-menu-list {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    z-index: 30;
    min-width: 200px;
    background: var(--color-surface-overlay);
    border: 1px solid var(--color-border-emphasis);
    border-radius: 6px;
    overflow: hidden;
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.18);
  }

  .open-menu-item {
    width: 100%;
    padding: 8px 12px;
    background: transparent;
    border: 0;
    color: var(--color-text);
    font: inherit;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
    display: flex;
    align-items: center;
  }

  .open-menu-item-divider {
    border-top: 1px solid var(--color-border);
  }

  .open-menu-item:hover {
    background: var(--color-surface-raised);
  }

  .item-label {
    flex: 1;
  }

  .memory-content-skeleton {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 28px 56px;
  }

  .content-scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  .content-inner {
    padding: 28px 56px 60px;
    max-width: 920px;
  }

  .frontmatter-card {
    margin-bottom: 18px;
    padding: 10px 14px;
    border: 1px solid var(--color-border);
    border-radius: 8px;
    background: var(--color-surface-overlay);
    font-size: 12px;
    color: var(--prose-body);
  }

  .fm-row {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 2px 0;
  }

  .fm-label {
    flex-shrink: 0;
    min-width: 5.2rem;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--color-text-muted);
  }

  .fm-label-inline {
    min-width: 0;
  }

  .fm-value {
    min-width: 0;
    word-break: break-word;
  }

  .fm-mono {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text);
  }

  .fm-desc {
    color: var(--color-text-secondary);
    font-size: 12px;
  }

  .fm-meta {
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--color-border);
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
  }

  .fm-chip {
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
  }

  .fm-chip-value {
    color: var(--color-text-secondary);
  }

  .markdown-body {
    font-size: 14px;
    line-height: 1.7;
    color: var(--prose-body);
  }

  .layer-heading {
    margin: 0 0 14px;
    font-size: 16px;
    font-weight: 600;
    color: var(--color-text);
  }

  .markdown-body :global(h1) {
    margin: 0 0 14px;
    padding: 0;
    border: 0;
    font-size: 16px;
    font-weight: 600;
    line-height: 1.35;
    color: var(--color-text);
  }

  .markdown-body :global(h2) {
    margin: 22px 0 10px;
    font-size: 14px;
    font-weight: 600;
    color: var(--color-text);
  }

  .markdown-body :global(h3) {
    margin: 18px 0 8px;
    font-size: 13px;
    font-weight: 600;
    color: var(--color-text);
  }

  .markdown-body :global(hr) {
    display: none;
  }

  .markdown-body :global(p),
  .markdown-body :global(ul),
  .markdown-body :global(ol) {
    margin: 0 0 12px;
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
