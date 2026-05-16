import { expect, test } from '@playwright/test'

test.describe('SessionDetail virtualization', () => {
  test('长会话滚动只挂载窗口，搜索可命中远端 chunk', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })

    await page.evaluate(() => {
      ;(
        window as unknown as {
          __cdtTest: { openTab: (s: string, p: string, l: string) => void }
        }
      ).__cdtTest.openTab('sess-rust-long', 'mock-rich-rust', '长会话滚动虚拟化')
    })

    const conversation = page.locator('.conversation')
    await expect(conversation).toHaveAttribute('data-virtualized', 'true', { timeout: 5_000 })

    const initialRows = await page.locator('.virtual-row').count()
    expect(initialRows).toBeGreaterThan(0)
    expect(initialRows).toBeLessThan(200)

    await conversation.evaluate((el) => {
      el.scrollTop = el.scrollHeight
      el.dispatchEvent(new Event('scroll', { bubbles: true }))
    })
    await expect(page.getByText('long fixture chunk 199')).toBeVisible({ timeout: 5_000 })

    await page.evaluate(() => {
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'f', metaKey: true, bubbles: true }))
    })
    await expect(page.locator('.search-input')).toBeVisible()
    await expect(conversation).toHaveAttribute('data-virtualized', 'false')
    await page.locator('.search-input').fill('remote-search-needle')
    await page.keyboard.press('Enter')
    await expect(page.getByText('remote-search-needle')).toBeVisible({ timeout: 5_000 })
  })
})
