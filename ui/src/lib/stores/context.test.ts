import { clearMocks } from '@tauri-apps/api/mocks'
import { afterEach, beforeEach, describe, expect, test } from 'vitest'

import { setupMockIPC } from '../tauriMock'

describe('contextStore', () => {
  beforeEach(() => {
    setupMockIPC('multi-project-rich')
  })

  afterEach(() => {
    clearMocks()
  })

  test('loadContexts 返回 local + ssh context', async () => {
    const { contextStore } = await import('./context.svelte')
    await contextStore.loadContexts()

    expect(contextStore.availableContexts.map((ctx) => ctx.id)).toEqual(['local', 'ssh-mock-prod'])
    expect(contextStore.activeContextId).toBe('local')
  })

  test('switchContext 在 IPC resolve 后退场，mock 模式不依赖 context_changed', async () => {
    const { contextStore } = await import('./context.svelte')
    await contextStore.loadContexts()

    await contextStore.switchContext('ssh-mock-prod')

    expect(contextStore.activeContextId).toBe('ssh-mock-prod')
    expect(contextStore.switching).toBe(false)
    expect(contextStore.switchingTo).toBeNull()
  })
})
