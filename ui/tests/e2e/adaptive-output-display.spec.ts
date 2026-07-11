// User story：长输出不淹没对话流（change adaptive-output-display）。
//
// 对应 spec Scenario：
// - session-display::长文本输出限高预览且完整内容留在 DOM
// - session-display::输出内部滚动区域键盘可访问（溢出的输出可键盘滚动）
// - tool-viewer-routing::超大行导向输出首尾切片（省略接缝 + 中段不在 DOM）
// - session-display::限高预览与搜索共存（搜索 hydrate 后限高仍生效、中段关键词命中）
//
// fixture：`multi-project-rich` 的 `sess-rust-active` 含专用 adaptiveChunk——
// 120 行 prose output（第 61 行埋 NEEDLE-MID-XYZQ）+ 1200 行 Bash 输出。

import { expect, test, type Page } from '@playwright/test'

// sess-rust-2：adaptiveChunk 专用承载会话（sess-rust-active 布局被
// tab-scroll-preserve 锚点断言依赖，不能增删 chunk）。
const SESSION_ID = 'sess-rust-2'
const PROJECT_ID = 'mock-rich-rust'

// 限高上限 22rem = 352px（--ao-preview-max-block），留边框 / 取整余量
const BOUNDED_MAX_PX = 360

async function openSession(page: Page) {
  await page.goto('/?mock=1&fixture=multi-project-rich')
  await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })
  await page.evaluate(
    ({ sid, pid }) => {
      ;(
        window as unknown as {
          __cdtTest: { openTab: (s: string, p: string, l: string) => void }
        }
      ).__cdtTest.openTab(sid, pid, '修复 watcher flake')
    },
    { sid: SESSION_ID, pid: PROJECT_ID },
  )
  await page.locator('.conversation').waitFor({ state: 'visible', timeout: 5_000 })
}

/** 展开 adaptiveChunk（默认折叠）——它的工具列表按钮文案是 "1 tool call · 1 message"。 */
async function expandAdaptiveChunk(page: Page) {
  const chunkToggle = page
    .getByRole('button', { name: '展开工具调用列表' })
    .filter({ hasText: '1 tool call · 1 message' })
    .first()
  await chunkToggle.scrollIntoViewIfNeeded()
  await chunkToggle.click()
}

/** 展开 adaptiveChunk 的长 prose Output 项，返回其限高 viewport locator。 */
async function expandLongProse(page: Page) {
  await expandAdaptiveChunk(page)
  const row = page.getByText('第 1 条：字段 field_0', { exact: false }).first()
  await row.scrollIntoViewIfNeeded()
  await row.click()
  const viewport = page.locator('.ao-prose .ao-viewport').first()
  await viewport.waitFor({ state: 'visible', timeout: 3_000 })
  return viewport
}

test.describe('自适应输出展示', () => {
  test('长 prose 限高预览：信息气味 + 完整内容留 DOM + 高度受限', async ({ page }) => {
    await openSession(page)
    const viewport = await expandLongProse(page)

    // 信息气味：总行数 + "预览"
    const scent = page.locator('.ao-prose .ao-scent').first()
    await expect(scent).toContainText('120 行')
    await expect(scent).toContainText('预览')

    // 高度受限（22rem 上限）
    const height = await viewport.evaluate((el) => el.getBoundingClientRect().height)
    expect(height).toBeLessThanOrEqual(BOUNDED_MAX_PX)

    // 完整内容留 DOM：中段关键词在（限高只是 CSS，不切片）。
    // lazy markdown 视口内 hydrate 后断言（attached 而非 visible——在滚动区内可能不可见）。
    await expect
      .poll(
        () => page.locator('.ao-prose .ao-viewport').first().textContent(),
        { timeout: 5_000 },
      )
      .toContain('NEEDLE-MID-XYZQ')
  })

  test('溢出的 prose viewport 可键盘聚焦滚动', async ({ page }) => {
    await openSession(page)
    const viewport = await expandLongProse(page)

    // hydrate 后内容溢出 → attachment 置 tabindex=0 + region role
    await expect.poll(() => viewport.getAttribute('tabindex'), { timeout: 5_000 }).toBe('0')
    const label = await viewport.getAttribute('aria-label')
    expect(label).toContain('Output')
    expect(label).toContain('可滚动')

    await viewport.focus()
    await expect(viewport).toBeFocused()
    await page.keyboard.press('ArrowDown')
    await page.keyboard.press('ArrowDown')
    await expect
      .poll(() => viewport.evaluate((el) => el.scrollTop), { timeout: 2_000 })
      .toBeGreaterThan(0)
  })

  test('超大 Bash 输出首尾切片：省略接缝 + 中段不在 DOM + 复制全文常驻', async ({ page }) => {
    await openSession(page)
    await expandAdaptiveChunk(page)
    // 展开 Bash 工具项（summary 含 seq 命令）
    const bashRow = page.getByText('seq 1 1200', { exact: false }).first()
    await bashRow.scrollIntoViewIfNeeded()
    await bashRow.click()

    const seam = page.locator('.output-seam').first()
    await seam.waitFor({ state: 'attached', timeout: 3_000 })
    await expect(seam).toContainText('已省略')
    await expect(seam).toContainText('400 行') // 1200 - 400*2

    const body = page.locator('.ao').filter({ has: seam }).first()
    const text = await body.textContent()
    expect(text).toContain('bash-line-0 ')
    expect(text).toContain('bash-line-1199')
    expect(text).not.toContain('bash-line-600 ')

    // 复制全文常驻（header 内非 hover-only）
    const copyBtn = body.locator('.ao-header button')
    await expect(copyBtn).toBeVisible()
    await expect(copyBtn).toBeEnabled()
  })

  test('搜索 hydrate 后限高仍生效、中段关键词命中', async ({ page }) => {
    await openSession(page)
    const viewport = await expandLongProse(page)

    // Cmd+F 搜索中段唯一关键词（搜索前 flushAll 强制 hydrate 全部 lazy markdown）
    await page.evaluate(() => {
      document.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 'f',
          metaKey: true,
          ctrlKey: true,
          bubbles: true,
          cancelable: true,
        }),
      )
    })
    const searchInput = page.locator('.search-bar input').first()
    await searchInput.waitFor({ state: 'visible', timeout: 3_000 })
    await searchInput.fill('NEEDLE-MID-XYZQ')
    await searchInput.press('Enter')

    // 命中 ≥ 1（mark 高亮落在限高 viewport 内）
    await expect
      .poll(() => page.locator('.ao-prose .ao-viewport mark').count(), { timeout: 5_000 })
      .toBeGreaterThan(0)

    // 搜索 hydrate 后限高不被破坏
    const height = await viewport.evaluate((el) => el.getBoundingClientRect().height)
    expect(height).toBeLessThanOrEqual(BOUNDED_MAX_PX)
  })
})
