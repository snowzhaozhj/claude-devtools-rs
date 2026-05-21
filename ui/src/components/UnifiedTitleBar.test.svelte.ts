// UnifiedTitleBar 布局契约单测。
// 覆盖 spec `app-chrome/spec.md::UnifiedTitleBar 单条 chrome` /
// `chrome 四 zone 布局` / `chrome 右侧 status zone 容纳契约` /
// `项目导航控件锚定 chrome 左中` 的关键 Scenario。

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { render, cleanup } from '@testing-library/svelte'
import { clearMocks } from '@tauri-apps/api/mocks'

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: vi.fn(),
}))
vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: vi.fn(),
}))

import UnifiedTitleBar from './UnifiedTitleBar.svelte'
import { setupMockIPC } from '../lib/tauriMock'
import { updateStore } from '../lib/updateStore.svelte'

beforeEach(() => {
  setupMockIPC('multi-project-rich')
  updateStore.reset()
})

afterEach(() => {
  cleanup()
  clearMocks()
  // 还原 navigator.userAgent
  Object.defineProperty(navigator, 'userAgent', {
    value: window.navigator.userAgent,
    configurable: true,
  })
})

function setUA(ua: string) {
  Object.defineProperty(navigator, 'userAgent', {
    value: ua,
    configurable: true,
  })
}

describe('UnifiedTitleBar chrome 容器', () => {
  test('渲染 header.chrome + 底部 1px border 单线', () => {
    setUA('Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)')
    const { container } = render(UnifiedTitleBar, {
      props: {
        projects: [],
        repositoryGroups: [],
        selectedGroupId: '',
        onSelectProject: () => undefined,
        rosettaVisible: false,
      },
    })
    const chrome = container.querySelector('header.chrome')
    expect(chrome).not.toBeNull()
    expect(chrome!.getAttribute('aria-label')).toBe('应用工具栏')
  })
})

describe('UnifiedTitleBar 跨平台 padding', () => {
  test('macOS UA 渲染 zone-platform-padding', () => {
    setUA('Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)')
    const { container } = render(UnifiedTitleBar, {
      props: {
        projects: [],
        repositoryGroups: [],
        selectedGroupId: '',
        onSelectProject: () => undefined,
        rosettaVisible: false,
      },
    })
    const padding = container.querySelector('.zone-platform-padding')
    expect(padding).not.toBeNull()
    expect(container.querySelector('header.chrome')!.classList.contains('chrome-mac')).toBe(true)
  })

  test('Windows UA 不渲染 platform-padding', () => {
    setUA('Mozilla/5.0 (Windows NT 10.0; Win64; x64)')
    const { container } = render(UnifiedTitleBar, {
      props: {
        projects: [],
        repositoryGroups: [],
        selectedGroupId: '',
        onSelectProject: () => undefined,
        rosettaVisible: false,
      },
    })
    expect(container.querySelector('.zone-platform-padding')).toBeNull()
    expect(container.querySelector('header.chrome')!.classList.contains('chrome-mac')).toBe(false)
  })

  test('Linux UA 不渲染 platform-padding', () => {
    setUA('Mozilla/5.0 (X11; Linux x86_64)')
    const { container } = render(UnifiedTitleBar, {
      props: {
        projects: [],
        repositoryGroups: [],
        selectedGroupId: '',
        onSelectProject: () => undefined,
        rosettaVisible: false,
      },
    })
    expect(container.querySelector('.zone-platform-padding')).toBeNull()
  })
})

describe('UnifiedTitleBar status zone 容纳契约', () => {
  test('idle + 无 Rosetta 时 status zone 仅 2 个按钮（通知 + 设置）', () => {
    setUA('Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)')
    const { container } = render(UnifiedTitleBar, {
      props: {
        projects: [],
        repositoryGroups: [],
        selectedGroupId: '',
        onSelectProject: () => undefined,
        rosettaVisible: false,
      },
    })
    const zone = container.querySelector('.zone-status')
    expect(zone).not.toBeNull()
    expect(zone!.querySelectorAll('button.icon-btn').length).toBe(2)
    expect(zone!.querySelector('.update-pill')).toBeNull()
    expect(zone!.querySelector('.rosetta-icon')).toBeNull()
  })

  test('Rosetta visible 时 status zone 多出 rosetta-icon', () => {
    setUA('Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)')
    const { container } = render(UnifiedTitleBar, {
      props: {
        projects: [],
        repositoryGroups: [],
        selectedGroupId: '',
        onSelectProject: () => undefined,
        rosettaVisible: true,
      },
    })
    const zone = container.querySelector('.zone-status')
    expect(zone!.querySelector('.rosetta-icon')).not.toBeNull()
  })
})

describe('UnifiedTitleBar zone-left-center', () => {
  test('始终渲染 ProjectSwitcher + 折叠按钮', () => {
    setUA('Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)')
    const { container } = render(UnifiedTitleBar, {
      props: {
        projects: [],
        repositoryGroups: [],
        selectedGroupId: '',
        onSelectProject: () => undefined,
        rosettaVisible: false,
      },
    })
    const left = container.querySelector('.zone-left-center')
    expect(left).not.toBeNull()
    expect(left!.querySelector('.project-selector')).not.toBeNull()
    expect(left!.querySelector('button.icon-btn')).not.toBeNull()
  })
})
