import { expect, test } from '@playwright/test'

test.describe('memory viewer', () => {
  test('无 memory 的项目不显示 Sidebar Memory 入口', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    await page.getByRole('button', { name: 'rust-port' }).first().click()
    await page.getByRole('button', { name: /feat-x/ }).click()

    await expect(page.getByRole('button', { name: /Memory \(/ })).toHaveCount(0)
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
    await expect(page.getByRole('button', { name: 'Open' })).toHaveCount(0)
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
    await expect(page.getByRole('heading', { name: 'Index' })).toBeVisible()
    await expect(page.getByText('feedback_chinese_language.md').first()).toBeVisible()
    await page.getByRole('button', { name: 'Open' }).click()
    await expect(page.evaluate(() => (window as unknown as { __lastOpenedPath?: string }).__lastOpenedPath)).resolves.toContain('/mock/mock-rich-rust/memory/MEMORY.md')

    await page.getByRole('button', { name: /始终使用中文/ }).click()

    await expect(page.getByText('对话、注释、文档和 OpenSpec 产物全部使用简体中文。')).toBeVisible()

    await page.getByLabel('选择 Memory 文件').selectOption('MEMORY.md')
    const urlBeforeLinkClick = page.url()
    await page.getByRole('link', { name: '始终使用中文' }).click()

    await expect(page.getByLabel('选择 Memory 文件')).toHaveValue('feedback_chinese_language.md')
    expect(page.url()).toBe(urlBeforeLinkClick)

    await page.getByRole('button', { name: 'Copy' }).click()

    await expect(page.getByRole('button', { name: '已复制' })).toBeVisible()
    await expect(page.evaluate(() => navigator.clipboard.readText())).resolves.toContain('# 始终使用中文')
  })
})
