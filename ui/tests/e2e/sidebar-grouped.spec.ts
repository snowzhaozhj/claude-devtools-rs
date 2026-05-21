// User story: ProjectSwitcher dropdown 把 RepositoryGroup 渲染为**单行**
// 入口——多 worktree 与单 worktree group 同分支，不再有 accordion / chevron /
// worktree count 徽章；worktree 维度的切换下沉到 sidebar 顶部 worktree filter。
//
// Spec：openspec/specs/sidebar-navigation/spec.md
//   §"默认渲染按仓库聚合的 Sidebar"（含 Scenario "不再渲染 accordion"）
//   §"项目选择"（含 Scenario "多 worktree group 单行展示无 accordion"）
//   §"活跃 worktree 选中状态"

import { expect, test } from '@playwright/test'

test.describe('sidebar grouped repository view (post-simplify)', () => {
  test('单 worktree group 渲染单行 .dropdown-item', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    // 用 ProjectSwitcher 的 project-selector 打开 dropdown（避免命中 dashboard）
    await page.locator('.project-selector').first().click()

    // claude-devtools 是单 worktree group → 渲染为 .dropdown-item
    const tsItem = page
      .locator('.dropdown-item')
      .filter({ hasText: 'claude-devtools' })
      .first()
    await expect(tsItem).toBeVisible({ timeout: 5_000 })
    // 总会话数应作为 .dropdown-item-count 渲染
    await expect(tsItem.locator('.dropdown-item-count')).toBeVisible()
  })

  test('多 worktree group 也渲染单行 .dropdown-item（不再 accordion）', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    await page.locator('.project-selector').first().click()

    // rust-port 含 main + feat-x 两个 worktree → spec 要求单行（无 chevron / badge）
    const rustItem = page
      .locator('.dropdown-item')
      .filter({ hasText: 'rust-port' })
      .first()
    await expect(rustItem).toBeVisible({ timeout: 5_000 })

    // spec Scenario "不再渲染 accordion"：dropdown 内 SHALL NOT 含 accordion 残留
    await expect(page.locator('.dropdown-group-row')).toHaveCount(0)
    await expect(page.locator('.dropdown-group-chevron')).toHaveCount(0)
    await expect(page.locator('.dropdown-group-badge')).toHaveCount(0)
    await expect(page.locator('.dropdown-item-worktree')).toHaveCount(0)
  })

  test('点击多 worktree group 行 → 切到该 group（保留默认选中 worktree）', async ({
    page,
  }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    await page.locator('.project-selector').first().click()

    const rustItem = page
      .locator('.dropdown-item')
      .filter({ hasText: 'rust-port' })
      .first()
    await expect(rustItem).toBeVisible({ timeout: 5_000 })

    await rustItem.click()
    // dropdown 关闭后 ProjectSwitcher 顶部应反映 group 名
    await expect(page.locator('.project-name').first()).toHaveText(/rust-port/, {
      timeout: 5_000,
    })
  })
})
