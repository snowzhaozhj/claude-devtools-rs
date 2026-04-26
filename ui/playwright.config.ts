import { defineConfig, devices } from '@playwright/test'

// 对应 spec frontend-test-pyramid §"Playwright 必须覆盖最小 user story 集"。
// 只跑 chromium；webServer 复用 vite dev（reuseExistingServer 本地，CI 强制 fresh）。

export default defineConfig({
  testDir: './tests/e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: process.env.CI ? 2 : undefined,
  reporter: process.env.CI ? [['html'], ['line']] : 'html',
  timeout: 30_000,
  expect: {
    timeout: 5_000,
    toHaveScreenshot: {
      // 跨平台亚像素 diff 容忍：spec D5 决定不 commit baseline，
      // CI 上首次自动生成，diff 较大时上传 artifact 给人审。
      maxDiffPixelRatio: 0.02,
    },
  },
  use: {
    baseURL: 'http://localhost:5173',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'off',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: 'npm run dev',
    url: 'http://localhost:5173',
    reuseExistingServer: !process.env.CI,
    stdout: 'pipe',
    stderr: 'pipe',
    timeout: 60_000,
  },
})
