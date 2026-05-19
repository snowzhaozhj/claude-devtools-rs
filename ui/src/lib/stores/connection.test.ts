import { emit } from '@tauri-apps/api/event'
import { clearMocks } from '@tauri-apps/api/mocks'
import { afterEach, beforeEach, describe, expect, test } from 'vitest'

import { setupMockIPC } from '../tauriMock'

describe('connectionStore', () => {
  beforeEach(() => {
    setupMockIPC('multi-project-rich')
  })

  afterEach(() => {
    clearMocks()
  })

  test('loadConfigHosts + resolveHost 填充表单', async () => {
    const { connectionStore } = await import('./connection.svelte')
    await connectionStore.loadConfigHosts()
    expect(connectionStore.configHosts).toContain('mock-prod')

    await connectionStore.resolveHost('mock-prod')
    expect(connectionStore.host).toBe('mock-prod')
    expect(connectionStore.port).toBe(22)
    expect(connectionStore.authMethod).toBe('sshConfig')
  })

  test('connect 成功保存 lastConnection 且不保存 password', async () => {
    const { connectionStore } = await import('./connection.svelte')
    connectionStore.host = 'mock-prod'
    connectionStore.port = 2222
    connectionStore.username = 'alice'
    connectionStore.authMethod = 'password'
    connectionStore.password = 'secret'

    await connectionStore.connect()

    expect(connectionStore.status).toBe('connected')
    expect(connectionStore.lastConnection).toMatchObject({
      host: 'mock-prod',
      port: 2222,
      username: 'alice',
      authMethod: 'password',
    })
    expect(connectionStore.lastConnection).not.toHaveProperty('password')
  })

  test('ssh_status event 更新 error 与 authChain', async () => {
    const { connectionStore } = await import('./connection.svelte')
    await connectionStore.startListening()

    await emit('ssh_status', {
      contextId: 'ssh-mock-prod',
      status: 'error',
      error: {
        code: 'ssh_auth_exhausted',
        attempts: [
          { source: { type: 'envAgent' }, outcome: { type: 'failure', data: 'socket missing' }, elapsedMs: 4 },
        ],
      },
      authChain: [
        { source: { type: 'envAgent' }, outcome: { type: 'failure', data: 'socket missing' }, elapsedMs: 4 },
      ],
    })

    expect(connectionStore.status).toBe('error')
    expect(connectionStore.authChain).toHaveLength(1)
    expect(connectionStore.errorDetail?.code).toBe('ssh_auth_exhausted')

    connectionStore.stopListening()
  })
})
