export function buildHtmlShell(title: string, bodyContent: string, tocItems: string[]): string {
  const tocHtml = tocItems
    .map((item, i) => `<a class="toc-item" href="#turn-${i + 1}">${escapeHtml(item)}</a>`)
    .join("\n      ");

  return `<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; img-src data:;">
  <title>${escapeHtml(title)}</title>
  <style>${CSS_CONTENT}</style>
</head>
<body>
  <nav class="toc" id="toc">
    <div class="toc-header">目录</div>
    <div class="toc-list">
      ${tocHtml}
    </div>
  </nav>
  <main class="content">
    ${bodyContent}
  </main>
  <button class="theme-toggle" id="theme-toggle" aria-label="切换主题">◐</button>
  <script>${JS_CONTENT}</script>
</body>
</html>`;
}

export function escapeHtml(str: string): string {
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

const CSS_CONTENT = `
:root {
  --bg: #ffffff;
  --bg-secondary: #f8f9fa;
  --bg-code: #f1f3f5;
  --text: #1a1a2e;
  --text-muted: #6c757d;
  --border: #dee2e6;
  --accent: #4a6cf7;
  --link: #4a6cf7;
  --toc-width: 220px;
  --thinking-bg: #fff3cd;
  --tool-bg: #e8f4fd;
}

[data-theme="dark"] {
  --bg: #1a1a2e;
  --bg-secondary: #16213e;
  --bg-code: #0f3460;
  --text: #e0e0e0;
  --text-muted: #8e99a4;
  --border: #2d3748;
  --accent: #6c8cff;
  --link: #6c8cff;
  --thinking-bg: #3d3200;
  --tool-bg: #0d2137;
}

* { box-sizing: border-box; margin: 0; padding: 0; }

body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  background: var(--bg);
  color: var(--text);
  line-height: 1.6;
  display: flex;
  min-height: 100vh;
}

.toc {
  position: fixed;
  top: 0;
  left: 0;
  width: var(--toc-width);
  height: 100vh;
  overflow-y: auto;
  border-right: 1px solid var(--border);
  background: var(--bg-secondary);
  padding: 16px 12px;
  font-size: 13px;
}

.toc-header {
  font-weight: 600;
  margin-bottom: 12px;
  font-size: 14px;
}

.toc-list { display: flex; flex-direction: column; gap: 4px; }

.toc-item {
  display: block;
  padding: 4px 8px;
  border-radius: 4px;
  color: var(--text-muted);
  text-decoration: none;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.toc-item:hover { background: var(--bg-code); color: var(--text); }

.content {
  margin-left: var(--toc-width);
  max-width: 900px;
  padding: 32px 40px;
  width: 100%;
}

.turn {
  margin-bottom: 32px;
  padding-bottom: 24px;
  border-bottom: 1px solid var(--border);
}

.turn-header {
  font-size: 16px;
  font-weight: 600;
  margin-bottom: 12px;
  color: var(--accent);
}

.turn-content { font-size: 14px; }
.turn-content p { margin-bottom: 8px; }

.thinking {
  background: var(--thinking-bg);
  border-radius: 6px;
  padding: 12px 16px;
  margin: 8px 0;
  font-size: 13px;
  color: var(--text-muted);
}

.thinking-header {
  cursor: pointer;
  font-weight: 500;
  user-select: none;
}

.thinking-content { display: none; margin-top: 8px; white-space: pre-wrap; }
.thinking.expanded .thinking-content { display: block; }

.tool-block {
  background: var(--tool-bg);
  border-radius: 6px;
  margin: 12px 0;
  overflow: hidden;
}

.tool-header {
  padding: 8px 16px;
  font-weight: 500;
  font-size: 13px;
  cursor: pointer;
  user-select: none;
  display: flex;
  align-items: center;
  gap: 8px;
}

.tool-header::before { content: "▶"; font-size: 10px; transition: transform 150ms; }
.tool-block.expanded .tool-header::before { transform: rotate(90deg); }

.tool-content { display: none; padding: 0 16px 12px; }
.tool-block.expanded .tool-content { display: block; }

pre {
  background: var(--bg-code);
  border-radius: 4px;
  padding: 12px;
  overflow-x: auto;
  font-size: 13px;
  line-height: 1.5;
  font-family: "SF Mono", "Fira Code", "JetBrains Mono", monospace;
}

code { font-family: inherit; }

.metadata-table {
  width: 100%;
  border-collapse: collapse;
  margin-bottom: 24px;
  font-size: 13px;
}

.metadata-table th,
.metadata-table td {
  padding: 6px 12px;
  border: 1px solid var(--border);
  text-align: left;
}

.metadata-table th { background: var(--bg-secondary); font-weight: 500; }

.subagent {
  border-left: 3px solid var(--accent);
  padding-left: 16px;
  margin: 12px 0;
}

.subagent-header { font-weight: 500; font-size: 13px; color: var(--text-muted); margin-bottom: 8px; }

.theme-toggle {
  position: fixed;
  top: 12px;
  right: 12px;
  width: 36px;
  height: 36px;
  border-radius: 50%;
  border: 1px solid var(--border);
  background: var(--bg-secondary);
  color: var(--text);
  font-size: 18px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}

.theme-toggle:hover { background: var(--bg-code); }

@media (max-width: 768px) {
  .toc { display: none; }
  .content { margin-left: 0; padding: 16px 20px; }
}
`;

const JS_CONTENT = `
(function() {
  var theme = localStorage.getItem('cdt-export-theme') || 'light';
  if (theme === 'dark') document.documentElement.setAttribute('data-theme', 'dark');

  document.getElementById('theme-toggle').addEventListener('click', function() {
    var current = document.documentElement.getAttribute('data-theme');
    var next = current === 'dark' ? 'light' : 'dark';
    if (next === 'dark') {
      document.documentElement.setAttribute('data-theme', 'dark');
    } else {
      document.documentElement.removeAttribute('data-theme');
    }
    localStorage.setItem('cdt-export-theme', next);
  });

  document.querySelectorAll('.tool-header').forEach(function(el) {
    el.addEventListener('click', function() {
      el.parentElement.classList.toggle('expanded');
    });
  });

  document.querySelectorAll('.thinking-header').forEach(function(el) {
    el.addEventListener('click', function() {
      el.parentElement.classList.toggle('expanded');
    });
  });
})();
`;
