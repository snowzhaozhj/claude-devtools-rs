// User story：用户用 Cmd+F 打开会话内搜索框后点击别处使其失焦，再次按 Cmd+F
// SHALL 重新聚焦搜索框并全选已有文本。
// spec.md::Requirement Cmd+F 激活会话内搜索 → Scenario「重复按 Cmd+F」。
//
// 回归保护：原 bug 是 openSearch 仅 `searchVisible = true`，在已 visible 时是
// Svelte 5 相等性 no-op，不触发 SearchBar 内仅依赖 `visible` 的 focus $effect →
// 失焦后再按 Cmd+F 无反应。修法引入单调递增的 focusRequestVersion prop 强制
// effect 重跑（SessionDetail.svelte / SearchBar.svelte）。

import { expect, test, type Page } from '@playwright/test'

const SESSION_ID = 'sess-rust-active'
const PROJECT_ID = 'mock-rich-rust'

/**
 * 派 platform-aware mod-key keydown。playwright `keyboard.press('Meta+f')` 在
 * body focus 漂走时事件不冒泡到 document（registry dispatcher listen 点），故走
 * evaluate 内 document.dispatchEvent + bubbles:true。与 session-jump-to-latest.spec.ts
 * 同款 helper。metaKey/ctrlKey 都置 true 由 registry normalize 展开。
 */
async function pressMod(page: Page, key: string) {
  await page.evaluate((k) => {
    document.dispatchEvent(
      new KeyboardEvent('keydown', {
        key: k,
        metaKey: true,
        ctrlKey: true,
        bubbles: true,
        cancelable: true,
      }),
    )
  }, key)
}

async function openSession(page: Page) {
  await page.goto('/?mock=1&fixture=multi-project-rich')
  await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })
  await page.evaluate(
    ({ sid, pid }) => {
      ;(
        window as unknown as {
          __cdtTest: { openTab: (s: string, p: string, l: string) => void }
        }
      ).__cdtTest.openTab(sid, pid, 'IPC 字段重构')
    },
    { sid: SESSION_ID, pid: PROJECT_ID },
  )
  await page.locator('.conversation').waitFor({ state: 'visible', timeout: 5_000 })
}

test.describe('会话内搜索：重复 Cmd+F 重新聚焦', () => {
  test('首次 Cmd+F 打开并聚焦搜索框（无 regression）', async ({ page }) => {
    await openSession(page)
    const searchInput = page.locator('.search-bar input').first()

    await pressMod(page, 'f')
    await searchInput.waitFor({ state: 'visible', timeout: 2_000 })
    await expect(searchInput).toBeFocused()
  })

  test('失焦后再次 Cmd+F：重新聚焦并全选已有文本', async ({ page }) => {
    await openSession(page)
    const searchInput = page.locator('.search-bar input').first()

    // 第一次 Cmd+F 打开 + 聚焦
    await pressMod(page, 'f')
    await searchInput.waitFor({ state: 'visible', timeout: 2_000 })
    await expect(searchInput).toBeFocused()

    // 输入查询文本，便于验证再次聚焦时 select() 全选
    await searchInput.fill('error')

    // 点击 conversation 别处 → 输入框失焦；SearchBar 无 click-outside 关闭逻辑，
    // 搜索框 SHALL 仍可见（这是触发 bug 的前置状态）
    await page.locator('.conversation').click({ force: true, position: { x: 100, y: 100 } })
    await expect(searchInput).not.toBeFocused()
    await expect(searchInput).toBeVisible()

    // 再次 Cmd+F → 重新聚焦（回归点：修复前此处不会重新 focus，input 保持失焦）
    await pressMod(page, 'f')
    await expect(searchInput).toBeFocused({ timeout: 2_000 })

    // 全选断言：selectionStart=0、selectionEnd=value.length（spec「select 全部文本」）
    const sel = await searchInput.evaluate((el: HTMLInputElement) => ({
      start: el.selectionStart,
      end: el.selectionEnd,
      len: el.value.length,
    }))
    expect(sel.len).toBeGreaterThan(0)
    expect(sel.start).toBe(0)
    expect(sel.end).toBe(sel.len)
  })
})
