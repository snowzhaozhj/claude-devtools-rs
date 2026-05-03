// User story: 折叠/展开 sidebar + git 分支栏跟随 active session 切换
//
// Spec：openspec/specs/sidebar-navigation/spec.md
//   §"侧栏折叠/展开" / §"项目 git 分支只读栏"

import { expect, test } from '@playwright/test'

test.describe('sidebar collapse and git branch row', () => {
  test('点折叠按钮 → sidebar 消失 → TabBar 展开按钮出现 → 点展开恢复', async ({ page }) => {
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
    await expect(page.locator('aside.sidebar')).toHaveCount(0, { timeout: 2_000 })

    // TabBar 最左侧的展开按钮出现
    const expandBtn = page.getByRole('button', { name: '展开侧栏' }).first()
    await expect(expandBtn).toBeVisible({ timeout: 2_000 })

    // 点击展开
    await expandBtn.click()

    // sidebar 恢复
    await expect(page.locator('aside.sidebar')).toHaveCount(1, { timeout: 2_000 })
  })

  test('Cmd+B 快捷键切换折叠/展开', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await page.getByText('rust-port').first().click()
    await expect(page.locator('aside.sidebar')).toHaveCount(1)

    // 按 Meta+B → 折叠
    await page.keyboard.press('Meta+b')
    await expect(page.locator('aside.sidebar')).toHaveCount(0, { timeout: 2_000 })

    // 再按一次 → 展开
    await page.keyboard.press('Meta+b')
    await expect(page.locator('aside.sidebar')).toHaveCount(1, { timeout: 2_000 })
  })

  test('git 分支栏渲染 active session 的 gitBranch', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })
    await page.getByText('rust-port').first().click()

    // 默认无 active：fixture 第一条 session（按 timestamp desc）是 sess-rust-active，
    // gitBranch = 'feat/frontend-test-infrastructure'
    const branchRow = page.locator('.branch-row .branch-name').first()
    await expect(branchRow).toContainText('feat/frontend-test-infrastructure', { timeout: 5_000 })

    // 切到 sess-rust-2（gitBranch=main）
    await page.evaluate(() => {
      ;(
        window as unknown as {
          __cdtTest: { openTab: (s: string, p: string, l: string) => void }
        }
      ).__cdtTest.openTab('sess-rust-2', 'mock-rich-rust', '修复 watcher flake')
    })

    await expect(branchRow).toContainText('main', { timeout: 5_000 })
  })
})
