// externalLinks 单测：覆盖外链拦截 / 内部锚点放行 / 修饰键与中键拦截 / 右键放行。
//
// 注：Tauri webview 不支持多标签，修饰键 + 中键 click 默认行为同样陷入窗口内
// 导航，因此一律拦截走 openUrl（codex review bug 1/2）。mockIPC 浏览器调试模式
// 下 plugin:opener|open_url 由 tauriMock 拦截到 window.open，外部观察是
// openUrl 被调用即可（mockIPC 链路属 tauriMock 自身测试范围）。

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

const openUrlMock = vi.fn<(url: string) => Promise<void>>()

vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: (url: string) => openUrlMock(url),
}))

import { attachExternalLinkInterceptor } from './externalLinks'

let detach: (() => void) | null = null

beforeEach(() => {
  openUrlMock.mockReset()
  openUrlMock.mockResolvedValue(undefined)
  document.body.innerHTML = ''
  detach = attachExternalLinkInterceptor()
})

afterEach(() => {
  detach?.()
  detach = null
})

function clickAnchor(href: string, init: MouseEventInit = {}): MouseEvent {
  const a = document.createElement('a')
  a.href = href
  a.textContent = 'link'
  document.body.appendChild(a)
  const event = new MouseEvent('click', { bubbles: true, cancelable: true, button: 0, ...init })
  a.dispatchEvent(event)
  return event
}

describe('attachExternalLinkInterceptor', () => {
  test('intercepts http external link and routes to openUrl', () => {
    const event = clickAnchor('https://example.com/foo')
    expect(event.defaultPrevented).toBe(true)
    expect(openUrlMock).toHaveBeenCalledTimes(1)
    expect(openUrlMock).toHaveBeenCalledWith('https://example.com/foo')
  })

  test('intercepts mailto link', () => {
    const event = clickAnchor('mailto:foo@bar.com')
    expect(event.defaultPrevented).toBe(true)
    expect(openUrlMock).toHaveBeenCalledWith('mailto:foo@bar.com')
  })

  test('lets in-page anchor pass through', () => {
    const event = clickAnchor('#section-1')
    expect(event.defaultPrevented).toBe(false)
    expect(openUrlMock).not.toHaveBeenCalled()
  })

  test('intercepts modifier-key clicks (Tauri 无多标签 fallback)', () => {
    const event = clickAnchor('https://example.com/x', { metaKey: true })
    expect(event.defaultPrevented).toBe(true)
    expect(openUrlMock).toHaveBeenCalledWith('https://example.com/x')
  })

  test('intercepts middle-click on external link', () => {
    const event = clickAnchor('https://example.com/x', { button: 1 })
    expect(event.defaultPrevented).toBe(true)
    expect(openUrlMock).toHaveBeenCalledWith('https://example.com/x')
  })

  test('lets right-click pass through (browser fires contextmenu separately)', () => {
    const event = clickAnchor('https://example.com/x', { button: 2 })
    expect(event.defaultPrevented).toBe(false)
    expect(openUrlMock).not.toHaveBeenCalled()
  })

  test('detach unregisters the listener', () => {
    detach?.()
    detach = null
    const event = clickAnchor('https://example.com')
    expect(event.defaultPrevented).toBe(false)
    expect(openUrlMock).not.toHaveBeenCalled()
  })
})
