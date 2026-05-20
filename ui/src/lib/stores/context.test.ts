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

  test('switchContext 在 IPC resolve 后退场，mock 模式不依赖 context_changed', async () => {
    const { contextStore } = await import('./context.svelte')
    await contextStore.loadContexts()

    await contextStore.switchContext('ssh-mock-prod')

    expect(contextStore.activeContextId).toBe('ssh-mock-prod')
    expect(contextStore.switching).toBe(false)
    expect(contextStore.switchingTo).toBeNull()
  })

  test('被动 context_changed activeContextId=null 回到 "local"', async () => {
    // 回归 fix `ssh-disconnect-context-store-sync`：后端 ssh_disconnect 把
    // active context 清空时，会 emit `ContextChanged { activeContextId: null,
    // kind: Local }`。前端 listener 之前 `if (nextActiveContextId)` 在 null
    // 时不更新 store，导致 contextStore 卡在被断开的 SSH context id，UI 各处
    // 依赖此 id 判断"当前 context"会读到错误值（症状：SSH 断开后本机功能受影响）。
    const { contextStore } = await import('./context.svelte')
    await contextStore.startListening()
    await contextStore.switchContext('ssh-mock-prod')
    expect(contextStore.activeContextId).toBe('ssh-mock-prod')

    await emit('context_changed', { activeContextId: null, kind: 'local' })
    // 让 listener 异步任务跑完
    await new Promise((resolve) => setTimeout(resolve, 50))

    expect(contextStore.activeContextId).toBe('local')
    contextStore.stopListening()
  })
})
