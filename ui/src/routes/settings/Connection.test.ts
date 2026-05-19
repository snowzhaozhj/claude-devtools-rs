import { clearMocks } from '@tauri-apps/api/mocks'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import { afterEach, beforeEach, describe, expect, test } from 'vitest'

import { setupMockIPC } from '../../lib/tauriMock'
import Connection from './Connection.svelte'

describe('Connection settings form', () => {
  beforeEach(() => {
    setupMockIPC('multi-project-rich')
  })

  afterEach(() => {
    cleanup()
    clearMocks()
  })

  test('host 为空时禁用 Connect 和 Test connection', async () => {
    render(Connection)

    await waitFor(() => expect(screen.getByRole('textbox', { name: 'Host' })).toBeInTheDocument())

    expect(screen.getByRole('button', { name: 'Connect' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Test connection' })).toBeDisabled()
  })

  test('选择 ssh config alias 后填充端口并显示测试状态', async () => {
    render(Connection)

    const hostInput = await screen.findByRole('textbox', { name: 'Host' })
    await fireEvent.focus(hostInput)
    await fireEvent.input(hostInput, { target: { value: 'mock' } })
    await fireEvent.click(await screen.findByRole('option', { name: 'mock-prod' }))

    await waitFor(() => expect(hostInput).toHaveValue('mock-prod'))
    expect(screen.getByRole('spinbutton', { name: 'Port' })).toHaveValue(22)
    expect(screen.getByRole('button', { name: 'Connect' })).toBeEnabled()

    await fireEvent.click(screen.getByRole('button', { name: 'Test connection' }))
    await waitFor(() => expect(screen.getByText('测试成功，active context 未切换')).toBeInTheDocument())
  })

  test('端口超出范围时禁用提交按钮', async () => {
    render(Connection)

    const hostInput = await screen.findByRole('textbox', { name: 'Host' })
    const portInput = screen.getByRole('spinbutton', { name: 'Port' })
    await fireEvent.input(hostInput, { target: { value: 'example.com' } })
    await fireEvent.input(portInput, { target: { value: '0' } })

    expect(screen.getByRole('button', { name: 'Connect' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Test connection' })).toBeDisabled()

    await fireEvent.input(portInput, { target: { value: '22' } })
    expect(screen.getByRole('button', { name: 'Connect' })).toBeEnabled()
  })
})
