// User story: 切到非 repo-root worktree 时 sidebar 顶部 memory 入口 SHALL 仍显示。
//
// 历史 bug：anchorWorktreeId 跟随 worktreeFilter，切到具体 worktree 后
// 后端按 worktree id 查 `~/.claude/projects/<wt-id>/memory/`，绝大多数 worktree
// 的 encoded project_dir 下根本没有 memory 目录 → `count=0` → memory 入口消失。
//
// 修复：memoryAnchorWorktreeId 恒定指向 group repo 根 worktree，不跟 filter 漂。
// pin/hide 仍走 anchorWorktreeId（per-worktree 有意义）。

import { expect, test } from '@playwright/test'

test.describe('sidebar memory entry vs worktree filter', () => {
  test('切到非 repo-root worktree 时 memory 入口 SHALL 仍显示且数量不变', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    // 切到含 main + feat-x 两个 worktree 的 rust-port group
    await page.locator('.project-selector').first().click()
    await page
      .locator('.dropdown-item')
      .filter({ hasText: 'rust-port' })
      .first()
      .click()

    // 默认 anchor = repo 根 mock-rich-rust（fixture 里 count=3 hasMemory=true）
    const memoryEntry = page.locator('.memory-entry')
    await expect(memoryEntry).toBeVisible({ timeout: 5_000 })
    await expect(memoryEntry).toContainText(/Memory \(3\)/)

    // 切 worktree filter 到 feat-x（fixture 里 mock-rich-rust-wt-feat 自己 count=0 / hasMemory=false）
    await page.locator('.worktree-filter-bar .dd-anchor').first().click()
    await page
      .locator('.dd-popover .dd-opt')
      .filter({ hasText: 'feat-x' })
      .first()
      .click()

    // 修复后：memory anchor 不跟 filter 漂，仍读 repo 根 count=3
    // 修复前：anchor 跟到 feat-x → count=0 → 入口消失
    await expect(memoryEntry).toBeVisible({ timeout: 5_000 })
    await expect(memoryEntry).toContainText(/Memory \(3\)/)
  })

  test('切回"全部 worktree"后 memory 入口仍显示', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    await page.locator('.project-selector').first().click()
    await page
      .locator('.dropdown-item')
      .filter({ hasText: 'rust-port' })
      .first()
      .click()

    const memoryEntry = page.locator('.memory-entry')
    await expect(memoryEntry).toBeVisible({ timeout: 5_000 })

    // 切到 feat-x 再切回全部 → 入口始终显示
    const filterAnchor = page.locator('.worktree-filter-bar .dd-anchor').first()
    await filterAnchor.click()
    await page.locator('.dd-popover .dd-opt').filter({ hasText: 'feat-x' }).first().click()
    await expect(memoryEntry).toBeVisible()

    await filterAnchor.click()
    await page.locator('.dd-popover .dd-opt').filter({ hasText: '全部' }).first().click()
    await expect(memoryEntry).toBeVisible()
    await expect(memoryEntry).toContainText(/Memory \(3\)/)
  })
})
