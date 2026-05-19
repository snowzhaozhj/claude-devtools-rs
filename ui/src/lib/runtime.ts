// runtime 检测：当前 UI 是跑在 Tauri webview 里还是浏览器里。
//
// 详见 openspec/specs/server-mode/spec.md::Requirement: 前端 SHALL 在浏览器
// runtime 切换到 HTTP/SSE transport。本 PR 仅暴露 isTauriRuntime() 给
// Settings View 隐藏 Browser Access section；transport 抽象层（HTTP/SSE
// fallback）在后续 PR 内补全。
//
// ⚠️ 单测覆盖：vitest 用 mockIPC 注入 `__TAURI_INTERNALS__` 后本函数返回
// `true`（与 Tauri runtime 行为一致），让 settings UI 测试能走 Tauri
// 分支验证 Browser Access section 渲染。

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
