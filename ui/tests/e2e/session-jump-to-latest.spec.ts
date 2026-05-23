// User story：长会话场景下用户上滚后用「跳到最新消息」浮动按钮 + 跨平台键盘
// 快捷键回到最新输出。spec.md::Requirement Quick Anchor Navigation。
//
// 实现细节：
// - fixture 现有 chunks 不足以让 conversation scrollHeight 远大于 clientHeight，
//   用 page.evaluate 在 conversation 内插一个高 spacer div 让 scrollHeight 巨大
// - 浏览器实际触发的 scroll / wheel / scrollend 事件由 playwright 直接走真 DOM

import { expect, test, type Page } from '@playwright/test'

const SESSION_ID = 'sess-rust-active'
const PROJECT_ID = 'mock-rich-rust'

/**
 * 用 dispatchEvent 派 mod-key keydown。playwright `keyboard.press('Meta+f')` 在
 * body focus 漂走时事件不冒泡到 document（registry dispatcher 的 listen 点），
 * 故走 evaluate 内 document.dispatchEvent + bubbles:true。
 *
 * 与 keyboard-shortcuts.spec.ts L36-44 同款 helper。
 */
async function pressMod(page: Page, key: string) {
  await page.evaluate((k) => {
    document.dispatchEvent(
      new KeyboardEvent('keydown', {
        key: k,
        metaKey: true,
        ctrlKey: true,
        bubbles: true,
        cancelable: true,
      }),
    )
  }, key)
}

async function openLongSession(page: Page) {
  await page.goto('/?mock=1&fixture=multi-project-rich')
  await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })
  await page.evaluate(
    ({ sid, pid }) => {
      ;(
        window as unknown as {
          __cdtTest: { openTab: (s: string, p: string, l: string) => void }
        }
      ).__cdtTest.openTab(sid, pid, 'IPC 字段重构')
    },
    { sid: SESSION_ID, pid: PROJECT_ID },
  )
  // 等 conversation 容器渲染
  await page.locator('.conversation').waitFor({ state: 'visible', timeout: 5_000 })
  // 在 conversation 末尾插一个 4000px 高 spacer，让 scrollHeight 远大于 clientHeight，
  // 模拟"长会话"——用户向上滚的距离才能 > JUMP_THRESHOLD (300px)。
  // 插 spacer 是 DOM 改动不触发 scroll 事件，所以再 dispatch 一次让 attach 内的
  // scheduleUpdateIsFar 跑，按钮按距底重新派生 visibility。
  await page.evaluate(() => {
    const conversation = document.querySelector<HTMLElement>('.conversation')
    if (!conversation) throw new Error('.conversation not found')
    const spacer = document.createElement('div')
    // .conversation 是 flex column 容器，spacer 默认 flex-shrink: 1 会被压扁。
    // min-height + flex-shrink: 0 强制保留高度让 scrollHeight 真增大。
    spacer.style.minHeight = '4000px'
    spacer.style.flexShrink = '0'
    spacer.setAttribute('data-test-spacer', '1')
    conversation.appendChild(spacer)
    // 强制触发一次 scroll 事件让 isFar 派生重算
    conversation.dispatchEvent(new Event('scroll'))
  })
  // 等 rAF 节流 + state 应用
  await page.waitForTimeout(50)
}

async function getDistanceFromBottom(page: Page) {
  return page.evaluate(() => {
    const c = document.querySelector<HTMLElement>('.conversation')
    if (!c) return -1
    return c.scrollHeight - c.scrollTop - c.clientHeight
  })
}

