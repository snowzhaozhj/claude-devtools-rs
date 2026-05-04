// User story: 折叠/展开 sidebar + git 分支 chip 在每条 SessionItem 行内
//
// Spec：openspec/specs/sidebar-navigation/spec.md
//   §"侧栏折叠/展开" / §"会话项展示"（含 git 分支 chip）

import { expect, test } from '@playwright/test'

test.describe('sidebar collapse and git branch chip', () => {
  test('点折叠按钮 → sidebar 隐藏 → TabBar 展开按钮出现 → 点展开恢复', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    // 选中 rust-port——用 dashboard 卡片定位，避免命中 sidebar header 触发 dropdown
    await page.locator('.dash-card').filter({ hasText: 'rust-port' }).click()
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

  test('git 分支 chip 在每条 SessionItem 行内显示', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await page.locator('.dash-card').filter({ hasText: 'rust-port' }).click()

    // SidebarHeader 不再有 .branch-row（已移到 SessionItem 行内）
    await expect(page.locator('.branch-row')).toHaveCount(0)

    // fixture 中：sess-rust-active gitBranch=feat/frontend-test-infrastructure，
    // sess-rust-2 / sess-rust-3 gitBranch=main——每条 SessionItem 第二行 meta
    // 末尾应有对应 .session-branch chip。
    const branchNames = page.locator('aside.sidebar .session-branch-name')
    await expect(branchNames.first()).toContainText(
      /feat\/frontend-test-infrastructure|main/,
      { timeout: 5_000 },
    )

    // 至少能看到不同的两个 branch（rust-port 项目下 active 在 feat 分支，
    // 其他在 main）
    const allBranchTexts = await branchNames.allInnerTexts()
    const uniqueBranches = new Set(allBranchTexts.map((t) => t.trim()))
    expect(uniqueBranches.size).toBeGreaterThanOrEqual(2)
    expect(uniqueBranches).toContain('feat/frontend-test-infrastructure')
    expect(uniqueBranches).toContain('main')
  })
})
