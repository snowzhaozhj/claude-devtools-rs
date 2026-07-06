// 数据根目录 MRU 快速切换下拉单测（change flexible-data-root）。
// 覆盖 settings-ui spec::General Section 展示 的两个 Scenario：
//   - 从历史快速切换数据根（recentRoots 非空渲染下拉）
//   - 无历史时不阻塞手动输入（recentRoots 空不渲染下拉）
//
// 走 setupMockIPC 注入 getConfig（含 recentRoots）；mockIPC structuredClone
// fixture，改 emptyFixture 后 SHALL 重新 setupMockIPC 让 handler 拿新副本
// （详 ui/CLAUDE.md::tauriMock）。

import { describe, expect, test, afterEach, beforeEach, vi } from 'vitest'
import { render, cleanup, waitFor } from '@testing-library/svelte'
import { clearMocks } from '@tauri-apps/api/mocks'

import SettingsView from './SettingsView.svelte'
import { setupMockIPC } from '../lib/tauriMock'
import { emptyFixture } from '../lib/__fixtures__/empty'

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}

const ROOT_INPUT = '#claude-root-input'
const MRU = '[aria-label="最近使用的数据根目录"]'

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

describe('SettingsView 数据根目录 MRU 下拉', () => {
  test('recentRoots 非空 → 渲染最近使用快速切换下拉', async () => {
    emptyFixture.config.general.claudeRootPath = '~/.qoder'
    emptyFixture.config.general.recentRoots = ['/data/alpha', '~/.qoder']
    setupMockIPC('empty')

    const { container } = render(SettingsView)
    await waitFor(() => {
      expect(container.querySelector(ROOT_INPUT)).not.toBeNull()
    })
    expect(container.querySelector(MRU)).not.toBeNull()
  })

  test('recentRoots 空 → 不渲染下拉，手动输入入口仍在', async () => {
    emptyFixture.config.general.recentRoots = []
    setupMockIPC('empty')

    const { container } = render(SettingsView)
    await waitFor(() => {
      expect(container.querySelector(ROOT_INPUT)).not.toBeNull()
    })
    expect(container.querySelector(MRU)).toBeNull()
  })
})
