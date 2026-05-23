// User story：用户在 SessionDetail 内滚动后切到其它 tab 再切回，conversation
// 滚动位置 SHALL 按视觉位置语义恢复——粘底优先 / 否则 anchor chunk + offset
// （spec tab-management::Per-tab UI 状态隔离 各 Scenario）。
//
// PR #223 的旧实现保存绝对 scrollTop 数值，与 lazy markdown 占位高度差互相作用
// 导致切回时 scrollTop 被浏览器 clamp，底部场景几千 px 偏差。本 spec 验证锚点法
// 修复（change `tab-scroll-restore-anchor`）。
//
// 真浏览器特异行为（detached element scrollTop=0、lazy markdown IntersectionObserver、
// scroll anchoring、MutationObserver pin 时序）jsdom 都不复现 → e2e 兜底。

import { expect, test, type Page } from '@playwright/test'

const SESSION_ID = 'sess-rust-active'
const PROJECT_ID = 'mock-rich-rust'

interface TestApi {
  openTab: (s: string, p: string, l: string) => void
  openSettingsTab: () => void
  setActiveTab: (id: string) => void
  getPaneLayout: () => { panes: { tabs: { id: string; type: string }[] }[] }
}

interface TabUIStateLike {
  atBottom: boolean
  anchorChunkId: string | null
  anchorOffsetPx: number
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
  // 撑高让 conversation 真有可滚区域——mock fixture 太短自然滚不动
  await page.evaluate(() => {
    const c = document.querySelector<HTMLElement>('.conversation')!
    const spacer = document.createElement('div')
    spacer.style.minHeight = '4000px'
    spacer.style.flexShrink = '0'
    spacer.setAttribute('data-test-spacer', '1')
    c.appendChild(spacer)
    void c.scrollHeight  // 强制 reflow
  })
}

async function getCurrentSessionTabId(page: Page): Promise<string> {
  return page.evaluate(() => {
    const api = (window as unknown as { __cdtTest: TestApi }).__cdtTest
    const layout = api.getPaneLayout()
    const sessTab = layout.panes[0].tabs.find((t) => t.type === 'session')!
    return sessTab.id
  })
}

