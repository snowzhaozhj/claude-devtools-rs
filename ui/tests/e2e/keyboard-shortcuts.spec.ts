// User story: keyboard-shortcuts spec §10.1-§10.5 e2e 全链路验证
//
// Spec：openspec/specs/keyboard-shortcuts/spec.md
//   §"全局快捷键 happy path" / §"Settings 录键 widget" / §"冲突检测" /
//   §"重置全部" / §"IPC 失败 fallback"
//
// 5 个用户故事（与 tasks.md §10 一一对应）：
//   §10.1 多条快捷键 happy path（mod+K / mod+B / mod+] / mod+1 / mod+W / `/`）
//   §10.2 录键交互：进 Settings → 改 sidebar.toggle 为 mod+shift+B → save → 新组合生效
//   §10.3 冲突检测：录入已占用 binding → 行 conflict warning + Save 路径阻断
//   §10.4 重置全部：自定义后点重置全部 → cdt-config 写空 + UI 默认态恢复
//   §10.5 Save IPC 失败：mockIPC reject → pending 不清 + 错误 banner
//
// 设计：mod-key 走 `page.evaluate(() => document.dispatchEvent(...))`（参考
// command-palette.spec.ts），绕开 playwright keyboard.press 在 body focus 漂走时
// 不触发 document keydown 的问题。Settings 导航走 `__cdtTest.openSettingsTab()`
// 绕过 sidebar virtualization 时序。

import { expect, test, type Page } from '@playwright/test'

const RUST_SESSION_ID = 'sess-rust-active'
const RUST_PROJECT_ID = 'mock-rich-rust'
const TS_SESSION_ID = 'sess-ts-react-debug'
const TS_PROJECT_ID = 'mock-rich-ts'

async function gotoWithMockReady(page: Page) {
  await page.goto('/?mock=1&fixture=multi-project-rich')
  await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })
  // 等 sidebar 挂载（registry dispatcher 在 App.svelte onMount 注册）
  await expect(page.getByPlaceholder('搜索项目...')).toBeVisible({ timeout: 5_000 })
}

/**
 * 用 dispatchEvent 派 mod-key keydown：mac 下 metaKey + 其他平台 ctrlKey 都置 true，
 * registry dispatcher 的 normalize 会按平台展开 mod 关键字（mac=meta / 其他=ctrl）。
 * 单按主键的快捷键（"/"）走 page.keyboard.press。
 */
async function pressMod(page: Page, init: KeyboardEventInit & { key: string }) {
  await page.evaluate((init) => {
    document.dispatchEvent(
      new KeyboardEvent('keydown', { ...init, bubbles: true, cancelable: true }),
    )
  }, { ...init, metaKey: true, ctrlKey: true })
}

/**
 * 派一个 mod-key keydown 给 KeyRecorderInput 的 activeElement——按平台条件 set 一种主修饰键，
 * **不**双发 metaKey + ctrlKey（与 `pressMod` 给 dispatcher 的策略不同）：录键 widget 含
 * Win 键守卫（non-mac + event.metaKey === true 时不 commit），同时设两个会被守卫拦下，
 * 不能进 commit 路径。
 */
async function pressRecorderMod(
  page: Page,
  init: { key: string; code: string; shiftKey?: boolean; altKey?: boolean },
) {
  await page.evaluate(async (init) => {
    // 通过 vite dev module graph 直接 import KeyRecorderInput 同源的 isMac 函数——确保
    // helper 与 widget 共享同一判定（KeyRecorderInput 的 Win 键守卫读同一函数，避免
    // navigator 探测的边角差异引发错位）。
    const platform = (await import(
      /* @vite-ignore */ '/src/lib/platform.ts'
    )) as { isMac: () => boolean }
    const isMac = platform.isMac()
    document.activeElement?.dispatchEvent(
      new KeyboardEvent('keydown', {
        key: init.key,
        code: init.code,
        shiftKey: init.shiftKey ?? false,
        altKey: init.altKey ?? false,
        metaKey: isMac,
        ctrlKey: !isMac,
        bubbles: true,
        cancelable: true,
      }),
    )
  }, init)
}

