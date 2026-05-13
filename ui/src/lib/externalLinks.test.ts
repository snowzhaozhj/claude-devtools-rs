// externalLinks 单测：覆盖外链拦截 / 内部链接放行 / 修饰键放行 / 非主键放行。

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

const openUrlMock = vi.fn<(url: string) => Promise<void>>()

vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: (url: string) => openUrlMock(url),
}))

import { attachExternalLinkInterceptor } from './externalLinks'

let detach: (() => void) | null = null
let windowOpenSpy: ReturnType<typeof vi.spyOn> | null = null

beforeEach(() => {
  openUrlMock.mockReset()
  openUrlMock.mockResolvedValue(undefined)
  document.body.innerHTML = ''
  detach = attachExternalLinkInterceptor()
  // 注入 __TAURI_INTERNALS__ 以走 Tauri 分支；按 case 删除可走 window.open fallback。
  ;(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {}
  windowOpenSpy = vi.spyOn(window, 'open').mockImplementation(() => null)
})

afterEach(() => {
  detach?.()
  detach = null
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__
  windowOpenSpy?.mockRestore()
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

  test('lets modifier-key clicks pass through (user wants new tab / save as)', () => {
    const event = clickAnchor('https://example.com', { metaKey: true })
    expect(event.defaultPrevented).toBe(false)
    expect(openUrlMock).not.toHaveBeenCalled()
  })

  test('ignores middle/right mouse buttons', () => {
    const event = clickAnchor('https://example.com', { button: 1 })
    expect(event.defaultPrevented).toBe(false)
    expect(openUrlMock).not.toHaveBeenCalled()
  })

  test('falls back to window.open when no Tauri runtime', () => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__
    const event = clickAnchor('https://example.com/bar')
    expect(event.defaultPrevented).toBe(true)
    expect(openUrlMock).not.toHaveBeenCalled()
    expect(windowOpenSpy).toHaveBeenCalledWith(
      'https://example.com/bar',
      '_blank',
      'noopener,noreferrer',
    )
  })

  test('detach unregisters the listener', () => {
    detach?.()
    detach = null
    const event = clickAnchor('https://example.com')
    expect(event.defaultPrevented).toBe(false)
    expect(openUrlMock).not.toHaveBeenCalled()
  })
})
