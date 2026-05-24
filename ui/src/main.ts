import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import { installGlobalContextMenuFallback } from './lib/contextMenu.svelte'
import { installSelectionContextMenu } from './lib/contextMenu/selectionMenu'
import { getMenuSettings } from './lib/contextMenu/settings.svelte'
import { getMenuItemDispatch } from './lib/contextMenu/dispatch'
import type { MenuItemContext } from './lib/contextMenu/menu-items'
import { installDeeplinkWatcher } from './lib/deeplink'
import { setPendingScrollChunkIdForSession, getActiveTab } from './lib/tabStore.svelte'

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

    // dev/test 暴露关键 store 函数到 window，让 Playwright / chrome-devtools mcp
    // 能直接调而不用走完整 UI 路径（避免 virtualization / 异步渲染时序导致的 flake）。
    // 任何 dev 入口（?http=1 真后端 / ?mock=1 / Tauri dev runtime）都注入——
    // 历史上只在 mockIPC 分支注入，导致 e2e-http-verify skill 推荐的 ?http=1 入口
    // 拿不到 helper 只能靠 sidebar click + virtualization 文本模糊匹配，flake 高。
    const {
      openSettingsTab,
      openNotificationsTab,
      openMemoryTab,
      openTab,
      setActiveTab,
      getPaneLayout,
    } = await import('./lib/tabStore.svelte')
    Object.assign(window, {
      __cdtTest: {
        openSettingsTab,
        openNotificationsTab,
        openMemoryTab,
        openTab,
        setActiveTab,
        getPaneLayout,
      },
    })

    if (params.has('http')) return // server-mode 调试：走真后端，不注入 mock
    const forceMock = params.has('mock')
    const noTauriRuntime = !('__TAURI_INTERNALS__' in window)
    if (!forceMock && !noTauriRuntime) return
    const fixtureName = params.get('fixture')
    const { setupMockIPC, simulateNotificationAdded, resetSimulatedNotifications } =
      await import('./lib/tauriMock')
    setupMockIPC(fixtureName)
    // mock-only e2e helper：让 Playwright 能在不依赖真 Tauri runtime / 真后端的
    // 情况下精确触发 `notification-added` push event 路径（issue #258 验收用）。
    // 仅在已注入 mockIPC 的 mock 分支暴露——`?http=1` 真后端 + 真 Tauri runtime
    // 都不会走到这里，自然不污染。
    Object.assign((window as unknown as { __cdtTest: Record<string, unknown> }).__cdtTest, {
      simulateNotificationAdded,
      resetSimulatedNotifications,
    })
  }
}

async function bootstrap(): Promise<void> {
  // ---- 三层级联 contextmenu 注册（spec frontend-context-menu::文本选区菜单）----
  //
  // 注册顺序硬约束（design.md::D10）：
  //   Layer 2 selection menu  先  ─┐
  //   Layer 3 global fallback 后  ─┘
  // bubble 阶段同一 phase 内，listener 按注册顺序执行——Layer 2 先跑才能在 Layer 3
  // preventDefault 之前判断选区。Layer 1（surface `use:contextMenu`）由各组件局部
  // 挂载，stopPropagation 拦截后事件不冒泡到 window，二三层不触发。

  installSelectionContextMenu(() => {
    // ctxProvider lazy 闭包：每次右键时调用，让 settings 改动 / dispatch 单例
    // 即时生效。selectionText 由 selectionMenu listener 自身覆写，这里给空串。
    const activeTab = getActiveTab()
    const ctx: MenuItemContext = {
      sessionId: activeTab?.type === 'session' ? activeTab.sessionId : '',
      projectId: activeTab?.type === 'session' ? activeTab.projectId : '',
      settings: getMenuSettings(),
      selectionText: '',
      dispatch: getMenuItemDispatch(),
    }
    return ctx
  })

  installGlobalContextMenuFallback()

  // Deeplink hash route watcher（spec session-display::pendingScrollChunkId）：
  // 解析 `#/session/<sid>/chunk/<cid>` → openSessionTab + 把 chunkId 写入对应
  // tab 的 pendingScrollChunkId。SessionDetail.onMount 加载 chunks 后消费一次。
  // 找不到 sessionId 对应的已开 tab 时 openSessionTab 会新开（projectId 留空——
  // 真 deeplink 场景下 sessionId 已唯一定位 session，但本仓 IPC 设计下 projectId
  // 是必传字段。Phase 2 仅在 sessionId 已被 cdt-cli 暴露的 list_sessions 收录时
  // 工作；外部跨 app deeplink 留 follow-up 用 Tauri deep-link plugin 注册 cdt://
  // protocol 时再补 sessionId → projectId 反查 IPC）。
  installDeeplinkWatcher((target) => {
    // 仅在 sessionId 已对应已开 tab 时设置 pendingScrollChunkId（spec 设计：
    // 用户复制 deeplink → 粘贴回 App 时 session 通常已在某个 tab 中打开）。
    // 找不到 tab 时 setPendingScrollChunkIdForSession 静默 no-op。
    //
    // Follow-up：若需支持"app 启动后从外部 URL 直接打开未在 tab 中的 session"，
    // 需新增 sessionId → projectId 反查 IPC（或 Tauri deep-link plugin 注册
    // cdt:// custom protocol），届时这里追加 openSessionTab(sessionId, projectId, label)。
    setPendingScrollChunkIdForSession(target.sessionId, target.chunkId)
  })

  await maybeSetupMock()
  mount(App, {
    target: document.getElementById('app')!,
  })
}

void bootstrap()