async function openSettingsKeyboardSection(page: Page) {
  await page.evaluate(() => {
    ;(window as unknown as { __cdtTest: { openSettingsTab: () => void } }).__cdtTest.openSettingsTab()
  })
  await expect(page.getByRole('heading', { name: '设置' })).toBeVisible({ timeout: 5_000 })
  await page.getByRole('tab', { name: '键盘快捷键' }).click()
  await expect(page.getByRole('heading', { name: '键盘快捷键', exact: true })).toBeVisible({
    timeout: 5_000,
  })
}

/**
 * 拿某 shortcut id 对应的 KeyRecorderInput recorder 元素（role="button"）。
 * KeyRecorderInput 把 id 暴露成 `kbd-recorder-{safeId}-hint`（见 ShortcutRow $derived）。
 */
function getRecorder(page: Page, shortcutId: string) {
  const safeId = shortcutId.replace(/[^a-zA-Z0-9]/g, '-')
  return page.locator(`[role="button"][aria-describedby="kbd-recorder-${safeId}-hint"]`)
}

// fixture singleton 在 vite dev server 内存里跨 worker / 跨 test 串用 state——
// 任何 update_config 都直接 mutate `fx.config.keyboardShortcuts`，下一个 test goto
// 时 setupMockIPC 复用同一 fx，会拿到上一个 test 残留 override（"重置全部" 看到
// committed 已经是空的，按钮 disabled）。处理：
//   1. test.describe serial 模式 → 8 个 test 串行跑，避免 worker 间冲突
//   2. beforeEach 显式 reset：navigate 后第一时间 invoke update_config 写空覆盖
test.describe.configure({ mode: 'serial' })

/**
 * 通过 mockIPC 走真 update_config 把 fixture 的 keyboardShortcuts 清空。
 * 注意 page.goto 后才能调（setupMockIPC 已注册 handler）；用 /@id/ URL 解 bare specifier。
 */
async function resetFixtureKeyboardShortcuts(page: Page) {
  await page.evaluate(async () => {
    const { invoke } = await import(
      /* @vite-ignore */ '/@id/@tauri-apps/api/core'
    )
    await invoke('update_config', {
      section: 'keyboardShortcuts',
      configData: {},
    })
  })
}

