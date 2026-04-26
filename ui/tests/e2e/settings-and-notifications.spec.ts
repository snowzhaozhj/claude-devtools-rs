// User story #5: Settings tab 三分区 visible + Notifications tab unread badge
//
// TabBar 仅在 pane 有 tab 时显示，因此需先打开任一 session tab 让 TabBar 出现，
// 再点齿轮 / 通知图标。这是仓库实际 UI 路径，对齐用户真实操作。

import { expect, test } from '@playwright/test'

async function gotoWithMockReady(page: import('@playwright/test').Page) {
  await page.goto('/?mock=1&fixture=multi-project-rich')
  // 等 dev-only `__cdtTest` 对象注入完成，才能直接调 store 函数
  await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })
}

test.describe('settings + notifications', () => {
  test('打开 Settings tab → 看到「常规」section + 主题 setting + 通知 sub-tab', async ({ page }) => {
    await gotoWithMockReady(page)
    await page.evaluate(() => {
      ;(window as unknown as { __cdtTest: { openSettingsTab: () => void } }).__cdtTest.openSettingsTab()
    })

    // 设置标题（h2）
    await expect(page.getByRole('heading', { name: '设置' })).toBeVisible({
      timeout: 5_000,
    })

    // 默认 activeSection='general' → 看到「常规」h3 + 主题 setting
    await expect(page.getByRole('heading', { name: '常规', level: 3 })).toBeVisible()
    await expect(page.getByText('主题').first()).toBeVisible()

    // section tab 切换按钮（常规 / 通知）都在
    await expect(page.getByRole('button', { name: '通知' })).toBeVisible()

    // 点切「通知」section → 看到 h3「通知」+「启用通知」label
    await page.getByRole('button', { name: '通知' }).click()
    await expect(page.getByRole('heading', { name: '通知', level: 3 })).toBeVisible()
    await expect(page.getByText('启用通知')).toBeVisible()
  })

  test('打开 Notifications tab → 看到通知列表与 unread 计数', async ({ page }) => {
    await gotoWithMockReady(page)
    await page.evaluate(() => {
      ;(window as unknown as { __cdtTest: { openNotificationsTab: () => void } }).__cdtTest.openNotificationsTab()
    })

    // Notifications view title
    await expect(page.getByRole('heading', { name: '通知' })).toBeVisible({
      timeout: 5_000,
    })

    // fixture 含 1 unread + 1 read = 2 通知，文本应含 "1 条未读"
    await expect(page.getByText(/1 条未读/)).toBeVisible({ timeout: 5_000 })
  })

  test('Settings 触发器分区 - 切到通知 sub-tab 后描述文本可见', async ({ page }) => {
    await gotoWithMockReady(page)
    await page.evaluate(() => {
      ;(window as unknown as { __cdtTest: { openSettingsTab: () => void } }).__cdtTest.openSettingsTab()
    })

    // 触发器分区在「通知」section 下 → 先切过去
    await page.getByRole('button', { name: '通知' }).click()

    await expect(
      page.getByText(/触发器监控会话中的工具错误/),
    ).toBeVisible({ timeout: 5_000 })
  })
})
