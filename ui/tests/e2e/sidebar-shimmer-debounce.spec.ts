// issue #259 e2e：metadata-pending shimmer 触发条件锁——
// (a) metadata 慢到达（> 1500 ms）→ shimmer 出现
// (b) metadata 快到达（< 1500 ms）→ shimmer 永不出现
// (c) 启动 5 s 内 sidebar shimmer 元素 < 5（issue 验收预算）
//
// mock 钩子：URL `?pendingMetadataDelayMs=N` 让 list_group_sessions
// 返回纯骨架（title=null/messageCount=0/isOngoing=false），N ms 后通过
// session-metadata-update emit 把真值补回。详见 ui/src/lib/tauriMock.ts。

import { expect, test } from '@playwright/test'

async function selectRustProject(page: import('@playwright/test').Page) {
  await page.locator('.dash-row, .dash-card', { hasText: 'rust-port' }).first().click()
}

test.describe('issue #259 metadata-pending shimmer 阈值', () => {
  test('metadata 慢到达（永不到达）→ 阈值后挂 .metadata-pending', async ({ page }) => {
    // delay=99999 → mock 不会 emit 真值，session 永远 pending
    await page.goto('/?mock=1&fixture=multi-project-rich&pendingMetadataDelayMs=99999')
    await selectRustProject(page)

    // 等 sidebar 至少渲染一条 session-item（骨架态，title 显 sessionId 前缀）
    await expect(page.locator('.session-item').first()).toBeVisible({ timeout: 5_000 })

    // 阈值前（< 1500 ms）：data-shimmer-state=pending-pre-threshold，无 .metadata-pending
    await expect(
      page.locator('.session-item[data-shimmer-state="pending-pre-threshold"]').first(),
    ).toBeVisible({ timeout: 1_000 })
    await expect(page.locator('.session-item.metadata-pending')).toHaveCount(0)

    // 阈值后（> 1500 ms）：至少 1 条进入 shimmering 态
    await expect(
      page.locator('.session-item[data-shimmer-state="shimmering"]').first(),
    ).toBeVisible({ timeout: 4_000 })
    expect(await page.locator('.session-item.metadata-pending').count()).toBeGreaterThan(0)
  })

  test('metadata 快到达（300 ms < 1500 ms）→ 永不显 shimmer', async ({ page }) => {
    await page.goto('/?mock=1&fixture=multi-project-rich&pendingMetadataDelayMs=300')
    await selectRustProject(page)

    await expect(page.locator('.session-item').first()).toBeVisible({ timeout: 5_000 })

    // 给 metadata 到达 + 阈值过窗一并空跑（300 ms emit + 1500 ms 阈值 + 余量）
    await page.waitForTimeout(2_200)

    // 任何 session-item 都不应进入 shimmering 态
    await expect(page.locator('.session-item.metadata-pending')).toHaveCount(0)
    await expect(page.locator('.session-item[data-shimmer-state="shimmering"]')).toHaveCount(0)
    // 全部 resolved
    const items = page.locator('.session-item')
    const count = await items.count()
    expect(count).toBeGreaterThan(0)
    for (let i = 0; i < count; i++) {
      await expect(items.nth(i)).toHaveAttribute('data-shimmer-state', 'resolved')
    }
  })

  test('issue 验收预算：默认 fixture 启动 5 s 内 shimmer 元素 < 5', async ({ page }) => {
    // 默认 fixture（不带 pendingMetadataDelayMs）下 sessions 直接 resolved，
    // 用例锁住"无 lag 场景下 shimmer 完全静默"——回归阈值改小或回到旧逻辑
    // 都会让本断言挂。
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await selectRustProject(page)
    await expect(page.locator('.session-item').first()).toBeVisible({ timeout: 5_000 })

    // 连续轮询 5 s，记录 shimmer 元素峰值数
    const start = Date.now()
    let peak = 0
    while (Date.now() - start < 5_000) {
      const n = await page.locator('.session-item.metadata-pending').count()
      if (n > peak) peak = n
      await page.waitForTimeout(150)
    }
    expect(peak).toBeLessThan(5)
  })
})
