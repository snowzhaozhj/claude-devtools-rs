import '@testing-library/jest-dom/vitest'
import { vi, afterEach } from 'vitest'

afterEach(() => {
  vi.restoreAllMocks()
  document.documentElement.removeAttribute('data-theme')
})

export function mockMatchMedia(matches: boolean): () => void {
  const listeners: Array<(e: MediaQueryListEvent) => void> = []
  const original = window.matchMedia
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches,
    media: query,
    onchange: null,
    addEventListener: (_: string, fn: (e: MediaQueryListEvent) => void) => listeners.push(fn),
    removeEventListener: (_: string, fn: (e: MediaQueryListEvent) => void) => {
      const idx = listeners.indexOf(fn)
      if (idx >= 0) listeners.splice(idx, 1)
    },
    dispatchEvent: () => true,
    addListener: () => {},
    removeListener: () => {},
  })) as unknown as typeof window.matchMedia
  return () => {
    window.matchMedia = original
  }
}
