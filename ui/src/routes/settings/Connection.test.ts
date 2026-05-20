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

  test('主机为空时禁用「连接」和「测试连接」', async () => {
    render(Connection)

    await waitFor(() => expect(screen.getByRole('textbox', { name: '主机' })).toBeInTheDocument())

    expect(screen.getByRole('button', { name: '连接' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '测试连接' })).toBeDisabled()
  })

  test('选择 ssh config alias 后填充端口并显示测试状态', async () => {
    render(Connection)

    const hostInput = await screen.findByRole('textbox', { name: '主机' })
    await fireEvent.focus(hostInput)
    await fireEvent.input(hostInput, { target: { value: 'mock' } })
    await fireEvent.click(await screen.findByRole('option', { name: 'mock-prod' }))

    await waitFor(() => expect(hostInput).toHaveValue('mock-prod'))
    expect(screen.getByRole('spinbutton', { name: '端口' })).toHaveValue(22)
    expect(screen.getByRole('button', { name: '连接' })).toBeEnabled()

    await fireEvent.click(screen.getByRole('button', { name: '测试连接' }))
    await waitFor(() => expect(screen.getByText('测试通过，未切换当前数据源')).toBeInTheDocument())
  })

  test('端口超出范围时禁用提交按钮', async () => {
    render(Connection)

    const hostInput = await screen.findByRole('textbox', { name: '主机' })
    const portInput = screen.getByRole('spinbutton', { name: '端口' })
    await fireEvent.input(hostInput, { target: { value: 'example.com' } })
    await fireEvent.input(portInput, { target: { value: '0' } })

    expect(screen.getByRole('button', { name: '连接' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '测试连接' })).toBeDisabled()

    await fireEvent.input(portInput, { target: { value: '22' } })
    expect(screen.getByRole('button', { name: '连接' })).toBeEnabled()
  })
})
