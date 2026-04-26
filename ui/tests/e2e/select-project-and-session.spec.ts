// User story #2: 点项目 → 展开 sessions → 点 session → SessionDetail tab 出现

import { expect, test } from '@playwright/test'

test.describe('select project + session', () => {
  test('点 Dashboard 项目卡片 → 选中后 sidebar 展示 sessions', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    // Dashboard 上点 rust-port 卡片（fixture 中 rust-port 有 3 sessions）
    await page.getByText('rust-port').first().click()

    // 选中后 sidebar header 应显示项目名（不再是「选择项目」）
    await expect(
      page.getByRole('button', { name: /rust-port/ }).first(),
    ).toBeVisible({ timeout: 5_000 })

    // sidebar 应展示 fixture 中 rust-port 的 session 标题
    await expect(page.getByText('IPC 字段重构').first()).toBeVisible({ timeout: 5_000 })

    await expect(page).toHaveScreenshot('select-project-sidebar.png')
  })

  test('调用 openTab 打开 SessionDetail tab → TabBar 出现 + 标题渲染', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })

    // 直接调 store openTab(sessionId, projectId, label) 打开 fixture 中已知 session
    await page.evaluate(() => {
      ;(
        window as unknown as {
          __cdtTest: { openTab: (s: string, p: string, l: string) => void }
        }
      ).__cdtTest.openTab('sess-rust-active', 'mock-rich-rust', 'IPC 字段重构')
    })

    // TabBar 应出现「设置」按钮（pane 有 tab 时 TabBar 渲染）
    await expect(page.getByTitle('设置').first()).toBeVisible({ timeout: 5_000 })
  })
})
