// User story #3: ControlOrMeta+K 调出 CommandPalette → 输入 → ↑↓ 导航 → ↵ 选中

import { expect, test } from '@playwright/test'

test.describe('command palette', () => {
  test('Cmd/Ctrl+K 弹出 CommandPalette 与键位提示', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    // 等 mockIPC 注入完成 + Sidebar 挂载（keydown handler 才注册）
    await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })
    await expect(page.getByPlaceholder('搜索项目...')).toBeVisible({ timeout: 5_000 })

    // Cmd/Ctrl+K：用 dispatchEvent 模拟，绕开浏览器 focus 不在 body 时
    // keyboard.press 不触发 document keydown 的问题
    await page.evaluate(() => {
      const ev = new KeyboardEvent('keydown', {
        key: 'k',
        metaKey: true,
        ctrlKey: true,
        bubbles: true,
      })
      document.dispatchEvent(ev)
    })

    // CommandPalette 搜索框 placeholder
    await expect(page.getByPlaceholder('搜索项目或会话...')).toBeVisible({ timeout: 5_000 })

    // 键位提示文本
    await expect(page.getByText('↑↓')).toBeVisible()
    await expect(page.getByText('esc')).toBeVisible()
  })

  test('CommandPalette 输入项目名能搜到', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    // Cmd/Ctrl+K：用 dispatchEvent 模拟，绕开浏览器 focus 不在 body 时
    // keyboard.press 不触发 document keydown 的问题
    await page.evaluate(() => {
      const ev = new KeyboardEvent('keydown', {
        key: 'k',
        metaKey: true,
        ctrlKey: true,
        bubbles: true,
      })
      document.dispatchEvent(ev)
    })
    const input = page.getByPlaceholder('搜索项目或会话...')
    await expect(input).toBeVisible()

    await input.fill('rust')

    // 应能在结果中看到 rust-port
    await expect(page.getByText('rust-port').first()).toBeVisible({ timeout: 3_000 })
  })

  test('Esc 关闭 CommandPalette', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })
    await expect(page.getByPlaceholder('搜索项目...')).toBeVisible({ timeout: 5_000 })

    // Cmd/Ctrl+K：用 dispatchEvent 模拟
    await page.evaluate(() => {
      const ev = new KeyboardEvent('keydown', {
        key: 'k',
        metaKey: true,
        ctrlKey: true,
        bubbles: true,
      })
      document.dispatchEvent(ev)
    })
    const input = page.getByPlaceholder('搜索项目或会话...')
    await expect(input).toBeVisible({ timeout: 5_000 })

    await page.keyboard.press('Escape')
    await expect(input).not.toBeVisible({ timeout: 3_000 })
  })
})
