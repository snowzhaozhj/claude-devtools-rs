// User story: 全应用右键菜单系统（capability frontend-context-menu）。
//
// 覆盖 Phase 1 三态决策：
// - 挂 use:contextMenu 的元素（sidebar 会话项 / TabBar 标签）→ 弹 app 自家菜单
//   ([role="menu"] 出现在 document.body 末尾）
// - 输入控件（input / textarea / contenteditable / data-allow-native-context）→
//   全局兜底**不**调 preventDefault，浏览器系统菜单可弹（这里只能验证 event 状态）
// - 其它任何位置（空白 / 滚动条 / 普通文本）→ 全局兜底 preventDefault，无 app 菜单
//
// Spec：openspec/specs/frontend-context-menu/spec.md
//   §"全局右键事件兜底策略"
//   §"AppContextMenu 通用浮层组件"
//   §"AppContextMenu 浮层 portal 到 document.body"

import { expect, test, type Page } from '@playwright/test'

async function dispatchContextMenu(
  page: Page,
  selector: string,
  index = 0,
): Promise<{ defaultPrevented: boolean }> {
  return page.evaluate(
    ({ sel, idx }) => {
      const el = document.querySelectorAll(sel)[idx] as HTMLElement | undefined
      if (!el) throw new Error(`no element matches selector: ${sel}[${idx}]`)
      const rect = el.getBoundingClientRect()
      const e = new MouseEvent('contextmenu', {
        bubbles: true,
        cancelable: true,
        clientX: rect.left + rect.width / 2,
        clientY: rect.top + rect.height / 2,
      })
      el.dispatchEvent(e)
      return { defaultPrevented: e.defaultPrevented }
    },
    { sel: selector, idx: index },
  )
}

test.describe('frontend-context-menu Phase 1', () => {
  test('Sidebar 会话项右键 → app 菜单 portal 到 body', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await page.getByText('rust-port').first().click()
    // ProjectSwitcher dropdown 收起后 sidebar 才能稳定可点
    await page.keyboard.press('Escape')
    await expect(page.locator('.session-item').first()).toBeVisible({
      timeout: 5_000,
    })

    // 直接派发 contextmenu，避免 ProjectSwitcher dropdown 遗留拦截 pointer events
    await dispatchContextMenu(page, '.session-item', 0)

    // 菜单 portal 到 body，[role="menu"] 应可见
    const menu = page.locator('[role="menu"]').first()
    await expect(menu).toBeVisible({ timeout: 2_000 })

    // 菜单不在 sidebar 内（DOM 结构验证 portal）
    const isInSidebar = await menu.evaluate((el) => !!el.closest('.sidebar'))
    expect(isInSidebar).toBe(false)

    // 菜单包含会话项菜单文案
    await expect(menu).toContainText('在新标签页打开')
    await expect(menu).toContainText('复制 Session ID')
  })

  test('Esc 关闭菜单', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await page.getByText('rust-port').first().click()
    await page.keyboard.press('Escape')
    await expect(page.locator('.session-item').first()).toBeVisible({
      timeout: 5_000,
    })
    await dispatchContextMenu(page, '.session-item', 0)
    const menu = page.locator('[role="menu"]').first()
    await expect(menu).toBeVisible({ timeout: 2_000 })

    await page.keyboard.press('Escape')
    await expect(menu).not.toBeVisible({ timeout: 1_000 })
  })

  test('外点关闭菜单', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await page.getByText('rust-port').first().click()
    await page.keyboard.press('Escape')
    await expect(page.locator('.session-item').first()).toBeVisible({
      timeout: 5_000,
    })
    await dispatchContextMenu(page, '.session-item', 0)
    const menu = page.locator('[role="menu"]').first()
    await expect(menu).toBeVisible({ timeout: 2_000 })

    // 直接 dispatchEvent mousedown 到 body，绕过 Playwright 视觉 click
    await page.evaluate(() => {
      document.body.dispatchEvent(
        new MouseEvent('mousedown', { bubbles: true, button: 0 }),
      )
    })
    await expect(menu).not.toBeVisible({ timeout: 1_000 })
  })

  test('全局兜底：空白区右键 preventDefault（不漏到系统菜单）', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await expect(page.locator('.app-layout')).toBeVisible({ timeout: 5_000 })
    // 在 dashboard 主背景上派发 contextmenu（无 use:contextMenu 元素）
    const result = await dispatchContextMenu(page, '.app-layout')
    expect(result.defaultPrevented).toBe(true)
  })

  test('全局兜底：input 元素放行（保留系统输入菜单）', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    // dashboard 顶部搜索框是 input[type="search"]
    const search = page.locator('.dash-search').first()
    await expect(search).toBeVisible({ timeout: 5_000 })
    const result = await page.evaluate(() => {
      const el = document.querySelector('.dash-search') as HTMLElement | null
      if (!el) throw new Error('no search input')
      const e = new MouseEvent('contextmenu', { bubbles: true, cancelable: true })
      el.dispatchEvent(e)
      return { defaultPrevented: e.defaultPrevented }
    })
    expect(result.defaultPrevented).toBe(false)
  })

  test('多个右键：仅保留 1 个菜单 instance', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await page.getByText('rust-port').first().click()
    await page.keyboard.press('Escape')
    const items = page.locator('.session-item')
    await expect(items.first()).toBeVisible({ timeout: 5_000 })

    await dispatchContextMenu(page, '.session-item', 0)
    await expect(page.locator('[role="menu"]')).toHaveCount(1)
    if ((await items.count()) > 1) {
      await dispatchContextMenu(page, '.session-item', 1)
      await expect(page.locator('[role="menu"]')).toHaveCount(1)
    }
  })
})
