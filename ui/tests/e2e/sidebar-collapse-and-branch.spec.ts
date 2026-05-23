// User story: 折叠/展开 sidebar + 多 wt group 下 worktree 标签在每条 SessionItem 行内
//
// Spec：openspec/specs/sidebar-navigation/spec.md
//   §"侧栏折叠/展开" / §"会话项展示"
//
// 历史：原版 PR #142 在每条 session 行尾显示 gitBranch chip（`.session-branch`）。
// PR-A 视觉重排后 gitBranch 行内不再展示——理由是 gitBranch 与 worktreeName
// 在 git worktree 设计意图下 90%+ 重叠（一 wt 一 branch），重复显示浪费列宽
// 且 branch 名长易截断；多 wt group 改用 `.session-wt-label` 显示 worktreeName
// 短名前缀，行末截断保留前缀（用户对 wt 名记忆主体在前段）。完整 branch 留 SessionDetail。

import { expect, test } from '@playwright/test'

test.describe('sidebar collapse and worktree label', () => {
  test('点折叠按钮 → sidebar 隐藏 → TabBar 展开按钮出现 → 点展开恢复', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    // 选中 rust-port——用 dashboard 行/卡片定位（list 默认 / grid 可选），避免命中 sidebar header
    await page.locator('.dash-row, .dash-card').filter({ hasText: 'rust-port' }).first().click()
    await expect(
      page.getByRole('button', { name: /rust-port/ }).first(),
    ).toBeVisible({ timeout: 5_000 })

    // SidebarHeader 折叠按钮存在
    const collapseBtn = page.getByRole('button', { name: '收起侧栏' }).first()
    await expect(collapseBtn).toBeVisible()

    // 点击折叠
    await collapseBtn.click()

    // sidebar aside 应已不存在
    // sidebar 始终挂载（避免 destroy/recreate 闪烁），collapsed 时通过
     // CSS width:0 + pointer-events:none 隐藏；用 .sidebar-collapsed class
     // 断言折叠态而非 DOM count
    await expect(page.locator('aside.sidebar')).toHaveClass(/sidebar-collapsed/, { timeout: 2_000 })

    // TabBar 最左侧的展开按钮出现
    const expandBtn = page.getByRole('button', { name: '展开侧栏' }).first()
    await expect(expandBtn).toBeVisible({ timeout: 2_000 })

    // 点击展开
    await expandBtn.click()

    // sidebar 恢复
    await expect(page.locator('aside.sidebar')).not.toHaveClass(/sidebar-collapsed/, { timeout: 2_000 })
  })

  test('Cmd+B 快捷键切换折叠/展开', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await page.getByText('rust-port').first().click()
    await expect(page.locator('aside.sidebar')).toHaveCount(1)

    // 按 Meta+B → 折叠
    await page.keyboard.press('Meta+b')
    // sidebar 始终挂载（避免 destroy/recreate 闪烁），collapsed 时通过
     // CSS width:0 + pointer-events:none 隐藏；用 .sidebar-collapsed class
     // 断言折叠态而非 DOM count
    await expect(page.locator('aside.sidebar')).toHaveClass(/sidebar-collapsed/, { timeout: 2_000 })

    // 再按一次 → 展开
    await page.keyboard.press('Meta+b')
    await expect(page.locator('aside.sidebar')).not.toHaveClass(/sidebar-collapsed/, { timeout: 2_000 })
  })

  test('多 wt group 下 worktree 标签在每条 SessionItem 行内显示', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await page.locator('.dash-row, .dash-card').filter({ hasText: 'rust-port' }).first().click()

    // SidebarHeader 不再有 .branch-row（已移到 SessionItem 行内）
    await expect(page.locator('.branch-row')).toHaveCount(0)

    // PR-A 行为：多 wt group（rust-port 含主仓 + feat-x worktree）每条 SessionItem
    // 在 meta 行末尾用 .session-wt-label 显示 ⌗{worktreeName}。
    // fixture 中：mock-rich-rust 的 worktreeName=rust-port，mock-rich-rust-wt-feat 的
    // worktreeName=feat-x——sidebar 应同时出现两个 wt label。
    const wtLabels = page.locator('aside.sidebar .session-wt-label')
    await expect(wtLabels.first()).toContainText(/⌗(rust-port|feat-x)/, { timeout: 5_000 })

    const allWtTexts = await wtLabels.allInnerTexts()
    const uniqueWts = new Set(allWtTexts.map((t) => t.trim()))
    expect(uniqueWts.size).toBeGreaterThanOrEqual(2)
    expect(uniqueWts).toContain('⌗rust-port')
    expect(uniqueWts).toContain('⌗feat-x')

    // gitBranch 行内不再渲染（已沉到 SessionDetail）—— DOM 上不应存在 .session-branch
    await expect(page.locator('aside.sidebar .session-branch')).toHaveCount(0)
  })
})
