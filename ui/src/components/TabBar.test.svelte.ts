// TabBar smoke 单测：依赖 tabStore（默认 pane-default）+ getCurrentWindow
// （Tauri window mock）。通知 / 设置按钮自 change `unified-title-bar` 起移到
// `UnifiedTitleBar` 的 status zone，TabBar 不再承载（详见 app-chrome spec）。

import { describe, expect, test, afterEach, beforeEach } from 'vitest'
import { render, cleanup } from '@testing-library/svelte'
import { clearMocks } from '@tauri-apps/api/mocks'

import TabBar from './TabBar.svelte'
import { setupMockIPC } from '../lib/tauriMock'
import { openSettingsTab, getPaneLayout, closeTab } from '../lib/tabStore.svelte'

beforeEach(() => {
  setupMockIPC('multi-project-rich')
})

afterEach(() => {
  // 清理 tabStore 中可能残留的 tab，避免相互污染
  const pane = getPaneLayout().panes[0]
  for (const t of [...pane.tabs]) {
    closeTab(t.id)
  }
  cleanup()
  clearMocks()
})

describe('TabBar smoke', () => {
  test('给定默认 pane id 可渲染 tab-bar，且不再承载通知/设置按钮', () => {
    const { container } = render(TabBar, {
      props: {
        paneId: 'pane-default',
        isFirstPane: true,
      },
    })
    expect(container.querySelector('.tab-bar')).not.toBeNull()
    // chrome 接管 status zone 后，TabBar 内 SHALL NOT 再有 .tab-actions
    expect(container.querySelector('.tab-actions')).toBeNull()
  })

  test('打开 settings tab 后 tab-list 含一个 tab-item', () => {
    openSettingsTab()
    const { container } = render(TabBar, {
      props: {
        paneId: 'pane-default',
        isFirstPane: true,
      },
    })
    const items = container.querySelectorAll('.tab-list .tab-item')
    expect(items.length).toBe(1)
    expect(items[0].getAttribute('data-pane-id')).toBe('pane-default')
  })

  test('未知 paneId 不抛错，tab-list 渲染为空', () => {
    const { container } = render(TabBar, {
      props: {
        paneId: 'pane-does-not-exist',
        isFirstPane: false,
      },
    })
    expect(container.querySelector('.tab-bar')).not.toBeNull()
    expect(container.querySelectorAll('.tab-list .tab-item').length).toBe(0)
  })
})
