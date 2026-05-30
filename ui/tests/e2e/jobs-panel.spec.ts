// User story: Background Jobs Panel
//
// Spec: openspec/changes/bg-jobs-panel/design.md
// 覆盖：TitleBar 入口 / 分组显示 / 展开详情 / 空态 / 降级隐藏

import { expect, test } from '@playwright/test'

async function gotoWithMockReady(page: import('@playwright/test').Page, params = '') {
  await page.goto(`/?mock=1&fixture=multi-project-rich${params}`)
  await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })
}

test.describe('Background Jobs Panel', () => {
  test('点击 TitleBar jobs icon → 打开 Jobs tab', async ({ page }) => {
    await gotoWithMockReady(page)

    // jobs icon 应在 TitleBar 可见（mock 默认 jobsDirExists=true）
    const jobsBtn = page.getByRole('button', { name: '后台任务' })
    await expect(jobsBtn).toBeVisible({ timeout: 5_000 })

    // 点击打开 Jobs tab
    await jobsBtn.click()

    // 应看到 Jobs 视图标题
    await expect(page.getByRole('heading', { name: 'Background Jobs' })).toBeVisible({
      timeout: 5_000,
    })
  })

  test('Jobs 视图显示分组（Working / Needs input / Completed）', async ({ page }) => {
    await gotoWithMockReady(page)

    // 通过 __cdtTest 直接打开 Jobs tab
    await page.evaluate(() => {
      ;(window as unknown as { __cdtTest: { openJobsTab: () => void } }).__cdtTest.openJobsTab()
    })

    await expect(page.getByRole('heading', { name: 'Background Jobs' })).toBeVisible({
      timeout: 5_000,
    })

    // mock 数据包含 working / blocked / done+PR / failed / idle
    // 验证分组标签可见
    await expect(page.getByText('Ready for review')).toBeVisible()
    await expect(page.getByText('Needs input')).toBeVisible()
    await expect(page.getByText('Working')).toBeVisible()
    await expect(page.getByText('Completed')).toBeVisible()
  })

  test('展开 job row 看到 intent + 操作按钮', async ({ page }) => {
    await gotoWithMockReady(page)

    await page.evaluate(() => {
      ;(window as unknown as { __cdtTest: { openJobsTab: () => void } }).__cdtTest.openJobsTab()
    })

    await expect(page.getByRole('heading', { name: 'Background Jobs' })).toBeVisible({
      timeout: 5_000,
    })

    // 找到 working job 行（feat/add-auth-flow）
    const jobRow = page.locator('.job-row').filter({ hasText: 'feat/add-auth-flow' })
    await expect(jobRow).toBeVisible()

    // 点击 chevron 展开
    await jobRow.locator('.chevron').click()

    // 展开后应看到 intent 文本
    await expect(page.getByText('Implement OAuth2 authentication flow')).toBeVisible()

    // 应看到"打开 session"操作按钮
    await expect(jobRow.getByRole('button', { name: /打开 session/ })).toBeVisible()

    // Working 组行应有 Stop 按钮
    await expect(jobRow.getByRole('button', { name: 'Stop' })).toBeVisible()
  })

  test('PR chip 可见 + Ready for review 组包含 Review PR 按钮', async ({ page }) => {
    await gotoWithMockReady(page)

    await page.evaluate(() => {
      ;(window as unknown as { __cdtTest: { openJobsTab: () => void } }).__cdtTest.openJobsTab()
    })

    await expect(page.getByRole('heading', { name: 'Background Jobs' })).toBeVisible({
      timeout: 5_000,
    })

    // feat/dashboard-redesign 有 PR child → Ready for review 组
    const prRow = page.locator('.job-row').filter({ hasText: 'feat/dashboard-redesign' })
    await expect(prRow).toBeVisible()

    // PR chip 可见
    await expect(prRow.locator('.pr-chip')).toBeVisible()

    // 展开看到 Review PR 按钮
    await prRow.locator('.chevron').click()
    await expect(prRow.getByRole('button', { name: /Review PR/ })).toBeVisible()
  })

  test('jobsDirExists=false 时 TitleBar icon 隐藏', async ({ page }) => {
    // ?jobs=none 让 mock 返回错误（模拟 jobs 目录不存在）
    await gotoWithMockReady(page, '&jobs=none')

    // 等页面加载完，确认其他 icon 可见（说明 TitleBar 已渲染）
    await expect(page.getByRole('button', { name: '通知' })).toBeVisible({ timeout: 5_000 })

    // jobs icon 不可见
    await expect(page.getByRole('button', { name: '后台任务' })).not.toBeVisible()
  })

  test('空列表时显示 "No background jobs"', async ({ page }) => {
    // ?jobs=empty 让 mock 返回空列表但 jobsDirExists=true
    await gotoWithMockReady(page, '&jobs=empty')

    await page.evaluate(() => {
      ;(window as unknown as { __cdtTest: { openJobsTab: () => void } }).__cdtTest.openJobsTab()
    })

    await expect(page.getByRole('heading', { name: 'Background Jobs' })).toBeVisible({
      timeout: 5_000,
    })

    // 空态文案可见
    await expect(page.getByText('No background jobs')).toBeVisible()
  })

  test('Badge 色点按优先级显示（mock 含 failed → 红点）', async ({ page }) => {
    await gotoWithMockReady(page)

    // mock 默认有 failed job → badge 应为红色
    const jobsBtn = page.getByRole('button', { name: '后台任务' })
    await expect(jobsBtn).toBeVisible({ timeout: 5_000 })

    // 红色 badge 点应可见
    await expect(jobsBtn.locator('.badge-red')).toBeVisible()
  })
})
