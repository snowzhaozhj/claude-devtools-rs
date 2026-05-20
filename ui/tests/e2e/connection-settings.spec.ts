import { expect, test } from '@playwright/test'

async function gotoConnectionSettings(page: import('@playwright/test').Page) {
  await page.goto('/?mock=1&fixture=multi-project-rich')
  await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })
  await page.evaluate(() => {
    ;(window as unknown as { __cdtTest: { openSettingsTab: () => void } }).__cdtTest.openSettingsTab()
  })
  await page.getByRole('tab', { name: '连接' }).click()
}

test.describe('connection settings', () => {
  test('Settings → Connection 输入 host alias 后显示联想与测试状态', async ({ page }) => {
    await gotoConnectionSettings(page)

    await expect(page.getByRole('heading', { name: '连接', exact: true })).toBeVisible({ timeout: 5_000 })
    await expect(page.getByText('本地模式')).toBeVisible()

    const host = page.getByRole('textbox', { name: '主机' })
    await host.fill('mock')
    await expect(page.getByRole('option', { name: 'mock-prod' })).toBeVisible()
    await page.getByRole('option', { name: 'mock-prod' }).click()

    await expect(host).toHaveValue('mock-prod')
    await expect(page.getByRole('spinbutton', { name: '端口' })).toHaveValue('22')
    await page.getByRole('button', { name: '测试连接' }).click()
    await expect(page.getByText('测试通过，未切换当前数据源')).toBeVisible()
  })
})
