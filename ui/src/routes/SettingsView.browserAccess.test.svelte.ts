// Browser Access subsection 单测——覆盖 server-mode settings UI 5 个 Scenario
// （详 openspec/specs/settings-ui/spec.md::General Section SHALL render
// Browser Access subsection in Tauri runtime）。
//
// 走 setupMockIPC 注入 http_server_start / _stop / _status mock；mockIPC 会
// 同时注入 `__TAURI_INTERNALS__`，让 isTauriRuntime() === true，从而触发
// "Tauri runtime 渲染 Browser Access" 分支（详 ui/CLAUDE.md::tauriMock
// 注入 __TAURI_INTERNALS__）。

import { describe, expect, test, afterEach, beforeEach, vi } from 'vitest'
import { render, cleanup, fireEvent, waitFor } from '@testing-library/svelte'
import { clearMocks } from '@tauri-apps/api/mocks'

import SettingsView from './SettingsView.svelte'
import { setupMockIPC, getActiveFixtureRef } from '../lib/tauriMock'
import { emptyFixture } from '../lib/__fixtures__/empty'

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}

beforeEach(() => {
  // fixture 在 module-level 单例（详 ui/CLAUDE.md::Playwright reuseExistingServer
  // 同源问题），测试间显式 reset server-mode 相关字段避免串状态
  emptyFixture.mockHttpServer = null
  emptyFixture.config.httpServer = { enabled: false, port: 3456 }
  setupMockIPC('empty')
  vi.stubGlobal('ResizeObserver', ResizeObserverStub)
  // jsdom 不实现 clipboard——stub 一个 spy
  Object.assign(navigator, {
    clipboard: { writeText: vi.fn().mockResolvedValue(undefined) },
  })
})

afterEach(() => {
  cleanup()
  clearMocks()
  vi.unstubAllGlobals()
})

