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

const renderer = new marked.Renderer();
renderer.code = function ({ text, lang }: { text: string; lang?: string }) {
  // Mermaid 代码块：输出占位 div，由 processMermaidBlocks 后处理
  if (lang === "mermaid") {
    const encoded = btoa(unescape(encodeURIComponent(text)));
    return `<div class="mermaid-block" data-code="${encoded}"><pre><code class="hljs">${DOMPurify.sanitize(text)}</code></pre></div>`;
  }
  const language = lang && hljs.getLanguage(lang) ? lang : undefined;
  const highlighted = language
    ? hljs.highlight(text, { language }).value
    : hljs.highlightAuto(text).value;
  return `<pre><code class="hljs">${highlighted}</code></pre>`;
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
  // 未注册或 "text" 等不支持的语言：直接 escape，避免 hljs.highlightAuto
  // 逐行做语言检测（多行 Read tool 输出 + 文本/未知扩展时 CPU 浪费）。
  if (!hljs.getLanguage(lang)) {
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
