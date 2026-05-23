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

  test('切到 feat-x 后点击 memory 入口打开的是 repo 根 worktree 的 Memory tab', async ({
    page,
  }) => {
    // codex 二审 coverage gap：anchor 选择正确不仅意味着可见性，更意味着点击行为；
    // 应打开 mock-rich-rust（repo 根）的 memory tab，不是 mock-rich-rust-wt-feat。
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })

    await page.locator('.project-selector').first().click()
    await page.locator('.dropdown-item').filter({ hasText: 'rust-port' }).first().click()

    // 切到 feat-x worktree
    await page.locator('.worktree-filter-bar .dd-anchor').first().click()
    await page.locator('.dd-popover .dd-opt').filter({ hasText: 'feat-x' }).first().click()

    // 点击 sidebar memory 入口
    await page.locator('.memory-entry').click()

    // 等 memory tab 渲染稳定
    await expect(page.locator('.memory-layers')).toBeVisible({ timeout: 5_000 })

    // 直接断言 active memory tab 的 projectId（codex round 2 反馈：比间接字符串匹配更稳）
    const activeProjectId = await page.evaluate(() => {
      const cdt = (
        window as unknown as {
          __cdtTest: {
            getPaneLayout: () => {
              focusedPaneId: string
              panes: { id: string; activeTabId: string | null; tabs: { id: string; type: string; projectId: string }[] }[]
            }
          }
        }
      ).__cdtTest
      const layout = cdt.getPaneLayout()
      const focused = layout.panes.find((p) => p.id === layout.focusedPaneId)
      const active = focused?.tabs.find((t) => t.id === focused.activeTabId)
      return active?.type === 'memory' ? active.projectId : null
    })
    expect(activeProjectId).toBe('mock-rich-rust')
  })

  test('切到无 memory 的单 worktree group 时 memory 入口隐藏', async ({ page }) => {
    // codex 二审 coverage gap：group 切换路径——从有 memory 的 rust-port 切到无 memory
    // 的单 worktree group（claude-devtools，fixture 内没显式设 memories 字段，mock IPC
    // fallback 返 count=0 / hasMemory=false）后 memory 入口 SHALL 隐藏，不残留旧 count。
    await page.goto('/?mock=1&fixture=multi-project-rich')

    await page.locator('.project-selector').first().click()
    await page.locator('.dropdown-item').filter({ hasText: 'rust-port' }).first().click()

    // 先看到 rust-port 的 memory 入口
    await expect(page.locator('.memory-entry')).toBeVisible({ timeout: 5_000 })

    // 切到 claude-devtools 单 worktree group（无 memory fixture）
    await page.locator('.project-selector').first().click()
    await page.locator('.dropdown-item').filter({ hasText: 'claude-devtools' }).first().click()

    // 等 sidebar 切换稳定后，memory 入口 SHALL 不再渲染
    await expect(page.locator('.session-filter-bar')).toBeVisible({ timeout: 5_000 })
    await expect(page.locator('.memory-entry')).toHaveCount(0)
  })
})