test.describe('Quick Anchor Navigation：跳到最新消息', () => {
  test('距底 > 300px 时按钮浮现，距底 ≤ 300px 时按钮隐藏', async ({ page }) => {
    await openLongSession(page)
    const btn = page.locator('.jump-to-latest')

    // 默认 scrollTop=0 → 距底 = scrollHeight - clientHeight ≈ 4000px > 300，按钮浮现
    await expect(btn).toHaveClass(/jump-to-latest-visible/)
    expect(await getDistanceFromBottom(page)).toBeGreaterThan(300)
    await expect(btn).toHaveAttribute('aria-hidden', 'false')

    // 滚到接近底部（距底 < 300）→ 按钮隐藏
    await page.evaluate(() => {
      const c = document.querySelector<HTMLElement>('.conversation')!
      c.scrollTo({ top: c.scrollHeight - c.clientHeight - 100, behavior: 'auto' })
    })
    await expect(btn).not.toHaveClass(/jump-to-latest-visible/, { timeout: 2_000 })
    expect(await getDistanceFromBottom(page)).toBeLessThan(300)
  })

  test('点击按钮 smooth 滚动到底，按钮隐藏', async ({ page }) => {
    await openLongSession(page)
    const btn = page.locator('.jump-to-latest')
    await expect(btn).toHaveClass(/jump-to-latest-visible/)

    // 用 evaluate 直接调 button.click()——绕过 sidebar svg 在 headless 下偶发的
    // pointer-event 拦截（layout 隔离问题，与产品行为无关）
    await page.evaluate(() => {
      document.querySelector<HTMLButtonElement>('.jump-to-latest')?.click()
    })
    // smooth scroll 异步完成；scrollend 事件清 isProgrammaticScroll，按钮重新派生 visibility
    await expect(btn).not.toHaveClass(/jump-to-latest-visible/, { timeout: 3_000 })
    expect(await getDistanceFromBottom(page)).toBeLessThanOrEqual(16)
  })

  test('键盘快捷键触发跳底（Cmd+ArrowDown on macOS, Ctrl+End 其它）', async ({ page }) => {
    await openLongSession(page)
    const btn = page.locator('.jump-to-latest')
    await expect(btn).toHaveClass(/jump-to-latest-visible/)

    // 让 SessionDetail 处于 focused pane + active tab —— 通过 click conversation
    // 容器把 focus 落到主 pane（force 绕过任何 sidebar pointer 拦截）
    await page.locator('.conversation').click({ force: true, position: { x: 100, y: 100 } })

    // 检测页面里 isMac 实际值（platform.ts 可能 cache 了启动时的 navigator）
    const macKey = await page.evaluate(() => /mac/i.test(navigator.platform || navigator.userAgent || ''))
    if (macKey) {
      await page.keyboard.press('Meta+ArrowDown')
    } else {
      await page.keyboard.press('Control+End')
    }
    await expect(btn).not.toHaveClass(/jump-to-latest-visible/, { timeout: 3_000 })
    expect(await getDistanceFromBottom(page)).toBeLessThanOrEqual(16)
  })

  test('SearchBar input focused 时按快捷键不拦截，conversation 不滚', async ({ page }) => {
    await openLongSession(page)
    // 打开 SearchBar：Cmd+F / Ctrl+F
    // mod-key 走 dispatchEvent（registry dispatcher 在 document）；platform-aware
    // ctrlKey/metaKey 都置 true 由 normalize 展开。详 pressMod 注释。
    await pressMod(page, 'f')
    const isMac = await page.evaluate(() => /mac/i.test(navigator.platform || navigator.userAgent || ''))
    const searchInput = page.locator('.search-bar input').first()
    await searchInput.waitFor({ state: 'visible', timeout: 2_000 })
    await searchInput.focus()

    const distBefore = await getDistanceFromBottom(page)
    expect(distBefore).toBeGreaterThan(300)

    await page.keyboard.press(isMac ? 'Meta+ArrowDown' : 'Control+End')
    // 搜索框 focused 时浏览器原生光标导航生效，conversation 不应滚动
    // 等一小段时间确保任何 smooth scroll 都已经完成（如果错误地拦截了的话）
    await page.waitForTimeout(500)
    const distAfter = await getDistanceFromBottom(page)
    // 容许 ±2px 抖动；关键是不能滚到底（distAfter 不能 ≤ 16）
    expect(distAfter).toBeGreaterThan(300)
  })

  test('未打开 ContextPanel 时按钮不带 jump-to-latest-shifted class', async ({ page }) => {
    await openLongSession(page)
    const btn = page.locator('.jump-to-latest')
    await expect(btn).toHaveClass(/jump-to-latest-visible/)
    await expect(btn).not.toHaveClass(/jump-to-latest-shifted/)
  })

  test('reduced-motion 下点击立即到底（behavior auto，不走 smooth）', async ({ page }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' })
    await openLongSession(page)
    const btn = page.locator('.jump-to-latest')
    await expect(btn).toHaveClass(/jump-to-latest-visible/)

    await page.evaluate(() => {
      document.querySelector<HTMLButtonElement>('.jump-to-latest')?.click()
    })
    // 'auto' behavior 同步到位 + queueMicrotask stop programmatic-scroll
    await page.waitForTimeout(50)
    expect(await getDistanceFromBottom(page)).toBeLessThanOrEqual(16)
    await expect(btn).not.toHaveClass(/jump-to-latest-visible/, { timeout: 1_000 })
  })

  test('快连点击不互扰，clearTimeout 旧 timer 让最终滚动稳定到底', async ({ page }) => {
    await openLongSession(page)
    const btn = page.locator('.jump-to-latest')
    await expect(btn).toHaveClass(/jump-to-latest-visible/)

    // 同 task 内连点 3 次：每次 startProgrammaticScroll 都 clearTimeout 旧 timer +
    // 重新 setTimeout，旧 timer 不应提前清掉新 scroll 的 flag
    await page.evaluate(() => {
      const b = document.querySelector<HTMLButtonElement>('.jump-to-latest')!
      b.click()
      b.click()
      b.click()
    })
    // 最终 smooth scroll 完成（scrollend 触发）→ button 隐藏 + 距底 ≤ 16
    await expect(btn).not.toHaveClass(/jump-to-latest-visible/, { timeout: 3_000 })
    expect(await getDistanceFromBottom(page)).toBeLessThanOrEqual(16)
  })
})
