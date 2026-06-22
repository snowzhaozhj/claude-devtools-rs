import { expect, test } from '@playwright/test'

// change cmdk-global-session-locate：Cmd+K 全局按 Session ID 跨项目定位。
// 用户故事：当前选中 rust-port，粘一个属于 claude-devtools 项目的 sessionId
// 片段 → 会话区跨项目浮出该会话（显示项目名）→ 打开正确会话。

test.describe('command palette global session-id locate', () => {
  test('locates a session from another project by id and opens it', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })

    // 打开命令面板（点 ⌘K 入口按钮，跨平台稳，避开 Meta/Ctrl 键位差异）
    await page.getByRole('button', { name: /跨项目搜索会话/ }).click()
    const input = page.getByRole('searchbox', { name: '命令面板搜索' })
    await expect(input).toBeVisible()

    // 输入跨项目 sessionId 片段（sess-ts-* 属于 claude-devtools，而非当前选中项目）
    await input.fill('sess-ts')

    // 会话区跨项目浮出该会话，且行内带项目名 claude-devtools
    const row = page
      .getByRole('button', { name: /sess-ts-.*claude-devtools/ })
      .first()
    await expect(row).toBeVisible({ timeout: 5_000 })

    // 打开 → 命令面板关闭（选择生效），未报错
    await row.click()
    await expect(input).toBeHidden({ timeout: 5_000 })
  })
})
