import { expect, test } from '@playwright/test'

test.describe('context panel', () => {
  test('tool output row scrolls to the owning AI chunk and tool node', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })

    await page.evaluate(() => {
      ;(
        window as unknown as {
          __cdtTest: { openTab: (s: string, p: string, l: string) => void }
        }
      ).__cdtTest.openTab('sess-rust-active', 'mock-rich-rust', 'IPC 字段重构')
    })

    await page.getByRole('button', { name: /Context 7/ }).click()
    await expect(page.getByText('Visible Context')).toBeVisible({ timeout: 5_000 })
    await page.getByRole('button', { name: /1 tool/ }).click()
    await page.getByRole('button', { name: /Bash/ }).click()

    const chunk = page.locator('[data-chunk-id="a-active-2:0"]')
    const tool = page.locator('[data-tool-use-id="tu-active-2"]')
    await expect(chunk).toBeVisible({ timeout: 5_000 })
    await expect(tool).toBeVisible({ timeout: 5_000 })
    await expect(chunk).toHaveClass(/msg-row-anchor-hit/)
    await expect(tool).toHaveClass(/tool-anchor-hit/)
  })
})
