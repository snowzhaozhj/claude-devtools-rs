// Spec: frontend-test-pyramid §"Playwright 必须覆盖最小 user story 集"
// User story #1: 启动应用 → 看到 Sidebar header + Dashboard 项目卡片网格

import { expect, test } from '@playwright/test'

test.describe('startup + dashboard', () => {
  test('启动后看到 sidebar 项目选择 + Dashboard 项目网格', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    // Sidebar header 默认显示「选择项目」（无 selected project 时）
    await expect(
      page.getByRole('button', { name: /选择项目|rust-port|claude-devtools/ }).first(),
    ).toBeVisible()

    // Dashboard 标题（搜索 placeholder + 「最近项目」section）
    await expect(page.getByPlaceholder('搜索项目...')).toBeVisible()
    await expect(page.getByText('最近项目')).toBeVisible()

    // Fixture 5 个项目应至少看到 1 个项目名（Dashboard 卡片渲染）
    await expect(page.getByText('rust-port').first()).toBeVisible()

    // 项目计数（fixture 5 个项目）
    await expect(page.getByText(/5 个项目/)).toBeVisible()

    await expect(page).toHaveScreenshot('startup-dashboard.png')
  })
})