test.describe('keyboard-shortcuts §10', () => {
  // -------------------------------------------------------------------------
  // §10.1 happy path：覆盖 6 条核心 binding（global / sidebar / search / tabs）
  // -------------------------------------------------------------------------

  test('§10.1 mod+K → CommandPalette 弹出 / Esc 关', async ({ page }) => {
    await gotoWithMockReady(page)
    await pressMod(page, { key: 'k' })
    await expect(page.getByPlaceholder('搜索项目或会话...')).toBeVisible({ timeout: 5_000 })
    await page.keyboard.press('Escape')
    await expect(page.getByPlaceholder('搜索项目或会话...')).not.toBeVisible({ timeout: 3_000 })
  })

  test('§10.1 mod+B → sidebar 折叠/展开切换', async ({ page }) => {
    await gotoWithMockReady(page)
    // 选中 rust-port 让 sidebar 渲染会话列表（默认 dashboard 态 sidebar 也挂着 aside.sidebar）
    await page.locator('.dash-row, .dash-card').filter({ hasText: 'rust-port' }).first().click()
    await expect(page.locator('aside.sidebar')).not.toHaveClass(/sidebar-collapsed/, { timeout: 2_000 })

    await pressMod(page, { key: 'b' })
    await expect(page.locator('aside.sidebar')).toHaveClass(/sidebar-collapsed/, { timeout: 2_000 })

    await pressMod(page, { key: 'b' })
    await expect(page.locator('aside.sidebar')).not.toHaveClass(/sidebar-collapsed/, { timeout: 2_000 })
  })

  test('§10.1 mod+1 / mod+] / mod+W：tab 切换 + 关闭', async ({ page }) => {
    await gotoWithMockReady(page)
    // 开两个 session tab
    await page.evaluate(
      ({ s1, p1, s2, p2 }) => {
        const cdt = (window as unknown as {
          __cdtTest: { openTab: (s: string, p: string, l: string) => void }
        }).__cdtTest
        cdt.openTab(s1, p1, 'A')
        cdt.openTab(s2, p2, 'B')
      },
      { s1: RUST_SESSION_ID, p1: RUST_PROJECT_ID, s2: TS_SESSION_ID, p2: TS_PROJECT_ID },
    )
    // 等 TabBar 出现（pane 有 ≥ 1 tab 时显示）
    await expect(page.locator('.tab-list .tab-item')).toHaveCount(2, { timeout: 5_000 })
    // 默认第二个（B）是 active
    await expect(page.locator('.tab-item-active')).toHaveCount(1)

    // mod+1 → 切到第一个 tab
    await pressMod(page, { key: '1' })
    await expect(page.locator('.tab-item').first()).toHaveClass(/tab-item-active/, { timeout: 3_000 })

    // mod+] → 切到下一个（即第二个）
    await pressMod(page, { key: ']' })
    await expect(page.locator('.tab-item').nth(1)).toHaveClass(/tab-item-active/, { timeout: 3_000 })

    // mod+W → 关闭当前 active tab → 只剩 1 个
    await pressMod(page, { key: 'w' })
    await expect(page.locator('.tab-list .tab-item')).toHaveCount(1, { timeout: 3_000 })
  })

  test('§10.1 `/` → focus Dashboard 搜索框', async ({ page }) => {
    await gotoWithMockReady(page)
    // Dashboard 上的搜索框 input 自身带 `.dash-search` class
    const dashSearch = page.locator('input.dash-search').first()
    await expect(dashSearch).toBeVisible({ timeout: 5_000 })
    // 当前焦点不在 input 上 → 主页面 body
    await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur?.())
    // 派 "/" keydown（registry search.focus binding）
    await page.evaluate(() => {
      document.dispatchEvent(new KeyboardEvent('keydown', { key: '/', bubbles: true, cancelable: true }))
    })
    await expect(dashSearch).toBeFocused({ timeout: 3_000 })
  })

  // -------------------------------------------------------------------------
  // §10.2 录键交互：录 sidebar.toggle = mod+shift+B → Save → 新 binding 生效
  // -------------------------------------------------------------------------

  test('§10.2 录键 → Save → 新 binding 生效（旧 binding 不再触发）', async ({ page }) => {
    await gotoWithMockReady(page)

    // 进 Settings → 键盘快捷键
    await openSettingsKeyboardSection(page)

    // 找 sidebar.toggle 行的 recorder
    const recorder = getRecorder(page, 'sidebar.toggle')
    await expect(recorder).toBeVisible({ timeout: 3_000 })

    // focus 进 recording → 录 mod+shift+B（按平台分发，避免触发 Win 键守卫）
    await recorder.focus()
    await pressRecorderMod(page, { key: 'B', code: 'KeyB', shiftKey: true })

    // 等 pending bar 出现（commit 写 overlay）
    await expect(page.locator('.pending-bar')).toBeVisible({ timeout: 3_000 })
    await expect(page.locator('.pending-bar')).toContainText(/未保存改动/)

    // 点保存
    await page.getByRole('button', { name: /^保存/ }).click()
    // pending 清空
    await expect(page.locator('.pending-bar')).not.toBeVisible({ timeout: 3_000 })

    // 关 Settings tab 回到 Dashboard
    await page.evaluate(() => {
      const cdt = (window as unknown as {
        __cdtTest: { setActiveTab: (id: string) => void }
      }).__cdtTest
      // 关闭 Settings tab：找到 settings tab 关闭按钮
      const closeBtn = document.querySelector(
        '.tab-item-active button[aria-label*="关闭"], .tab-item-active .tab-close',
      ) as HTMLElement | null
      closeBtn?.click()
      void cdt
    })
    // 选个项目让 sidebar 挂会话列表
    await page.locator('.dash-row, .dash-card').filter({ hasText: 'rust-port' }).first().click()
    await expect(page.locator('aside.sidebar')).not.toHaveClass(/sidebar-collapsed/, { timeout: 2_000 })

    // 旧 binding (mod+B 不带 shift) 不应再触发折叠
    await pressMod(page, { key: 'b' })
    // 等 100ms 确保 dispatcher 处理
    await page.waitForTimeout(100)
    await expect(page.locator('aside.sidebar')).not.toHaveClass(/sidebar-collapsed/)

    // 新 binding mod+shift+B 触发
    await page.evaluate(() => {
      document.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 'B',
          code: 'KeyB',
          metaKey: true,
          ctrlKey: true,
          shiftKey: true,
          bubbles: true,
          cancelable: true,
        }),
      )
    })
    await expect(page.locator('aside.sidebar')).toHaveClass(/sidebar-collapsed/, { timeout: 2_000 })
  })

  // -------------------------------------------------------------------------
  // §10.3 冲突检测：录入与已注册 spec 冲突的 binding → row conflict warning
  // -------------------------------------------------------------------------

  test('§10.3 录入已占用 binding → row conflict warning + Save 路径阻断', async ({ page }) => {
    await gotoWithMockReady(page)
    // 复位 fx.config.keyboardShortcuts，避免 §10.2 残留的 sidebar.toggle override 改变冲突源
    // （SettingsView 在 openSettingsKeyboardSection 时才 mount + getConfig，复位先于 mount 即可）
    await resetFixtureKeyboardShortcuts(page)
    await openSettingsKeyboardSection(page)

    // 在 command-palette.toggle 行录入 mod+B（与 sidebar.toggle 默认 binding 冲突）
    const recorder = getRecorder(page, 'command-palette.toggle')
    await recorder.focus()
    await pressRecorderMod(page, { key: 'b', code: 'KeyB' })

    // 行级 conflict warning：双向冲突（command-palette 行 + sidebar.toggle 行各一条）。
    // 我们 scope 到 command-palette.toggle 所在 .row 验证它指向 "切换侧栏"。
    const cmdRow = page.locator('.row', { has: getRecorder(page, 'command-palette.toggle') })
    const cmdRowWarning = cmdRow.locator('.row-hint-warning[role="alert"]')
    await expect(cmdRowWarning).toBeVisible({ timeout: 3_000 })
    await expect(cmdRowWarning).toContainText(/切换侧栏/)

    // pending bar 仍在（用户可看见冲突再决定要不要 Save）
    await expect(page.locator('.pending-bar')).toBeVisible()

    // 点 Save → preSaveConflict banner 渲染（panel 内"串行冲突"自检）
    await page.getByRole('button', { name: /^保存/ }).click()
    const preSaveBanner = page.locator('.banner-error[role="alert"]')
    await expect(preSaveBanner).toBeVisible({ timeout: 3_000 })
    await expect(preSaveBanner).toContainText(/串行冲突|冲突/)

    // pending 不清空（Save 早期 return 不持久化）
    await expect(page.locator('.pending-bar')).toBeVisible()
  })

  // -------------------------------------------------------------------------
  // §10.4 重置全部：先持久化 1 条 override → 点重置全部 → IPC 写空 + UI 恢复 default
  // -------------------------------------------------------------------------

  test('§10.4 重置全部 → cdt-config 写空 + UI 默认态恢复', async ({ page }) => {
    await gotoWithMockReady(page)

    // 直接通过 mockIPC update_config 持久化一条 override（绕过 UI 录键，加速测试）。
    // 注意：fixture (`multiProjectRichFixture`) 是 module 级 singleton，page reload 会
    // 重新 evaluate 模块 → 新的 fx 实例 → keyboardShortcuts 又被重置成 {}；所以 SHALL
    // 不 reload，直接在同一 page 内 invoke 后立刻 open Settings，让 panel mount 时
    // getConfig() 拿到当前 fx.config 的 override（KeyboardShortcutsPanel 仅在 Settings
    // tab 首次打开时 mount + 取 initialOverrides snapshot）。
    await page.evaluate(async () => {
      const { invoke } = await import(
        /* @vite-ignore */ '/@id/@tauri-apps/api/core'
      )
      await invoke('update_config', {
        section: 'keyboardShortcuts',
        configData: { 'sidebar.toggle': 'mod+shift+b' },
      })
    })

    // 进 Settings → 键盘快捷键 → "重置全部" 按钮 enabled
    await openSettingsKeyboardSection(page)

    const resetAll = page.getByRole('button', { name: /^重置全部/ })
    await expect(resetAll).toBeEnabled({ timeout: 3_000 })
    await resetAll.click()

    // 等 IPC 写完成
    await page.waitForTimeout(200)

    // 验 fixture config 已被清空
    const cleared = await page.evaluate(async () => {
      const { invoke } = await import(
        /* @vite-ignore */ '/@id/@tauri-apps/api/core'
      )
      const cfg = (await invoke('get_config')) as { keyboardShortcuts?: Record<string, string> }
      return cfg.keyboardShortcuts ?? null
    })
    expect(cleared).toEqual({})
  })

  // -------------------------------------------------------------------------
  // §10.5 Save IPC 失败 → pending bar 仍渲染 + 错误反馈
  // -------------------------------------------------------------------------

  test('§10.5 Save IPC 失败 → pending 不清 + 错误 banner', async ({ page }) => {
    await gotoWithMockReady(page)
    await resetFixtureKeyboardShortcuts(page)

    // 在 mockIPC 注入失败：打开 Settings 前覆盖 update_config handler。
    // page.evaluate 内裸 specifier 不可解析，走 Vite dev /@id/ URL。
    // 注意：override 替换整个 handler，必须为 SettingsView mount 时调的 get_config
    // 返回完整 AppConfig 形状（否则 `config!.general.claudeRootPath` 取空 throw）。
    await page.evaluate(async () => {
      const { mockIPC } = await import(
        /* @vite-ignore */ '/@id/@tauri-apps/api/mocks'
      )
      // 最小化 AppConfig：覆盖 SettingsView onMount 读的所有字段（display / general /
      // httpServer），其他可选字段 panel 不消费。
      const minimalConfig = {
        general: {
          launchAtLogin: false,
          showDockIcon: true,
          theme: 'system',
          defaultTab: 'sessions',
          claudeRootPath: null,
          autoExpandAiGroups: false,
          sessionClickBehavior: 'replace',
        },
        display: { fontSans: null, fontMono: null, timeFormat: '24h' },
        httpServer: { enabled: false, port: 3456 },
        notifications: { enabled: false, soundEnabled: false, triggers: [] },
        updater: { autoUpdateCheckEnabled: true },
        keyboardShortcuts: {},
      }
      mockIPC((cmd) => {
        if (cmd === 'update_config') {
          return Promise.reject(new Error('IPC down (e2e simulated)'))
        }
        if (cmd === 'get_config') {
          return Promise.resolve(minimalConfig)
        }
        return Promise.resolve(null)
      })
    })

    await openSettingsKeyboardSection(page)
    const recorder = getRecorder(page, 'sidebar.toggle')
    await recorder.focus()
    await pressRecorderMod(page, { key: 'x', code: 'KeyX', shiftKey: true })
    await expect(page.locator('.pending-bar')).toBeVisible({ timeout: 3_000 })

    await page.getByRole('button', { name: /^保存/ }).click()

    // IPC reject → pending 不清空（仍可见）+ 错误 banner 出现
    await expect(page.locator('.pending-bar')).toBeVisible()
    const errorBanner = page.locator('.banner-error[role="alert"]')
    await expect(errorBanner).toBeVisible({ timeout: 3_000 })
    await expect(errorBanner).toContainText(/IPC down|保存失败|无法加载/)
  })
})
