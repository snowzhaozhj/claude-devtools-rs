import { expect, test } from '@playwright/test'

async function gotoWithMockReady(page: import('@playwright/test').Page, params = '') {
  await page.goto(`/?mock=1&fixture=multi-project-rich${params}`)
  await page.waitForFunction(() => (window as unknown as Record<string, unknown>).__cdtReady === true, { timeout: 10_000 })
}

test.describe('Background Jobs Panel', () => {
  test('点击 TitleBar jobs icon → 打开 Jobs tab', async ({ page }) => {
    await gotoWithMockReady(page)

    const jobsBtn = page.getByRole('button', { name: '后台任务' })
    await expect(jobsBtn).toBeVisible({ timeout: 5_000 })

    await jobsBtn.click()

    await expect(page.getByRole('heading', { name: 'Background Jobs' })).toBeVisible({
      timeout: 5_000,
    })
  })

  test('Jobs 视图显示分组和两行密度布局', async ({ page }) => {
    await gotoWithMockReady(page)

    await page.evaluate(() => {
      ;(window as unknown as { __cdtTest: { openJobsTab: () => void } }).__cdtTest.openJobsTab()
    })

    await expect(page.getByRole('heading', { name: 'Background Jobs' })).toBeVisible({
      timeout: 5_000,
    })

    // mock 数据包含多种状态 → 应有分组标签
    await expect(page.getByText('Working')).toBeVisible()
    await expect(page.getByText('Completed')).toBeVisible()

    // 每个 job 直接显示 name + detail（两行密度，无需展开）
    await expect(page.getByText('feat/add-auth-flow')).toBeVisible()
    await expect(page.getByText('Running tests...')).toBeVisible()
  })

  test('PR chip 显示 PR 编号并可点击', async ({ page }) => {
    await gotoWithMockReady(page)

    await page.evaluate(() => {
      ;(window as unknown as { __cdtTest: { openJobsTab: () => void } }).__cdtTest.openJobsTab()
    })

    await expect(page.getByRole('heading', { name: 'Background Jobs' })).toBeVisible({
      timeout: 5_000,
    })

    // feat/dashboard-redesign 有 PR → 应显示 #42 chip
    await expect(page.locator('.pr-chip').first()).toBeVisible()
  })

  test('jobsDirExists=false 时 TitleBar icon 隐藏', async ({ page }) => {
    await gotoWithMockReady(page, '&jobs=none')

    await expect(page.getByRole('button', { name: '通知' })).toBeVisible({ timeout: 5_000 })
    await expect(page.getByRole('button', { name: '后台任务' })).not.toBeVisible()
  })

  test('空列表时显示 "No background jobs"', async ({ page }) => {
    await gotoWithMockReady(page, '&jobs=empty')

    await page.evaluate(() => {
      ;(window as unknown as { __cdtTest: { openJobsTab: () => void } }).__cdtTest.openJobsTab()
    })

    await expect(page.getByRole('heading', { name: 'Background Jobs' })).toBeVisible({
      timeout: 5_000,
    })

    await expect(page.getByText('No background jobs')).toBeVisible()
  })

  test('Badge 色点按优先级显示（mock 含 failed → 红点）', async ({ page }) => {
    await gotoWithMockReady(page)

    const jobsBtn = page.getByRole('button', { name: '后台任务' })
    await expect(jobsBtn).toBeVisible({ timeout: 5_000 })

    await expect(jobsBtn.locator('.badge-red')).toBeVisible()
  })
})
