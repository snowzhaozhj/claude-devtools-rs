// User story: SidebarHeader dropdown 按 git 仓库聚合渲染，多 worktree group
// 折叠/展开切换，单 worktree group 平铺直选。
//
// Spec：openspec/specs/sidebar-navigation/spec.md
//   §"默认渲染按仓库聚合的 Sidebar"
//   §"活跃 worktree 选中状态"

import { expect, test } from '@playwright/test'

test.describe('sidebar grouped repository view', () => {
  test('单 worktree group 平铺一行，无 chevron / 数量徽章', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    // 用 SidebarHeader 的 project-selector 打开 dropdown（避免命中 dashboard）
    await page.locator('.project-selector').first().click()

    // claude-devtools 是单成员 group → 渲染为 .dropdown-item（含 .dropdown-item-name）
    const tsItem = page
      .locator('.dropdown-item')
      .filter({ hasText: 'claude-devtools' })
      .first()
    await expect(tsItem).toBeVisible({ timeout: 5_000 })
    // 单成员 group 不渲染 .dropdown-group-row（无 chevron + 数量徽章）
    await expect(
      page.locator('.dropdown-group-row').filter({ hasText: 'claude-devtools' }),
    ).toHaveCount(0)
  })

  test('多 worktree group 默认折叠 → 点击展开显示子项 → 点子项切换选中', async ({
    page,
  }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    // 打开 SidebarHeader dropdown
    await page.locator('.project-selector').first().click()

    // rust-port 是含 main + feat-x 双 worktree 的 group → 渲染为 .dropdown-group-row
    const groupRow = page.locator('.dropdown-group-row').filter({ hasText: 'rust-port' })
    await expect(groupRow).toBeVisible({ timeout: 5_000 })
    // 数量徽章应显示 worktree 数量（2）
    await expect(groupRow.locator('.dropdown-group-badge')).toHaveText('2')

    // dropdown 打开时若当前选中 worktree 在该 group 内，自动展开
    // （fixture 启动默认选中 mock-rich-rust 即 main worktree，应自动展开）
    const featItem = page
      .locator('.dropdown-item-worktree')
      .filter({ hasText: 'feat-x' })
      .first()
    await expect(featItem).toBeVisible({ timeout: 2_000 })

    // 点击 feat-x 子项 → dropdown 关闭 + selectedProjectId 切到该 worktree
    await featItem.click()
    // dropdown 关闭后 SidebarHeader 显示名应更新为 "rust-port · feat-x"
    await expect(page.locator('.project-name').first()).toHaveText(/feat-x/, {
      timeout: 5_000,
    })
  })

  test('点击 chevron 折叠/展开切换子项可见性', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    await page.locator('.project-selector').first().click()
    const groupRow = page.locator('.dropdown-group-row').filter({ hasText: 'rust-port' })
    await expect(groupRow).toBeVisible({ timeout: 5_000 })

    // 自动展开后子项可见，点 chevron 折叠
    const featItem = page
      .locator('.dropdown-item-worktree')
      .filter({ hasText: 'feat-x' })
      .first()
    await expect(featItem).toBeVisible({ timeout: 2_000 })

    await groupRow.click()
    await expect(featItem).toBeHidden({ timeout: 2_000 })

    // 再点一次 → 重新展开
    await groupRow.click()
    await expect(featItem).toBeVisible({ timeout: 2_000 })
  })
})
