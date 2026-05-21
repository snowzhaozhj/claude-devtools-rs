// User story: 多 worktree group 的 sidebar 顶部 worktree filter 下拉。
//
// Spec: openspec/specs/sidebar-navigation/spec.md
//   §"Worktree filter dropdown for multi-worktree group"

import { expect, test } from '@playwright/test'

test.describe('worktree filter dropdown', () => {
  test('多 worktree group 顶部 SHALL 显示 worktree filter dropdown', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    // 切到含 main + feat-x 的 rust-port group
    await page.locator('.project-selector').first().click()
    await page
      .locator('.dropdown-item')
      .filter({ hasText: 'rust-port' })
      .first()
      .click()

    // sidebar 顶部 worktree-filter-bar 应显示
    await expect(page.locator('.worktree-filter-bar')).toBeVisible({ timeout: 5_000 })

    // 默认选中 "全部 worktree"
    const anchor = page.locator('.worktree-filter-bar .dd-anchor').first()
    await expect(anchor).toContainText(/全部/, { timeout: 2_000 })
  })

  test('单 worktree group SHALL NOT 显示 worktree filter dropdown', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    // claude-devtools 是单 worktree group
    await page.locator('.project-selector').first().click()
    await page
      .locator('.dropdown-item')
      .filter({ hasText: 'claude-devtools' })
      .first()
      .click()

    // 等 sidebar session-list 渲染后再断言 filter 隐藏
    await expect(page.locator('.session-filter-bar')).toBeVisible({ timeout: 5_000 })
    await expect(page.locator('.worktree-filter-bar')).toHaveCount(0)
  })
})