describe('SettingsView Browser Access subsection', () => {
  test('Tauri runtime 渲染 Browser Access toggle + 端口输入', async () => {
    const { container } = render(SettingsView)
    // 等 onMount 完成
    await waitFor(() => {
      expect(container.querySelector('[data-testid="browser-access-port"]')).not.toBeNull()
    })
    // toggle 默认 off（getHttpServerStatus 返回 running=false）
    const status = container.querySelector('[data-testid="browser-access-running"]')
    expect(status).toBeNull()
    const portInput = container.querySelector(
      '[data-testid="browser-access-port"]',
    ) as HTMLInputElement
    expect(portInput.value).toBe('3456')
  })

  test('点 toggle 启动 server → 显示运行中标识 + URL + Copy 按钮', async () => {
    const { container } = render(SettingsView)
    await waitFor(() =>
      expect(container.querySelector('[data-testid="browser-access-port"]')).not.toBeNull(),
    )
    const toggle = container.querySelector('[aria-label="启用浏览器访问"]') as HTMLButtonElement
    expect(toggle).not.toBeNull()
    expect(toggle.getAttribute('aria-checked')).toBe('false')

    await fireEvent.click(toggle)

    await waitFor(() => {
      const running = container.querySelector('[data-testid="browser-access-running"]')
      expect(running).not.toBeNull()
      expect(running!.textContent).toMatch(/运行中/)
      expect(running!.textContent).toMatch(/3456/)
    })
    // toggle 翻成 on
    expect(toggle.getAttribute('aria-checked')).toBe('true')
  })

  test('Copy URL 按钮点击写剪贴板并显示 Copied', async () => {
    const writeText = (navigator.clipboard.writeText as ReturnType<typeof vi.fn>)
    const { container } = render(SettingsView)
    await waitFor(() =>
      expect(container.querySelector('[data-testid="browser-access-port"]')).not.toBeNull(),
    )
    const toggle = container.querySelector('[aria-label="启用浏览器访问"]') as HTMLButtonElement
    await fireEvent.click(toggle)
    await waitFor(() =>
      expect(container.querySelector('[data-testid="browser-access-copy"]')).not.toBeNull(),
    )
    const copyBtn = container.querySelector(
      '[data-testid="browser-access-copy"]',
    ) as HTMLButtonElement
    await fireEvent.click(copyBtn)
    expect(writeText).toHaveBeenCalledWith('http://localhost:3456')
    await waitFor(() => expect(copyBtn.textContent?.trim()).toBe('已复制'))
  })

  test('端口输入 blur 持久化到 httpServer 配置', async () => {
    const { container } = render(SettingsView)
    await waitFor(() =>
      expect(container.querySelector('[data-testid="browser-access-port"]')).not.toBeNull(),
    )
    const portInput = container.querySelector(
      '[data-testid="browser-access-port"]',
    ) as HTMLInputElement
    await fireEvent.input(portInput, { target: { value: '3500' } })
    await fireEvent.blur(portInput)

    await waitFor(() => {
      // setupMockIPC `structuredClone` fixture 后 IPC handler 持有的是副本而非
      // `emptyFixture` 模块原对象，断言走 `getActiveFixtureRef()` 拿副本。
      expect(getActiveFixtureRef()?.config.httpServer?.port).toBe(3500)
    })
  })

  test('运行中锁定端口输入并提示关闭后修改', async () => {
    const { container } = render(SettingsView)
    await waitFor(() =>
      expect(container.querySelector('[data-testid="browser-access-port"]')).not.toBeNull(),
    )
    const toggle = container.querySelector('[aria-label="启用浏览器访问"]') as HTMLButtonElement
    await fireEvent.click(toggle)

    await waitFor(() => {
      const portInput = container.querySelector(
        '[data-testid="browser-access-port"]',
      ) as HTMLInputElement
      expect(portInput.disabled).toBe(true)
      expect(container.querySelector('[data-testid="browser-access-port-locked"]')?.textContent).toMatch(
        /已锁定/,
      )
    })
  })

  test('启动失败（非法端口）→ inline 错误展示且不自动消失', async () => {
    const { container } = render(SettingsView)
    await waitFor(() =>
      expect(container.querySelector('[data-testid="browser-access-port"]')).not.toBeNull(),
    )
    const portInput = container.querySelector(
      '[data-testid="browser-access-port"]',
    ) as HTMLInputElement
    await fireEvent.input(portInput, { target: { value: '80' } })

    const toggle = container.querySelector('[aria-label="启用浏览器访问"]') as HTMLButtonElement
    await fireEvent.click(toggle)

    await waitFor(() => {
      const err = container.querySelector('[data-testid="browser-access-error"]')
      expect(err).not.toBeNull()
      expect(err!.textContent).toMatch(/1024|端口|range/i)
    })
    // toggle 保持 off
    expect(toggle.getAttribute('aria-checked')).toBe('false')
    // 错误不会在本次失败路径中被自动清掉
    const err = container.querySelector('[data-testid="browser-access-error"]')
    expect(err).not.toBeNull()
    // 持久化没被改（未走到 IPC 成功路径）
    const running = container.querySelector('[data-testid="browser-access-running"]')
    expect(running).toBeNull()
  })

  test('开 → 关 → 状态行消失', async () => {
    const { container } = render(SettingsView)
    await waitFor(() =>
      expect(container.querySelector('[data-testid="browser-access-port"]')).not.toBeNull(),
    )
    const toggle = container.querySelector('[aria-label="启用浏览器访问"]') as HTMLButtonElement
    await fireEvent.click(toggle)
    await waitFor(() =>
      expect(container.querySelector('[data-testid="browser-access-running"]')).not.toBeNull(),
    )

    await waitFor(() => {
      const currentToggle = container.querySelector('[aria-label="启用浏览器访问"]') as HTMLButtonElement
      expect(currentToggle.getAttribute('aria-checked')).toBe('true')
      expect(currentToggle.disabled).toBe(false)
    })
    const currentToggle = container.querySelector('[aria-label="启用浏览器访问"]') as HTMLButtonElement
    await fireEvent.click(currentToggle)
    await waitFor(() => {
      const latestToggle = container.querySelector('[aria-label="启用浏览器访问"]') as HTMLButtonElement
      expect(container.querySelector('[data-testid="browser-access-running"]')).toBeNull()
      expect(latestToggle.getAttribute('aria-checked')).toBe('false')
    })
  })
})

describe('SettingsView Browser Access subsection: browser runtime hides section', () => {
  // 单独 describe：本组测试明确删除 __TAURI_INTERNALS__ 模拟纯浏览器 runtime
  // （tauriMock 默认会注入；这里在 setupMockIPC 之后手动撤掉）
  beforeEach(() => {
    // mockIPC 注入了；删掉模拟浏览器
    delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
  })

  test('浏览器 runtime 不渲染 Browser Access section', async () => {
    const { container } = render(SettingsView)
    // 等 onMount 跑完
    await waitFor(() => {
      // 仍有 General section 内其他控件（如 themeDropdown）渲染——确保 SettingsView 没整体白屏
      // 主断言：Browser Access 相关 data-testid 不存在
      expect(container.querySelector('[data-testid="browser-access-port"]')).toBeNull()
      expect(container.querySelector('[aria-label="启用浏览器访问"]')).toBeNull()
    })
  })
})
