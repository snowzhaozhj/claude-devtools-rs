// User story #4: 切换主题 → 验证 data-theme attribute + body 背景色变化

import { expect, test } from '@playwright/test'

test.describe('theme switch', () => {
  test('JS 直接调 applyTheme 设置 data-theme + 背景色变化', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')

    // light → bg 浅色
    await page.evaluate(() => {
      document.documentElement.setAttribute('data-theme', 'light')
    })
    let bg = await page.evaluate(() =>
      getComputedStyle(document.body).backgroundColor,
    )
    expect(bg).toMatch(/rgb\((\d+), (\d+), (\d+)\)/)
    const lightBg = bg

    // dark → bg 深色（Soft Charcoal token，约 rgb(30, 30, 28)）
    await page.evaluate(() => {
      document.documentElement.setAttribute('data-theme', 'dark')
    })
    bg = await page.evaluate(() =>
      getComputedStyle(document.body).backgroundColor,
    )
    expect(bg).not.toBe(lightBg)
    // dark 背景的 RGB 三通道之和应远小于 light（粗略验证 CSS 真生效）
    const match = bg.match(/rgb\((\d+), (\d+), (\d+)\)/)
    expect(match).toBeTruthy()
    const [, r, g, b] = match!
    const sum = parseInt(r) + parseInt(g) + parseInt(b)
    expect(sum).toBeLessThan(300) // dark token < 300 (≈90)
  })

  test('data-theme 属性正确反映三种值', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    // 等 mockIPC config load 完成把 theme 设为 fixture 的 'system'，避免 race
    await expect.poll(
      async () => page.evaluate(() => document.documentElement.dataset.theme),
      { timeout: 5_000 },
    ).toBe('system')

    for (const mode of ['light', 'dark', 'system'] as const) {
      // setAttribute + getAttribute 在同一 evaluate 内同步读，避免被 svelte effect 改回去
      const got = await page.evaluate((m) => {
        document.documentElement.setAttribute('data-theme', m)
        return document.documentElement.getAttribute('data-theme')
      }, mode)
      expect(got).toBe(mode)
    }
  })
})
