// App root switch coordinator 集成测试（change redesign-data-root-switcher）。
// 覆盖：root 切换成功事件后关闭 root-scoped tabs，保留 Settings 等
// 非 root-scoped 工作台 tab，并只触发一次当前 root project/group 刷新。

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { render, cleanup, waitFor } from '@testing-library/svelte'
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks'

import App from './App.svelte'
import { setupMockIPC } from './lib/tauriMock'
import { getAllTabs, openMemoryTab, openSettingsTab, openTab, getPaneLayout } from './lib/tabStore.svelte'
import type { InvokeArgs } from '@tauri-apps/api/core'

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}

beforeEach(() => {
  vi.stubGlobal('ResizeObserver', ResizeObserverStub)
  setupMockIPC('multi-project-rich')
})

afterEach(() => {
  cleanup()
  clearMocks()
  vi.unstubAllGlobals()
})

describe('App data root switch coordinator', () => {
  test('cdt-data-root-changed 清 root-scoped tabs，保留 Settings tab，并只刷新一次 project data', async () => {
    render(App)
    await waitFor(() => expect((window as unknown as { __cdtReady?: boolean }).__cdtReady).toBe(true))

    openSettingsTab()
    openTab('sess-root-app', 'mock-rich-rust', 'Root App')
    openMemoryTab('mock-rich-rust')
    expect(getAllTabs().some((t) => t.type === 'session' || t.type === 'memory')).toBe(true)

    clearMocks()
    let listRepositoryGroupsCalls = 0
    mockIPC((cmd: string, _args?: InvokeArgs): unknown => {
      if (cmd === 'list_repository_groups') {
        listRepositoryGroupsCalls += 1
        return []
      }
      if (cmd === 'list_projects') return []
      if (cmd === 'get_config') return {
        general: { theme: 'system', sessionClickBehavior: 'replace', claudeRootPath: null, recentRoots: [] },
        display: { timeFormat: '24h' },
        notifications: { enabled: true },
        _version: 0,
      }
      if (cmd === 'list_available_terminals') return ['terminal']
      if (cmd === 'get_http_server_status') return { running: false, port: 3456, lastError: null }
      if (cmd === 'list_jobs') return []
      if (cmd === 'get_notification_summary') return { unreadCount: 0 }
      if (cmd === 'get_cli_status') return { installed: false, path: null, version: null }
      if (cmd === 'list_notifications') return []
      if (cmd === 'list_agent_configs') return []
      if (cmd === 'is_running_under_rosetta') return false
      return null
    }, { shouldMockEvents: true })

    window.dispatchEvent(new CustomEvent('cdt-data-root-changed'))

    await waitFor(() => {
      const tabs = getAllTabs()
      expect(tabs.some((t) => t.type === 'session' || t.type === 'memory')).toBe(false)
      expect(tabs.some((t) => t.type === 'settings')).toBe(true)
      const layout = getPaneLayout()
      const activeTab = tabs.find((t) => t.id === layout.panes[0].activeTabId)
      expect(activeTab?.type).toBe('settings')
    })
    await waitFor(() => expect(listRepositoryGroupsCalls).toBe(1))
  })
})
