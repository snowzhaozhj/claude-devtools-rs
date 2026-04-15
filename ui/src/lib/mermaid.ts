// Mermaid 图表渲染辅助——动态加载 + DOM 后处理

type MermaidAPI = typeof import("mermaid").default;

let mermaidInstance: MermaidAPI | null = null;
let lastTheme: string | null = null;

async function ensureMermaid(): Promise<MermaidAPI> {
  if (!mermaidInstance) {
    const mod = await import("mermaid");
    mermaidInstance = mod.default;
  }

  const isDark = document.documentElement.getAttribute("data-theme") === "dark";
  const theme = isDark ? "dark" : "default";

  if (lastTheme !== theme) {
    mermaidInstance.initialize({
      startOnLoad: false,
      theme,
      securityLevel: "strict",
      fontFamily: "ui-sans-serif, system-ui, sans-serif",
    });
    lastTheme = theme;
  }

  return mermaidInstance;
}

/**
 * 扫描容器内所有 .mermaid-block（未处理的），渲染为 SVG 图表。
 * 在 SessionDetail 的 $effect 中调用。
 */
export async function processMermaidBlocks(container: HTMLElement): Promise<void> {
  const blocks = container.querySelectorAll<HTMLElement>(".mermaid-block:not(.mermaid-done)");
  if (blocks.length === 0) return;

  const mermaid = await ensureMermaid();

  for (const block of blocks) {
    const encoded = block.getAttribute("data-code");
    if (!encoded) continue;

    let code: string;
    try {
      code = decodeURIComponent(escape(atob(encoded)));
    } catch {
      block.classList.add("mermaid-done");
      continue;
    }

    try {
      const id = `mermaid-${crypto.randomUUID().slice(0, 8)}`;
      const { svg } = await mermaid.render(id, code);

      // 保留代码用于切换
      block.setAttribute("data-rendered", "true");
      const svgDiv = document.createElement("div");
      svgDiv.className = "mermaid-svg";
      svgDiv.innerHTML = svg;

      // 添加 Code/Diagram 切换按钮
      const toolbar = document.createElement("div");
      toolbar.className = "mermaid-toolbar";
      toolbar.innerHTML = `<button class="mermaid-toggle-btn" title="切换代码/图表">Code</button>`;

      const pre = block.querySelector("pre");
      let showCode = false;

      toolbar.querySelector("button")!.addEventListener("click", () => {
        showCode = !showCode;
        if (pre) pre.style.display = showCode ? "block" : "none";
        svgDiv.style.display = showCode ? "none" : "block";
        toolbar.querySelector("button")!.textContent = showCode ? "Diagram" : "Code";
      });

      // 默认隐藏代码，显示图表
      if (pre) pre.style.display = "none";

      block.insertBefore(toolbar, block.firstChild);
      block.appendChild(svgDiv);
    } catch (err) {
      // 渲染失败——保留代码视图，添加错误提示
      const errDiv = document.createElement("div");
      errDiv.className = "mermaid-error";
      errDiv.textContent = `Mermaid 渲染失败: ${err instanceof Error ? err.message : String(err)}`;
      block.insertBefore(errDiv, block.firstChild);
    }

    block.classList.add("mermaid-done");
  }
}
