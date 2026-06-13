// User story: 右键 AI 组内的工具展开块（slash / Output / Thinking / User message）
// SHALL 弹出"该块"的复制菜单，且复制内容为该块自身文本（不冒泡到整条 AI 消息菜单）。
//
// Spec：openspec/specs/session-display/spec.md §"消息 chunk 右键菜单"
//   （Scenario "右键 Output 工具展开块弹该块菜单" / "右键 Thinking / User message 工具展开块弹该块菜单"）
//   openspec/specs/frontend-context-menu/spec.md §"menu-items 函数库"（buildMarkdownBlockItems）
//
// 关键区分点：fixture `sess-rust-active` 的 AI chunk `a-active-1`——
// - user_message 块文本 = "等一下，先看看 IPC 那边有没有 breaking change"
// - 整条 AI 消息 aiChunkToMarkdown（仅 kind="text" 步骤）= "我来帮你检查 LocalDataApi 的字段命名。"
// 二者截然不同，故剪贴板内容 == user_message 块文本即证明：右键落在块上、复制块内容、
// 未冒泡到整条 AI 消息菜单。

import { expect, test } from '@playwright/test'

const USER_BLOCK_TEXT = '等一下，先看看 IPC 那边有没有 breaking change'
const AI_MESSAGE_TEXT = '我来帮你检查 LocalDataApi 的字段命名。'

test.describe('工具展开块右键复制', () => {
  test('右键 User message 块 → 复制该块文本而非整条 AI 消息', async ({ page }) => {
    // 拦截 navigator.clipboard.writeText（避开 e2e clipboard 权限 flake）
    await page.addInitScript(() => {
      ;(window as unknown as { __copied: string[] }).__copied = []
      Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        value: {
          writeText: (t: string) => {
            ;(window as unknown as { __copied: string[] }).__copied.push(t)
            return Promise.resolve()
          },
        },
      })
    })

    await page.goto('/?mock=1&fixture=multi-project-rich')
    await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })
    await page.evaluate(() => {
      ;(
        window as unknown as {
          __cdtTest: { openTab: (s: string, p: string, l: string) => void }
        }
      ).__cdtTest.openTab('sess-rust-active', 'mock-rich-rust', 'IPC 字段重构')
    })

    // 展开 AI 组工具调用列表（默认折叠）
    const toolsToggle = page.locator('button[aria-label="展开工具调用列表"]').first()
    await expect(toolsToggle).toBeVisible({ timeout: 5_000 })
    await toolsToggle.click()

    // 定位含该 user_message 文本的 BaseItem 并展开
    const userItem = page
      .locator('.base-item')
      .filter({ hasText: USER_BLOCK_TEXT })
      .first()
    await expect(userItem).toBeVisible({ timeout: 5_000 })
    await userItem.locator('.base-item-header').first().click()

    // 展开后该块的 prose 容器出现（lazy 占位仍有高度，可承载右键）
    const prose = userItem.locator('.prose.lazy-md').first()
    await expect(prose).toBeVisible({ timeout: 5_000 })

    // 右键 prose → 弹该块菜单
    await prose.click({ button: 'right' })
    const menu = page.locator('[role="menu"]').first()
    await expect(menu).toBeVisible({ timeout: 2_000 })
    await expect(menu).toContainText('复制为 Markdown')
    await expect(menu).toContainText('复制纯文本')

    // 点"复制为 Markdown"，验剪贴板写入的是该块文本（非整条 AI 消息）
    await menu.getByText('复制为 Markdown').click()
    await expect
      .poll(
        () =>
          page.evaluate(
            () => (window as unknown as { __copied: string[] }).__copied,
          ),
        { timeout: 2_000 },
      )
      .toContain(USER_BLOCK_TEXT)

    const copied = await page.evaluate(
      () => (window as unknown as { __copied: string[] }).__copied,
    )
    expect(copied).not.toContain(AI_MESSAGE_TEXT)
  })

  test('右键 User message 块菜单不冒泡到整条 AI 消息菜单', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })
    await page.evaluate(() => {
      ;(
        window as unknown as {
          __cdtTest: { openTab: (s: string, p: string, l: string) => void }
        }
      ).__cdtTest.openTab('sess-rust-active', 'mock-rich-rust', 'IPC 字段重构')
    })

    const toolsToggle = page.locator('button[aria-label="展开工具调用列表"]').first()
    await expect(toolsToggle).toBeVisible({ timeout: 5_000 })
    await toolsToggle.click()

    const userItem = page
      .locator('.base-item')
      .filter({ hasText: USER_BLOCK_TEXT })
      .first()
    await userItem.locator('.base-item-header').first().click()
    const prose = userItem.locator('.prose.lazy-md').first()
    await expect(prose).toBeVisible({ timeout: 5_000 })

    await prose.click({ button: 'right' })
    // 同一刻仅一个菜单 instance（块菜单），不会因冒泡叠加
    await expect(page.locator('[role="menu"]')).toHaveCount(1, { timeout: 2_000 })
  })
})
