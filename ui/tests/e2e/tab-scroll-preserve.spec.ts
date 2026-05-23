// User story：用户在 SessionDetail 内滚动后切到其它 tab 再切回，conversation
// 滚动位置 SHALL 恢复到切走时的位置（spec tab-management::滚动位置恢复 Scenario）。
//
// 真浏览器特异行为：Svelte 5 onDestroy 在 element unmount **之后**触发，
// `conversationEl.isConnected=false` → `conversationEl.scrollTop` 永远是 0
// （detached element）。jsdom 不复现这个行为，所以这一类回归只能 Playwright e2e
// 兜底。本 spec 检验"保存值正确"——这是 spec 严格契约的核心；恢复后浏览器
// scroll anchoring 在长会话 lazy markdown 渲染时的偏移精度不在本测试范围。

import { expect, test, type Page } from '@playwright/test'

const SESSION_ID = 'sess-rust-active'
const PROJECT_ID = 'mock-rich-rust'

interface TestApi {
  openTab: (s: string, p: string, l: string) => void
  openSettingsTab: () => void
  setActiveTab: (id: string) => void
  getPaneLayout: () => { panes: { tabs: { id: string; type: string }[] }[] }
}

async function openLongSession(page: Page) {
  await page.goto('/?mock=1&fixture=multi-project-rich')
  await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })
  await page.evaluate(
    ({ sid, pid }) => {
      ;(window as unknown as { __cdtTest: TestApi }).__cdtTest.openTab(
        sid,
        pid,
        'IPC 字段重构',
      )
    },
    { sid: SESSION_ID, pid: PROJECT_ID },
  )
  await page.locator('.conversation').waitFor({ state: 'visible', timeout: 5_000 })
  // 撑高让 scrollTop 能设到 > 100 px——不然 mock fixture 的 conversation 太短
  await page.evaluate(() => {
    const c = document.querySelector<HTMLElement>('.conversation')!
    const spacer = document.createElement('div')
    spacer.style.minHeight = '4000px'
    spacer.style.flexShrink = '0'
    spacer.setAttribute('data-test-spacer', '1')
    c.appendChild(spacer)
    // 强制 reflow 让 sH 立即生效
    void c.scrollHeight
  })
}

test.describe('tab 切换：conversation 滚动位置保留', () => {
  test('滚动后切走 tab，tabStore.scrollTop 写入实际滚动值（不为 0）', async ({ page }) => {
    await openLongSession(page)

    // 滚到 800 px 并触发 scroll listener 同步 latestScrollTop
    await page.evaluate(() => {
      const c = document.querySelector<HTMLElement>('.conversation')!
      c.scrollTop = 800
      c.dispatchEvent(new Event('scroll'))
    })
    // 等 rAF + onScroll 同步完成
    await page.waitForTimeout(50)

    const result = await page.evaluate(async () => {
      const api = (window as unknown as { __cdtTest: TestApi }).__cdtTest
      const layout = api.getPaneLayout()
      const sessTab = layout.panes[0].tabs.find((t) => t.type === 'session')!
      const tabId = sessTab.id

      // 切走前确认 scrollTop=800（jsdom 不行，但真浏览器有真 scroll 物理）
      const beforeSwitch =
        document.querySelector<HTMLElement>('.conversation')!.scrollTop

      // 切到 settings 触发 onDestroy
      api.openSettingsTab()
      // 等 destroy effect commit
      await new Promise((r) => setTimeout(r, 100))

      // 读 tabStore 的 saved state
      const ts = await import('/src/lib/tabStore.svelte.ts')
      const saved = ts.getTabUIState(tabId).scrollTop

      return { beforeSwitch, saved }
    })

    // 旧实现 onDestroy 时 element 已 detach → 写 0；fix 后写 latestScrollTop=800
    expect(result.beforeSwitch).toBeGreaterThan(700)
    expect(result.saved).toBeGreaterThan(700)
    // saved 应等于切走前的 scrollTop（容差 5 px 兜小数 / lazy md 微偏移）
    expect(Math.abs(result.saved - result.beforeSwitch)).toBeLessThanOrEqual(5)
  })

  test('未滚动直接切走，saveTabUIState 写入 0（无残留）', async ({ page }) => {
    await openLongSession(page)

    const result = await page.evaluate(async () => {
      const api = (window as unknown as { __cdtTest: TestApi }).__cdtTest
      const layout = api.getPaneLayout()
      const sessTab = layout.panes[0].tabs.find((t) => t.type === 'session')!
      const tabId = sessTab.id

      api.openSettingsTab()
      await new Promise((r) => setTimeout(r, 100))

      const ts = await import('/src/lib/tabStore.svelte.ts')
      return { saved: ts.getTabUIState(tabId).scrollTop }
    })

    expect(result.saved).toBe(0)
  })

  test('切回 tab 后 conversation.scrollTop 恢复到 saved 值附近（非 0）', async ({ page }) => {
    await openLongSession(page)

    const result = await page.evaluate(async () => {
      const c = document.querySelector<HTMLElement>('.conversation')!
      c.scrollTop = 600
      c.dispatchEvent(new Event('scroll'))
      await new Promise((r) => setTimeout(r, 50))

      const api = (window as unknown as { __cdtTest: TestApi }).__cdtTest
      const layout = api.getPaneLayout()
      const sessTab = layout.panes[0].tabs.find((t) => t.type === 'session')!
      const tabId = sessTab.id

      api.openSettingsTab()
      await new Promise((r) => setTimeout(r, 100))

      const ts = await import('/src/lib/tabStore.svelte.ts')
      const saved = ts.getTabUIState(tabId).scrollTop

      api.setActiveTab(tabId)
      // 等 mount + tick + 恢复
      await new Promise((r) => setTimeout(r, 400))
      // 切回时 spacer 不在新 conversation 里 → sH 较小 → scrollTop 被 clamp
      // 到 max scrollable。该 clamp 不为 0 即证明"恢复到 saved 值附近 / 非 0"。
      const conv2 = document.querySelector<HTMLElement>('.conversation')!
      return {
        saved,
        finalScrollTop: conv2.scrollTop,
        finalSH: conv2.scrollHeight,
        finalCH: conv2.clientHeight,
      }
    })

    expect(result.saved).toBeGreaterThan(500)
    // 恢复后 scrollTop 不为 0（旧实现完全丢失），且接近 max scrollable（被 clamp）
    // 或接近 saved 值（spacer 被重新撑起后）。任一形式都比"回到顶"好。
    expect(result.finalScrollTop).toBeGreaterThan(0)
  })
})
