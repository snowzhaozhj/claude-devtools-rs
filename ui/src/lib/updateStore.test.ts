// updateStore 状态机单测。
//
// 覆盖：
// - showAvailable 写入 banner 可见 + 各字段
// - remindLater 关 banner 但保留 status
// - dismiss 关 banner 且 status 回 idle（非 downloading 时）
// - skipVersion 失败回滚 visible
//
// downloadAndInstall 涉及 plugin-updater 的真实调用，不在本文件覆盖；
// 由 Playwright e2e + 手动 `just dev` 覆盖。

import { afterEach, describe, expect, test, vi } from 'vitest'

// 必须在 import updateStore 之前 mock plugin 模块
vi.mock('@tauri-apps/plugin-updater', () => ({
  check: vi.fn(),
}))
vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: vi.fn(),
}))
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

import { invoke } from '@tauri-apps/api/core'
import { updateStore } from './updateStore.svelte'

const samplePayload = {
  currentVersion: '0.2.0',
  newVersion: '0.3.0',
  notes: '## 0.3.0\n- 新功能',
  signatureOk: true,
}

afterEach(() => {
  updateStore.reset()
  vi.mocked(invoke).mockReset()
})

describe('updateStore.showAvailable', () => {
  test('写入字段并使 banner 可见', () => {
    updateStore.showAvailable(samplePayload)
    expect(updateStore.status).toBe('available')
    expect(updateStore.currentVersion).toBe('0.2.0')
    expect(updateStore.newVersion).toBe('0.3.0')
    expect(updateStore.notes).toBe('## 0.3.0\n- 新功能')
    expect(updateStore.visible).toBe(true)
  })
})

describe('updateStore.remindLater', () => {
  test('关 banner 但保留 status 与字段', () => {
    updateStore.showAvailable(samplePayload)
    updateStore.remindLater()
    expect(updateStore.visible).toBe(false)
    expect(updateStore.status).toBe('available')
    expect(updateStore.newVersion).toBe('0.3.0')
  })
})

describe('updateStore.dismiss', () => {
  test('关 banner 且 status 回 idle（非 downloading）', () => {
    updateStore.showAvailable(samplePayload)
    updateStore.dismiss()
    expect(updateStore.visible).toBe(false)
    expect(updateStore.status).toBe('idle')
  })
})

describe('updateStore.skipVersion', () => {
  test('成功时调 update_config IPC 写入 skippedUpdateVersion', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({})
    updateStore.showAvailable(samplePayload)

    await updateStore.skipVersion()

    expect(invoke).toHaveBeenCalledWith('update_config', {
      section: 'updater',
      configData: { skippedUpdateVersion: '0.3.0' },
    })
    expect(updateStore.visible).toBe(false)
  })

  test('失败时回滚 visible 让用户能再试', async () => {
    vi.mocked(invoke).mockRejectedValueOnce(new Error('disk full'))
    updateStore.showAvailable(samplePayload)

    await expect(updateStore.skipVersion()).rejects.toThrow('disk full')
    expect(updateStore.visible).toBe(true)
  })
})
