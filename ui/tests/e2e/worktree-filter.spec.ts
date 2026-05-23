// User story: 多 worktree group 的 sidebar 顶部 worktree filter chip cluster。
//
// Spec: openspec/specs/sidebar-navigation/spec.md
//   §"Worktree filter chip cluster for multi-worktree group"

import { expect, test } from '@playwright/test'

test.describe('worktree filter chip cluster', () => {
  test('多 worktree group 顶部 SHALL 显示 chip cluster + 默认选中「全部」', async ({ page }) => {
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

    // chip cluster：role="radiogroup" + 多个 role="radio"
    const cluster = page.locator('.worktree-filter-bar [role="radiogroup"]')
    await expect(cluster).toBeVisible({ timeout: 2_000 })
    const chips = cluster.locator('[role="radio"]')
    await expect(chips).toHaveCount(3)

    // 「全部」chip 默认 active（aria-checked=true）
    const allChip = chips.first()
    await expect(allChip).toHaveText('全部')
    await expect(allChip).toHaveAttribute('aria-checked', 'true')
  })

  test('单 worktree group SHALL NOT 显示 chip cluster', async ({ page }) => {
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

  test('点击 chip 切换 worktreeFilter + active 视觉态切换', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    await page.locator('.project-selector').first().click()
    await page
      .locator('.dropdown-item')
      .filter({ hasText: 'rust-port' })
      .first()
      .click()

    const chips = page.locator('.worktree-filter-bar [role="radio"]')
    await expect(chips).toHaveCount(3, { timeout: 5_000 })

    // 点 ⌗feat-x chip
    const featChip = chips.filter({ hasText: '⌗feat-x' }).first()
    await featChip.click()
    await expect(featChip).toHaveAttribute('aria-checked', 'true')
    // 「全部」chip 切回 default 态
    await expect(chips.first()).toHaveAttribute('aria-checked', 'false')
  })

  test('键盘 ArrowRight 在「全部」chip 上切到下一 chip 并触发选中', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    await page.locator('.project-selector').first().click()
    await page
      .locator('.dropdown-item')
      .filter({ hasText: 'rust-port' })
      .first()
      .click()

    const chips = page.locator('.worktree-filter-bar [role="radio"]')
    await expect(chips).toHaveCount(3, { timeout: 5_000 })

    // 焦点落在「全部」chip 后按 ArrowRight
    const allChip = chips.first()
    await allChip.focus()
    await page.keyboard.press('ArrowRight')

    // 第二个 chip（⌗rust-port，isMainWorktree 优先）SHALL 切到 active
    await expect(chips.nth(1)).toHaveAttribute('aria-checked', 'true')
  })
})
