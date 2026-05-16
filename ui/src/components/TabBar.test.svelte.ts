// TabBar smoke 单测：依赖 tabStore（默认 pane-default）+ getNotifications IPC
// + getCurrentWindow（Tauri window mock）。用 setupMockIPC 一把铺平后端。

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
  test('给定默认 pane id 可渲染 tab-bar + 通知/设置按钮', () => {
    const { container } = render(TabBar, {
      props: {
        paneId: 'pane-default',
        isFirstPane: true,
      },
    })
    expect(container.querySelector('.tab-bar')).not.toBeNull()
    const actionBtns = container.querySelectorAll('.tab-actions .tab-action-btn')
    // 通知 + 设置
    expect(actionBtns.length).toBe(2)
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
