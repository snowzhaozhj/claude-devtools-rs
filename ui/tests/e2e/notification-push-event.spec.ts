// User story: issue #258 通知 push event 主路径替代 30s 轮询。
//
// 验收：后端 `app.emit("notification-added")` 触发后，前端 unread badge 在
// 100 ms 内更新（远低于原 30s 轮询粒度）。本 spec 通过 mock-only helper
// `__cdtTest.simulateNotificationAdded` 模拟后端推送 + mutate fixture state，
// 走完整 listener → refreshUnreadCount → setBadgeCount → UnifiedTitleBar
// $derived 重渲链路。
//
// helper 实现见 `ui/src/lib/tauriMock.ts::simulateNotificationAdded`。production
// bundle 完全 DCE 不含这些代码（`main.ts` 内 `import.meta.env.DEV` 块，验证
// 见 `tauriMock.bundle.test.ts`）。

import { expect, test } from '@playwright/test'

async function gotoWithMockReady(page: import('@playwright/test').Page) {
  await page.goto('/?mock=1&fixture=multi-project-rich')
  await page.waitForFunction(
    () =>
      '__cdtTest' in window &&
      typeof (window as unknown as { __cdtTest: { simulateNotificationAdded?: unknown } })
        .__cdtTest.simulateNotificationAdded === 'function',
    { timeout: 5_000 },
  )
}

test.describe('notification push event (issue #258)', () => {
  test.afterEach(async ({ page }) => {
    // fixture state 是 vite dev server 进程内 module-level 共享引用——
    // local reuseExistingServer=true 时下一个 spec 会拿到本 spec mutate 后的
    // 残留 unreadCount。显式复位掉所有模拟通知（id 前缀 notif-sim-）。
    await page.evaluate(() => {
      const cdt = (window as unknown as {
        __cdtTest?: { resetSimulatedNotifications?: () => void }
      }).__cdtTest
      cdt?.resetSimulatedNotifications?.()
    })
  })

  test('notification-added emit 后 100 ms 内 chrome 通知 badge 增加 +1', async ({ page }) => {
    await gotoWithMockReady(page)

    // multi-project-rich fixture 默认 1 unread → chrome 通知按钮显示 badge "1"
    const notifButton = page.getByRole('button', { name: '通知' })
    await expect(notifButton).toBeVisible({ timeout: 5_000 })
    const badge = notifButton.locator('.badge')
    await expect(badge).toHaveText('1', { timeout: 5_000 })

    // 触发 push event 并记录起点；等 < 100 ms 内 unread → 2
    const t0 = await page.evaluate(async () => {
      const cdt = (window as unknown as {
        __cdtTest: { simulateNotificationAdded: () => Promise<void> }
      }).__cdtTest
      const start = performance.now()
      await cdt.simulateNotificationAdded()
      return start
    })

    // expect.timeout 默认 5_000 ms 兜底过宽——这里要严格 SLA：100 ms。
    // 实际链路：emit → listener → IPC mock get_notifications（同步 return）
    // → setUnreadCount → $derived → DOM patch；mock 模式无网络 / 子进程开销。
    await expect(badge).toHaveText('2', { timeout: 1_000 })
    const elapsed = await page.evaluate((start: number) => performance.now() - start, t0)
    expect(elapsed, `badge 更新延迟 ${elapsed.toFixed(1)}ms 应 < 100ms`).toBeLessThan(100)
  })

  test('多次 simulateNotificationAdded 串发 unread 单调递增', async ({ page }) => {
    await gotoWithMockReady(page)

    const notifButton = page.getByRole('button', { name: '通知' })
    const badge = notifButton.locator('.badge')
    await expect(badge).toHaveText('1', { timeout: 5_000 })

    // 串发 3 次（不同 id）→ unread 1 → 2 → 3 → 4
    for (let i = 0; i < 3; i += 1) {
      await page.evaluate(
        async (idx: number) => {
          const cdt = (window as unknown as {
            __cdtTest: {
              simulateNotificationAdded: (override?: { id?: string }) => Promise<void>
            }
          }).__cdtTest
          await cdt.simulateNotificationAdded({ id: `notif-sim-burst-${idx}` })
        },
        i,
      )
    }

    await expect(badge).toHaveText('4', { timeout: 1_000 })
  })
})
