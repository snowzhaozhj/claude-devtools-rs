import { expect, test } from '@playwright/test'

test.describe('memory viewer', () => {
  test('切到无 memory 的 worktree 时 sidebar memory 入口仍显示 group 维度的 memory', async ({
    page,
  }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    // 切到 rust-port group（含 main + feat-x 双 worktree）
    await page.locator('.dash-row, .dash-card', { hasText: 'rust-port' }).first().click()
    // sidebar 顶部 worktree filter dropdown 切到 feat-x（spec D6）。
    // change `sidebar-memory-anchor-uses-group-root`：memory 入口 anchor SHALL 恒定
    // 指向 group 内 repo 根 worktree（不跟随 worktree filter）。即使 feat-x 自己
    // 没 memory（fixture: count=0），sidebar 入口 SHALL 仍显示 main worktree 的
    // count=3。
    await page.locator('.worktree-filter-bar .dd-anchor').click()
    await page.locator('.dd-popover .dd-opt-label', { hasText: 'feat-x' }).click()

    await expect(page.getByRole('button', { name: /Memory \(3\)/ })).toBeVisible({ timeout: 5_000 })
  })

  test('空 Memory tab 展示空状态并禁用操作', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })

    await page.evaluate(() => {
      ;(
        window as unknown as {
          __cdtTest: { openMemoryTab: (p: string, l?: string) => void }
        }
      ).__cdtTest.openMemoryTab('mock-rich-rust-wt-feat', 'Memory')
    })

    await expect(page.getByText('当前项目没有 Memory。')).toBeVisible()
    await expect(page.getByRole('button', { name: /打开方式/ })).toHaveCount(0)
    await expect(page.getByRole('button', { name: 'Copy' })).toHaveCount(0)
  })

  test('从 Sidebar 打开 Memory tab、切换文件并复制 Markdown 原文', async ({ page, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write'])
    await page.goto('/?mock=1&fixture=multi-project-rich')

    await page.evaluate(() => {
      window.addEventListener(
        '__cdtMockOpenPath',
        (event) => {
          ;(window as unknown as { __lastOpenedPath?: string }).__lastOpenedPath = (event as CustomEvent<string>).detail
        },
        { once: true },
      )
    })
    await page.getByRole('button', { name: /Memory \(3\)/ }).click()

    await expect(page.getByRole('tab', { name: /Memory/ })).toBeVisible({ timeout: 5_000 })
    await expect(page.getByText('feedback_chinese_language.md').first()).toBeVisible()
    await page.getByRole('button', { name: /打开方式/ }).click()
    await page.getByRole('menuitem', { name: '用默认应用打开' }).click()
    await expect(page.evaluate(() => (window as unknown as { __lastOpenedPath?: string }).__lastOpenedPath)).resolves.toContain('/mock/mock-rich-rust/memory/MEMORY.md')

    await page.getByRole('button', { name: /始终使用中文/ }).click()

    await expect(page.getByText('对话、注释、文档和 OpenSpec 产物全部使用简体中文。')).toBeVisible()

    await page.getByRole('button', { name: /MEMORY\.md/ }).click()
    await expect(page.getByTestId('memory-current-file')).toHaveText('MEMORY.md')
    const urlBeforeLinkClick = page.url()
    await page.getByRole('link', { name: '始终使用中文' }).click()

    await expect(page.getByTestId('memory-current-file')).toHaveText('feedback_chinese_language.md')
    expect(page.url()).toBe(urlBeforeLinkClick)

    await page.getByRole('button', { name: 'Copy' }).click()

    await expect(page.getByRole('button', { name: '已复制' })).toBeVisible()
    await expect(page.evaluate(() => navigator.clipboard.readText())).resolves.toContain('# 始终使用中文')
  })
})
