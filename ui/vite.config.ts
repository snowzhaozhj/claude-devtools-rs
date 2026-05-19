import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

// https://vite.dev/config/
export default defineConfig({
  plugins: [svelte()],
  // src-tauri/tauri.conf.json::devUrl 写死 http://localhost:5173；vite 默认 bind
  // wildcard / IPv6 `[::1]`，当外部进程占住 IPv4 `127.0.0.1:5173` 时 vite 仍能
  // 在 `[::1]:5173` 起来 "ready" 但 macOS WKWebView 解析 `localhost` 优先 IPv4
  // → 拿到外部进程的响应（或 connection refused）→ Tauri 窗口白屏，且没有可
  // 见错误。`host: 127.0.0.1` 强制同一地址族，`strictPort` 让端口冲突时立即
  // 报错暴露问题，而不是默默 fallback 到 5174/5175 让 Tauri webview 拿不到内容。
  server: {
    host: '127.0.0.1',
    port: 5173,
    strictPort: true,
  },
})