test.describe('tab 切换：conversation 滚动位置保留（锚点法）', () => {
  test('滚到底切走切回仍粘底（atBottom 语义）', async ({ page }) => {
    await openLongSession(page)
    const tabId = await getCurrentSessionTabId(page)

    // 滚到底部并触发 scroll listener
    await page.evaluate(() => {
      const c = document.querySelector<HTMLElement>('.conversation')!
      c.scrollTop = c.scrollHeight  // 浏览器自动 clamp 到 max
      c.dispatchEvent(new Event('scroll'))
    })
    await page.waitForTimeout(50)

    // 验证保存时点 atBottom=true
    const saved = await page.evaluate(async (id) => {
      const ts = await import('/src/lib/tabStore.svelte.ts')
      ;(window as unknown as { __cdtTest: TestApi }).__cdtTest.openSettingsTab()
      await new Promise((r) => setTimeout(r, 100))
      return ts.getTabUIState(id) as TabUIStateLike
    }, tabId)
    expect(saved.atBottom).toBe(true)
    expect(saved.anchorChunkId).toBeNull()

    // 切回，等 mount + tick + bottom pin
    await page.evaluate((id) => {
      ;(window as unknown as { __cdtTest: TestApi }).__cdtTest.setActiveTab(id)
    }, tabId)
    await page.waitForTimeout(400)

    const restored = await page.evaluate(() => {
      const c = document.querySelector<HTMLElement>('.conversation')!
      const dist = c.scrollHeight - c.scrollTop - c.clientHeight
      return { dist, scrollTop: c.scrollTop, scrollHeight: c.scrollHeight }
    })
    expect(restored.dist).toBeLessThanOrEqual(16)  // 仍粘底
  })

  test('滚到中间位置切走切回 anchor chunk 视口顶偏差 ≤ 50 px', async ({ page }) => {
    await openLongSession(page)
    const tabId = await getCurrentSessionTabId(page)

    // 滚到中间位置（spacer 之前的 chunks 区域），让某 chunk 跨视口顶或完全在视口内
    await page.evaluate(() => {
      const c = document.querySelector<HTMLElement>('.conversation')!
      c.scrollTop = 200
      c.dispatchEvent(new Event('scroll'))
    })
    await page.waitForTimeout(50)

    // 抓切走前 anchor chunk 在视口内的位置
    const before = await page.evaluate(async (id) => {
      const c = document.querySelector<HTMLElement>('.conversation')!
      const ts = await import('/src/lib/tabStore.svelte.ts')
      ;(window as unknown as { __cdtTest: TestApi }).__cdtTest.openSettingsTab()
      await new Promise((r) => setTimeout(r, 100))
      const saved = ts.getTabUIState(id) as TabUIStateLike
      return { saved, beforeScrollTop: c.scrollTop }
    }, tabId)
    expect(before.saved.atBottom).toBe(false)
    expect(before.saved.anchorChunkId).not.toBeNull()

    // 切回
    await page.evaluate((id) => {
      ;(window as unknown as { __cdtTest: TestApi }).__cdtTest.setActiveTab(id)
    }, tabId)
    await page.waitForTimeout(400)

    // 切回后再撑 spacer（新 mount 没有）
    await page.evaluate(() => {
      const c = document.querySelector<HTMLElement>('.conversation')!
      const spacer = document.createElement('div')
      spacer.style.minHeight = '4000px'
      spacer.style.flexShrink = '0'
      c.appendChild(spacer)
    })

    // anchor chunk 在视口顶的位置 vs 切走前保存的 offset 差 ≤ 50 px
    const afterDelta = await page.evaluate((savedAnchor) => {
      const c = document.querySelector<HTMLElement>('.conversation')!
      const containerRect = c.getBoundingClientRect()
      const target = c.querySelector<HTMLElement>(
        `[data-chunk-id="${savedAnchor.anchorChunkId!.replace(/"/g, '\\"')}"]`,
      )
      if (!target) return { delta: Infinity, missing: true }
      const rect = target.getBoundingClientRect()
      const currentOffset = rect.top - containerRect.top
      return {
        delta: Math.abs(currentOffset - savedAnchor.anchorOffsetPx),
        missing: false,
      }
    }, before.saved)
    expect(afterDelta.missing).toBe(false)
    expect(afterDelta.delta).toBeLessThanOrEqual(50)
  })

  test('anchorChunkId 失效（chunk 被删） → 降级到顶部 + console.warn', async ({ page }) => {
    // 注册顺序很重要：console listener SHALL 先于 page.goto/navigate，
    // 否则之前的 console 事件不会被收到（且按消息文本过滤而非 type，
    // 不同 chromium 版本对 warn 的 type 命名可能为 'warn' 或 'warning'）
    const consoleMessages: string[] = []
    page.on('console', (msg) => consoleMessages.push(msg.text()))

    await openLongSession(page)
    const tabId = await getCurrentSessionTabId(page)

    // 顺序很关键：openSettingsTab 触发 SessionDetail onDestroy，会用 latestAnchor
    // 覆盖 tabStore，**必须**等 destroy 落定后再手工 saveTabUIState 覆盖；否则
    // 切回时读到的是 destroy 时刻保存的真实 anchor 不是失效锚点
    await page.evaluate(async () => {
      ;(window as unknown as { __cdtTest: TestApi }).__cdtTest.openSettingsTab()
      await new Promise((r) => setTimeout(r, 100))
    })

    await page.evaluate(
      async ({ id }) => {
        const ts = await import('/src/lib/tabStore.svelte.ts')
        ts.saveTabUIState(id, {
          expandedChunks: new Set(),
          expandedItems: new Set(),
          searchVisible: false,
          contextPanelVisible: false,
          atBottom: false,
          anchorChunkId: 'absolutely-nonexistent-chunk-id',
          anchorOffsetPx: 100,
        })
      },
      { id: tabId },
    )

    // 切回触发 mount → restoreScrollAnchor 走 anchor 失效兜底路径
    await page.evaluate((id) => {
      ;(window as unknown as { __cdtTest: TestApi }).__cdtTest.setActiveTab(id)
    }, tabId)
    await page.waitForTimeout(400)

    const final = await page.evaluate(
      () => document.querySelector<HTMLElement>('.conversation')!.scrollTop,
    )
    expect(final).toBe(0)  // 降级到顶部
    expect(consoleMessages.some((w) => w.includes('anchorChunkId not found'))).toBe(true)
  })
})
