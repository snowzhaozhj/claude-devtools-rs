// User story: sidebar 点击会话项的 replace / new-tab 行为 + Cmd/Ctrl 翻转 + Settings 偏好
// 对应 `tabStore::openSessionTab` / `PaneView::{#key}` / `SettingsView::sessionClickBehavior`

import { expect, test } from '@playwright/test'

async function selectRustProject(page: import('@playwright/test').Page) {
  await page.goto('/?mock=1&fixture=multi-project-rich')
  // 通过 Dashboard 卡片精确选项目（避免误中 sidebar header chip 触发 dropdown）
  await page.locator('.dash-card', { hasText: 'rust-port' }).click()
  // sidebar 出现 fixture 中 rust-port 的 session
  await expect(page.getByText('IPC 字段重构').first()).toBeVisible({ timeout: 5_000 })
}

test.describe('sidebar 点击会话行为', () => {
  test('默认 replace：连续点不同会话 → 仅 1 个 tab，内容随之刷新', async ({ page }) => {
    await selectRustProject(page)

    await page.locator('.session-item', { hasText: 'IPC 字段重构' }).click()
    await expect(page.locator('.tab-item')).toHaveCount(1)
    await expect(page.locator('.tab-item').first()).toContainText('IPC 字段重构')

    await page.locator('.session-item', { hasText: '修复 watcher flake' }).click()
    // 仍只 1 个 tab，但 label 已替换
    await expect(page.locator('.tab-item')).toHaveCount(1)
    await expect(page.locator('.tab-item').first()).toContainText('修复 watcher flake')
  })

  test('Cmd/Ctrl + 点击 翻转默认 → 即使 replace 模式也开新 tab', async ({ page }) => {
    await selectRustProject(page)

    await page.locator('.session-item', { hasText: 'IPC 字段重构' }).click()
    await expect(page.locator('.tab-item')).toHaveCount(1)

    const modifier = process.platform === 'darwin' ? 'Meta' : 'Control'
    await page.locator('.session-item', { hasText: '修复 watcher flake' }).click({
      modifiers: [modifier],
    })
    // 修饰键 → 强制新开
    await expect(page.locator('.tab-item')).toHaveCount(2)
  })

  test('Settings 偏好改 new-tab → 普通点击开新 tab + 再次进 Settings 显示一致', async ({ page }) => {
    await selectRustProject(page)
    await page.locator('.session-item', { hasText: 'IPC 字段重构' }).click()
    await expect(page.locator('.tab-item')).toHaveCount(1)

    // 进 settings（绕过 sidebar 直接走 dev-only test hook，避免 vlist 时序 flake）
    await page.evaluate(() => {
      ;(window as unknown as { __cdtTest: { openSettingsTab: () => void } })
        .__cdtTest.openSettingsTab()
    })
    // 改成 "new-tab"
    const select = page.locator('select.control-select').filter({
      has: page.locator('option[value="new-tab"]'),
    })
    await expect(select).toBeVisible()
    await select.selectOption('new-tab')

    // 关 Settings tab（点 tab-close 按钮）
    await page
      .locator('.tab-item', { hasText: 'Settings' })
      .locator('.tab-close')
      .click()

    // 现在普通点不同会话 SHALL 开新 tab
    await page.locator('.session-item', { hasText: '修复 watcher flake' }).click()
    await expect(page.locator('.tab-item')).toHaveCount(2)

    // 再次进 Settings：select 状态 SHALL 反映持久化偏好
    await page.evaluate(() => {
      ;(window as unknown as { __cdtTest: { openSettingsTab: () => void } })
        .__cdtTest.openSettingsTab()
    })
    const selectAfter = page.locator('select.control-select').filter({
      has: page.locator('option[value="new-tab"]'),
    })
    await expect(selectAfter).toHaveValue('new-tab')
  })
})
