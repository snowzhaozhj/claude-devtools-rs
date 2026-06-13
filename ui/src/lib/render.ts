import { marked } from "marked";
import hljs from "highlight.js/lib/core";
import json from "highlight.js/lib/languages/json";
import bash from "highlight.js/lib/languages/bash";
import typescript from "highlight.js/lib/languages/typescript";
import javascript from "highlight.js/lib/languages/javascript";
import rust from "highlight.js/lib/languages/rust";
import python from "highlight.js/lib/languages/python";
import markdown from "highlight.js/lib/languages/markdown";
import yaml from "highlight.js/lib/languages/yaml";
import xml from "highlight.js/lib/languages/xml";
import css from "highlight.js/lib/languages/css";
import go from "highlight.js/lib/languages/go";
import java from "highlight.js/lib/languages/java";
import kotlin from "highlight.js/lib/languages/kotlin";
import c from "highlight.js/lib/languages/c";
import cpp from "highlight.js/lib/languages/cpp";
import csharp from "highlight.js/lib/languages/csharp";
import swift from "highlight.js/lib/languages/swift";
import ruby from "highlight.js/lib/languages/ruby";
import php from "highlight.js/lib/languages/php";
import lua from "highlight.js/lib/languages/lua";
import dart from "highlight.js/lib/languages/dart";
import scala from "highlight.js/lib/languages/scala";
import sql from "highlight.js/lib/languages/sql";
import dockerfile from "highlight.js/lib/languages/dockerfile";
import makefile from "highlight.js/lib/languages/makefile";
import perl from "highlight.js/lib/languages/perl";
import r from "highlight.js/lib/languages/r";
import powershell from "highlight.js/lib/languages/powershell";
import ini from "highlight.js/lib/languages/ini";
import protobuf from "highlight.js/lib/languages/protobuf";
import graphql from "highlight.js/lib/languages/graphql";
import http from "highlight.js/lib/languages/http";
import diff from "highlight.js/lib/languages/diff";
import properties from "highlight.js/lib/languages/properties";
import nginx from "highlight.js/lib/languages/nginx";
import plaintext from "highlight.js/lib/languages/plaintext";
import objectivec from "highlight.js/lib/languages/objectivec";
import less from "highlight.js/lib/languages/less";
import elixir from "highlight.js/lib/languages/elixir";
import erlang from "highlight.js/lib/languages/erlang";
import haskell from "highlight.js/lib/languages/haskell";
import ocaml from "highlight.js/lib/languages/ocaml";
import fsharp from "highlight.js/lib/languages/fsharp";
import vbnet from "highlight.js/lib/languages/vbnet";
import julia from "highlight.js/lib/languages/julia";
import nim from "highlight.js/lib/languages/nim";
import nix from "highlight.js/lib/languages/nix";
import coffeescript from "highlight.js/lib/languages/coffeescript";
import groovy from "highlight.js/lib/languages/groovy";
import gradle from "highlight.js/lib/languages/gradle";
import cmake from "highlight.js/lib/languages/cmake";
import latex from "highlight.js/lib/languages/latex";
import DOMPurify from "dompurify";

hljs.registerLanguage("json", json);
hljs.registerLanguage("bash", bash);
hljs.registerLanguage("typescript", typescript);
hljs.registerLanguage("javascript", javascript);
hljs.registerLanguage("rust", rust);
hljs.registerLanguage("python", python);
hljs.registerLanguage("markdown", markdown);
hljs.registerLanguage("yaml", yaml);
hljs.registerLanguage("xml", xml);
hljs.registerLanguage("html", xml);
hljs.registerLanguage("css", css);
hljs.registerLanguage("scss", css);
hljs.registerLanguage("go", go);
hljs.registerLanguage("java", java);
hljs.registerLanguage("kotlin", kotlin);
hljs.registerLanguage("c", c);
hljs.registerLanguage("cpp", cpp);
hljs.registerLanguage("csharp", csharp);
hljs.registerLanguage("swift", swift);
hljs.registerLanguage("ruby", ruby);
hljs.registerLanguage("php", php);
hljs.registerLanguage("lua", lua);
hljs.registerLanguage("dart", dart);
hljs.registerLanguage("scala", scala);
hljs.registerLanguage("sql", sql);
hljs.registerLanguage("dockerfile", dockerfile);
hljs.registerLanguage("makefile", makefile);
hljs.registerLanguage("perl", perl);
hljs.registerLanguage("r", r);
hljs.registerLanguage("powershell", powershell);
hljs.registerLanguage("ini", ini);
// toml 语法与 ini 高度兼容（hljs 官方按 alias 处理）
hljs.registerLanguage("toml", ini);
hljs.registerLanguage("protobuf", protobuf);
hljs.registerLanguage("graphql", graphql);
hljs.registerLanguage("http", http);
hljs.registerLanguage("diff", diff);
hljs.registerLanguage("properties", properties);
hljs.registerLanguage("nginx", nginx);
hljs.registerLanguage("plaintext", plaintext);
hljs.registerLanguage("text", plaintext);
hljs.registerLanguage("objectivec", objectivec);
hljs.registerLanguage("less", less);
hljs.registerLanguage("elixir", elixir);
hljs.registerLanguage("erlang", erlang);
hljs.registerLanguage("haskell", haskell);
hljs.registerLanguage("ocaml", ocaml);
hljs.registerLanguage("fsharp", fsharp);
hljs.registerLanguage("vbnet", vbnet);
hljs.registerLanguage("julia", julia);
hljs.registerLanguage("nim", nim);
hljs.registerLanguage("nix", nix);
hljs.registerLanguage("coffeescript", coffeescript);
hljs.registerLanguage("groovy", groovy);
hljs.registerLanguage("gradle", gradle);
hljs.registerLanguage("cmake", cmake);
hljs.registerLanguage("latex", latex);

