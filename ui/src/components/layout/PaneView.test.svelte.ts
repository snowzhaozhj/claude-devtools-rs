// PaneView 条件渲染单测：sole pane + 无 tab 时（= Dashboard 工作台）SHALL NOT
// 渲染 TabBar——chrome 重构后通知/设置已迁到 UnifiedTitleBar，留下 40px 空横条
// 与底 border 切线会在 chrome 与 Dashboard 搜索框之间制造无意义空白带。
// 多 pane 即便 tabs 为空仍渲染 TabBar：承载 focus accent indicator + drop zone。

import { describe, expect, test, afterEach, beforeEach } from 'vitest'
import { render, cleanup } from '@testing-library/svelte'
import { clearMocks } from '@tauri-apps/api/mocks'

import PaneView from './PaneView.svelte'
import { setupMockIPC } from '../../lib/tauriMock'
import type { Pane } from '../../lib/paneTypes'

beforeEach(() => {
  setupMockIPC('multi-project-rich')
})

afterEach(() => {
  cleanup()
  clearMocks()
})

function emptyPane(id = 'p-test'): Pane {
  return { id, tabs: [], activeTabId: null, widthFraction: 1 }
}

describe('PaneView 条件渲染 TabBar', () => {
  test('sole pane + 无 tab：不渲染 TabBar（让 Dashboard 直接贴 chrome 下沿）', () => {
    const { container } = render(PaneView, {
      props: {
        pane: emptyPane(),
        selectedProjectId: '',
        onSelectProject: () => {},
        isSolePane: true,
        isFirstPane: true,
      },
    })
    expect(container.querySelector('.tab-bar')).toBeNull()
  })

  test('多 pane + 无 tab：仍渲染 TabBar（承载 focus indicator + drop zone）', () => {
    const { container } = render(PaneView, {
      props: {
        pane: emptyPane(),
        selectedProjectId: '',
        onSelectProject: () => {},
        isSolePane: false,
        isFirstPane: false,
      },
    })
    expect(container.querySelector('.tab-bar')).not.toBeNull()
  })
})
