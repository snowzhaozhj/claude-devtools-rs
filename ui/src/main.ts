import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'

// dev/test 环境注入 mockIPC：URL ?mock=1 强制开启，或浏览器无 Tauri runtime
// 时自动开启。真 cargo tauri dev 窗口由 Tauri 注入 __TAURI_INTERNALS__，
// 完全旁路本逻辑。
//
// URL ?http=1 切到 server-mode UI 调试入口：跳过 mockIPC，让 transport 走
// BrowserTransport fetch `/api/*` → vite proxy → cdt-cli :3456。用于
// chrome-devtools mcp 端到端验证 + 远端 server-mode UI 本地调试。
async function maybeSetupMock(): Promise<void> {
  // production build: import.meta.env.DEV 被 vite 替换成 false，整个 if 块
  // 被 esbuild DCE，连同 dynamic import './lib/tauriMock' 一起从 bundle 剔除。
  if (import.meta.env.DEV) {
    const params = new URLSearchParams(window.location.search)
    if (params.has('http')) return // server-mode 调试：走真后端，不注入 mock
    const forceMock = params.has('mock')
    const noTauriRuntime = !('__TAURI_INTERNALS__' in window)
    if (!forceMock && !noTauriRuntime) return
    const fixtureName = params.get('fixture')
    const { setupMockIPC } = await import('./lib/tauriMock')
    setupMockIPC(fixtureName)
    // dev/test 暴露关键 store 函数到 window，让 Playwright 能直接调
    // 而不用走完整 UI 路径（避免 virtualization / 异步渲染时序导致的 flake）。
    const { openSettingsTab, openNotificationsTab, openMemoryTab, openTab, setActiveTab } =
      await import('./lib/tabStore.svelte')
    Object.assign(window, {
      __cdtTest: {
        openSettingsTab,
        openNotificationsTab,
        openMemoryTab,
        openTab,
        setActiveTab,
      },
    })
  }
}

async function bootstrap(): Promise<void> {
  await maybeSetupMock()
  mount(App, {
    target: document.getElementById('app')!,
  })
}

void bootstrap()
