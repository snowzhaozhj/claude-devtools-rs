// User story: 右键 AI 组内的工具展开块（slash / Output / Thinking / User message）
// SHALL 弹出"该块"的复制菜单，且复制内容为该块自身文本（不冒泡到整条 AI 消息菜单）。
//
// Spec：openspec/specs/session-display/spec.md §"消息 chunk 右键菜单"
//   （Scenario "右键 Output 工具展开块弹该块菜单" / "右键 Thinking / User message 工具展开块弹该块菜单"）
//   openspec/specs/frontend-context-menu/spec.md §"menu-items 函数库"（buildMarkdownBlockItems）
//   （以上 Scenario archive 前位于 change delta，archive 后 sync 进主 spec）
//
// 关键区分点：fixture `sess-rust-active` 的 AI chunk `a-active-1`——
// - user_message 块文本 = "等一下，先看看 IPC 那边有没有 breaking change"
// - 整条 AI 消息 aiChunkToMarkdown（仅 kind="text" 步骤）= "我来帮你检查 LocalDataApi 的字段命名。"
// 二者截然不同，故剪贴板内容 == user_message 块文本即证明：右键落在块上、复制块内容、
// 未冒泡到整条 AI 消息菜单。fixture 该 session 唯一可直接右键的 markdown 块就是 user_message
// 块（仅 1 个 text step → 成 lastOutput 不入 di.items；无 thinking step / 无 slashCommands）。

import { expect, test, type Page } from '@playwright/test'

const USER_BLOCK_TEXT = '等一下，先看看 IPC 那边有没有 breaking change'
const AI_MESSAGE_TEXT = '我来帮你检查 LocalDataApi 的字段命名。'

// 拦截 navigator.clipboard.writeText（避开 e2e clipboard 权限 flake）。必须在 goto 前调。
async function interceptClipboard(page: Page): Promise<void> {
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
}

function readCopied(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as unknown as { __copied: string[] }).__copied)
}

// 打开 sess-rust-active → 展开工具区 → 展开 user_message 块 → 返回其 prose 容器。
async function openUserBlockProse(page: Page) {
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
  await expect(userItem).toBeVisible({ timeout: 5_000 })
  await userItem.locator('.base-item-header').first().click()

  const prose = userItem.locator('.prose.lazy-md').first()
  await expect(prose).toBeVisible({ timeout: 5_000 })
  return prose
}

test.describe('工具展开块右键复制', () => {
  test('右键 User message 块 →「复制为 Markdown」写该块文本而非整条 AI 消息', async ({
    page,
  }) => {
    await interceptClipboard(page)
    const prose = await openUserBlockProse(page)

    await prose.click({ button: 'right' })
    const menu = page.locator('[role="menu"]').first()
    await expect(menu).toBeVisible({ timeout: 2_000 })
    await expect(menu).toContainText('复制为 Markdown')
    await expect(menu).toContainText('复制纯文本')

    await menu.getByText('复制为 Markdown').click()
    await expect
      .poll(() => readCopied(page), { timeout: 2_000 })
      .toContain(USER_BLOCK_TEXT)
    expect(await readCopied(page)).not.toContain(AI_MESSAGE_TEXT)
  })

  test('右键 User message 块 →「复制纯文本」写该块内容、仅 1 个菜单 instance（不冒泡到父 AI 菜单）', async ({
    page,
  }) => {
    await interceptClipboard(page)
    const prose = await openUserBlockProse(page)

    await prose.click({ button: 'right' })
    // 仅 1 个菜单 instance：不会因冒泡叠加成两个菜单
    await expect(page.locator('[role="menu"]')).toHaveCount(1, { timeout: 2_000 })

    // 强断言判别力：点「复制纯文本」验复制内容来自该块。
    // 若右键误弹成父 AI 消息菜单（label 同样含「复制纯文本」、instance 也是 1），
    // 复制内容会是整条消息文本 AI_MESSAGE_TEXT——内容断言能抓到，count 断言抓不到。
    const menu = page.locator('[role="menu"]').first()
    await menu.getByText('复制纯文本').click()
    await expect
      .poll(() => readCopied(page), { timeout: 2_000 })
      .toContain(USER_BLOCK_TEXT)
    expect(await readCopied(page)).not.toContain(AI_MESSAGE_TEXT)
  })
})
