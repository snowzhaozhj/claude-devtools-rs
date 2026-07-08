// 数据根目录轻量 source switcher 单测（change redesign-data-root-switcher）。
// 覆盖 settings-ui spec::General Section 展示：当前路径展示、最近列表过滤、
// 最近为空隐藏、输入路径原地展开、取消与错误态。

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { render, cleanup, waitFor, fireEvent } from '@testing-library/svelte'
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks'

import SettingsView from './SettingsView.svelte'
import { setupMockIPC } from '../lib/tauriMock'
import { emptyFixture } from '../lib/__fixtures__/empty'

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}

beforeEach(() => {
  emptyFixture.config.general.claudeRootPath = null
  emptyFixture.config.general.recentRoots = []
  vi.stubGlobal('ResizeObserver', ResizeObserverStub)
})

afterEach(() => {
  cleanup()
  clearMocks()
  vi.unstubAllGlobals()
})

describe('SettingsView 数据目录 source switcher', () => {
  test('默认 root 只显示一次，并以低权重状态标记为默认', async () => {
    emptyFixture.config.general.claudeRootPath = null
    emptyFixture.config.general.recentRoots = ['~/.claude']
    setupMockIPC('empty')

    const { container, getByText, queryByLabelText } = render(SettingsView)
    await waitFor(() => expect(getByText('~/.claude')).toBeTruthy())

    expect(getByText('默认')).toBeTruthy()
    expect(queryByLabelText('最近使用的数据根目录')).toBeNull()
    expect(container.querySelectorAll('.data-root-path')).toHaveLength(1)
  })

  test('自定义 root 显示为自定义，并把 ~/.claude 作为默认切换项', async () => {
    emptyFixture.config.general.claudeRootPath = '~/.qoder'
    emptyFixture.config.general.recentRoots = ['/data/alpha', '~/.qoder', '/data/beta']
    setupMockIPC('empty')

    const { getByText, queryByText, getByLabelText } = render(SettingsView)
    await waitFor(() => expect(getByText('~/.qoder')).toBeTruthy())

    expect(getByText('自定义')).toBeTruthy()
    expect(getByLabelText('最近使用的数据根目录')).toBeTruthy()
    expect(getByText('~/.claude')).toBeTruthy()
    expect(getByText('/data/alpha')).toBeTruthy()
    expect(getByText('/data/beta')).toBeTruthy()
    // 当前路径只在 current row 出现，不在“最近”列表重复出现。
    expect(queryByText('~/.qoder')).toBeTruthy()
    expect(document.querySelectorAll('.data-root-recent-path[title="~/.qoder"]')).toHaveLength(0)
  })

  test('已有 ~/.claude 字符串配置按默认 root 展示，不出现同名默认切换项', async () => {
    emptyFixture.config.general.claudeRootPath = '~/.claude'
    emptyFixture.config.general.recentRoots = ['~/.claude', '/data/alpha']
    setupMockIPC('empty')

    const { container, getByText, queryByLabelText, queryByText } = render(SettingsView)
    await waitFor(() => expect(getByText('~/.claude')).toBeTruthy())

    expect(getByText('默认')).toBeTruthy()
    expect(queryByText('自定义')).toBeNull()
    expect(queryByLabelText('恢复默认数据根目录')).toBeNull()
    expect(getByText('/data/alpha')).toBeTruthy()
    expect(container.querySelectorAll('.data-root-recent-path[title="~/.claude"]')).toHaveLength(0)
  })

  test('默认 root 手动输入 ~/.claude 会归一化为 no-op，不保存自定义字符串', async () => {
    emptyFixture.config.general.claudeRootPath = null
    setupMockIPC('empty')
    let rootSwitchEvents = 0
    const onRootSwitch = () => { rootSwitchEvents += 1 }
    window.addEventListener('cdt-data-root-changed', onRootSwitch)

    try {
      const { getByText, getByLabelText, queryByLabelText } = render(SettingsView)
      await waitFor(() => expect(getByText('输入')).toBeTruthy())

      await fireEvent.click(getByText('输入'))
      const input = getByLabelText('输入数据根目录路径') as HTMLInputElement
      await fireEvent.input(input, { target: { value: '~/.claude/' } })
      await fireEvent.click(getByText('应用'))

      await waitFor(() => expect(queryByLabelText('输入数据根目录路径')).toBeNull())
      expect(getByText('默认')).toBeTruthy()
      expect(rootSwitchEvents).toBe(0)
      expect(emptyFixture.config.general.claudeRootPath).toBeNull()
    } finally {
      window.removeEventListener('cdt-data-root-changed', onRootSwitch)
    }
  })

  test('输入路径按钮原地展开，Esc 取消恢复按钮行', async () => {
    emptyFixture.config.general.claudeRootPath = '~/.qoder'
    setupMockIPC('empty')

    const { getByText, getByLabelText, queryByLabelText } = render(SettingsView)
    await waitFor(() => expect(getByText('输入')).toBeTruthy())

    await fireEvent.click(getByText('输入'))
    const input = getByLabelText('输入数据根目录路径') as HTMLInputElement
    expect(input.value).toBe('~/.qoder')
    expect(getByText('应用')).toBeTruthy()

    await fireEvent.keyDown(input, { key: 'Escape' })
    expect(queryByLabelText('输入数据根目录路径')).toBeNull()
    expect(getByText('输入')).toBeTruthy()
  })

  test('保存失败不会 dispatch root switch 事件', async () => {
    emptyFixture.config.general.claudeRootPath = '~/.qoder'
    let rootSwitchEvents = 0
    const onRootSwitch = () => { rootSwitchEvents += 1 }
    window.addEventListener('cdt-data-root-changed', onRootSwitch)
    mockIPC(vi.fn((cmd) => {
      if (cmd === 'get_config') return { ...emptyFixture.config, _version: 0 }
      if (cmd === 'get_version') return '0.0.0-test'
      if (cmd === 'list_available_terminals') return ['terminal']
      if (cmd === 'get_http_server_status') return { running: false, port: 3456, lastError: null }
      if (cmd === 'update_config') throw new Error('invalid root')
      throw new Error(`unexpected command: ${cmd}`)
    }))

    try {
      const { getByText, getByLabelText } = render(SettingsView)
      await waitFor(() => expect(getByText('输入')).toBeTruthy())

      await fireEvent.click(getByText('输入'))
      const input = getByLabelText('输入数据根目录路径') as HTMLInputElement
      await fireEvent.input(input, { target: { value: 'relative/path' } })
      await fireEvent.click(getByText('应用'))

      await waitFor(() => expect(getByText(/保存失败/)).toBeTruthy())
      expect(getByLabelText('输入数据根目录路径')).toBeTruthy()
      expect(rootSwitchEvents).toBe(0)
    } finally {
      window.removeEventListener('cdt-data-root-changed', onRootSwitch)
    }
  })
})