const COPY_ICON_SVG = `<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>`;
const CHECK_ICON_SVG = `<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>`;

const renderer = new marked.Renderer();
renderer.code = function ({ text, lang }: { text: string; lang?: string }) {
  if (lang === "mermaid") {
    const encoded = btoa(unescape(encodeURIComponent(text)));
    return `<div class="mermaid-block" data-code="${encoded}"><pre><code class="hljs">${DOMPurify.sanitize(text)}</code></pre></div>`;
  }
  const language = lang && hljs.getLanguage(lang) ? lang : undefined;
  const highlighted = language ? hljs.highlight(text, { language }).value : escapeHtml(text);
  const encoded = btoa(unescape(encodeURIComponent(text)));
  const langLabel = language ? `<span class="code-block-lang">${escapeHtml(language)}</span>` : "";
  return `<div class="code-block-wrapper"><div class="code-block-actions">${langLabel}<button type="button" class="code-block-copy" data-code="${encoded}" title="Copy code">${COPY_ICON_SVG}</button></div><pre><code class="hljs">${highlighted}</code></pre></div>`;
};

const _copyTimeouts = new WeakMap<Element, ReturnType<typeof setTimeout>>();
if (typeof document !== "undefined") {
  document.addEventListener("click", (e) => {
    const btn = (e.target as HTMLElement).closest(".code-block-copy") as HTMLElement | null;
    if (!btn?.dataset.code) return;
    e.stopPropagation();
    const raw = decodeURIComponent(escape(atob(btn.dataset.code)));
    navigator.clipboard.writeText(raw).then(() => {
      const prev = _copyTimeouts.get(btn);
      if (prev !== undefined) clearTimeout(prev);
      btn.classList.add("copied");
      btn.innerHTML = CHECK_ICON_SVG;
      _copyTimeouts.set(btn, setTimeout(() => {
        btn.classList.remove("copied");
        btn.innerHTML = COPY_ICON_SVG;
        _copyTimeouts.delete(btn);
      }, 2000));
    }, () => {});
  });
}

renderer.table = function (token) {
  let header = "";
  for (const cell of token.header) header += this.tablecell(cell);
  const thead = this.tablerow({ text: header });

  let rows = "";
  for (const row of token.rows) {
    let rowHtml = "";
    for (const cell of row) rowHtml += this.tablecell(cell);
    rows += this.tablerow({ text: rowHtml });
  }
  const tbody = rows ? `<tbody>${rows}</tbody>` : "";
  return `<div class="table-scroll"><table>\n<thead>\n${thead}</thead>\n${tbody}</table>\n</div>`;
};

marked.setOptions({ renderer, async: false, breaks: true });

class LRU<V> {
  private map = new Map<string, V>();
  constructor(private capacity: number) {}
  get(key: string): V | undefined {
    const v = this.map.get(key);
    if (v === undefined) return undefined;
    this.map.delete(key);
    this.map.set(key, v);
    return v;
  }
  set(key: string, value: V): void {
    if (this.map.has(key)) {
      this.map.delete(key);
    } else if (this.map.size >= this.capacity) {
      const first = this.map.keys().next().value;
      if (first !== undefined) this.map.delete(first);
    }
    this.map.set(key, value);
  }
}

const highlightCache = new LRU<string>(4096);
const markdownCache = new LRU<string>(256);

// 超出此长度的输入不缓存（OutputBlock 可传整段 tool output；按 entry 数限制
// LRU 会让单 entry 把内存撑爆）。约等于 32 KB。
const HIGHLIGHT_CACHE_MAX_LEN = 32 * 1024;
const MARKDOWN_CACHE_MAX_LEN = 64 * 1024;

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

export function renderMarkdown(text: string): string {
  const cacheable = text.length <= MARKDOWN_CACHE_MAX_LEN;
  if (cacheable) {
    const cached = markdownCache.get(text);
    if (cached !== undefined) return cached;
  }
  const raw = marked.parse(text) as string;
  const sanitized = DOMPurify.sanitize(raw);
  if (cacheable) markdownCache.set(text, sanitized);
  return sanitized;
}

export function highlightCode(code: string, lang: string = "json"): string {
  // text / plaintext / 未注册语言：直接 escape，避免 hljs.highlight 跑无意义的纯文本规则
  // 或对未知扩展跑 highlightAuto 逐行检测（多行 Read tool 输出会浪费 CPU）。
  if (lang === "text" || lang === "plaintext" || !hljs.getLanguage(lang)) {
    return escapeHtml(code);
  }
  const cacheable = code.length <= HIGHLIGHT_CACHE_MAX_LEN;
  if (cacheable) {
    const key = `${lang}\0${code}`;
    const cached = highlightCache.get(key);
    if (cached !== undefined) return cached;
    const sanitized = DOMPurify.sanitize(hljs.highlight(code, { language: lang }).value);
    highlightCache.set(key, sanitized);
    return sanitized;
  }
  return DOMPurify.sanitize(hljs.highlight(code, { language: lang }).value);
}
