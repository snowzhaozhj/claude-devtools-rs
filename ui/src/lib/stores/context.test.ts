import { emit } from '@tauri-apps/api/event'
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

  test('switchContext 显示 overlay 状态，context_changed 后退场', async () => {
    const { contextStore } = await import('./context.svelte')
    await contextStore.loadContexts()
    await contextStore.startListening()

    await contextStore.switchContext('ssh-mock-prod')
    expect(contextStore.switching).toBe(true)
    expect(contextStore.switchingTo).toBe('ssh-mock-prod')

    await emit('context_changed', { activeContextId: 'ssh-mock-prod', kind: 'ssh' })

    expect(contextStore.activeContextId).toBe('ssh-mock-prod')
    expect(contextStore.switching).toBe(false)
    expect(contextStore.switchingTo).toBeNull()

    contextStore.stopListening()
  })
})
