// UpdateStatusPill 状态机 + popover 生命周期单测。
// 覆盖 spec `app-auto-update/spec.md::UpdateStatusPill 状态机与 popover` 的
// 各 Scenario：idle 不渲染、available pill 可点击、downloading 显示进度、
// downloaded 点击触发 relaunch、error 可见、popover 已展开期间 store 切 idle
// 时 popover 关闭 + listener 释放（D3b idle race）。

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { flushSync, tick } from 'svelte'
import { render, cleanup, fireEvent } from '@testing-library/svelte'

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: vi.fn(),
}))
vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: vi.fn(async () => undefined),
}))
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))
vi.mock('../lib/render', () => ({
  renderMarkdown: (s: string) => `<p>${s}</p>`,
}))

import UpdateStatusPill from './UpdateStatusPill.svelte'
import { updateStore } from '../lib/updateStore.svelte'
import { relaunch } from '@tauri-apps/plugin-process'

const samplePayload = {
  currentVersion: '0.5.3',
  newVersion: '0.5.4',
  notes: '## 0.5.4\n- new pill',
  signatureOk: true,
}

beforeEach(() => {
  updateStore.reset()
})

afterEach(() => {
  cleanup()
  vi.mocked(relaunch).mockReset()
})

describe('UpdateStatusPill 渲染', () => {
  test('idle 态不渲染任何 DOM 节点', () => {
    const { container } = render(UpdateStatusPill)
    expect(container.querySelector('.update-pill')).toBeNull()
  })

  test('available 态渲染 pill 含版本号与 aria-label', async () => {
    updateStore.showAvailable(samplePayload)
    flushSync()
    const { container } = render(UpdateStatusPill)
    await tick()
    const pill = container.querySelector('.update-pill') as HTMLButtonElement | null
    expect(pill).not.toBeNull()
    expect(pill!.classList.contains('pill-available')).toBe(true)
    expect(pill!.getAttribute('aria-label')).toMatch(/可用更新 v0\.5\.4/)
    expect(pill!.textContent).toContain('v0.5.4')
  })

  test('downloading 态显示百分比文本 + 环形进度 SVG', async () => {
    updateStore.showAvailable(samplePayload)
    updateStore.status = 'downloading'
    updateStore.contentLength = 100
    updateStore.downloaded = 32
    flushSync()
    const { container } = render(UpdateStatusPill)
    await tick()
    const pill = container.querySelector('.update-pill') as HTMLElement | null
    expect(pill).not.toBeNull()
    expect(pill!.classList.contains('pill-downloading')).toBe(true)
    expect(pill!.textContent).toContain('32%')
    expect(container.querySelector('.pill-ring svg')).not.toBeNull()
  })

  test('downloaded 态 pill 文本是"重启更新"', async () => {
    updateStore.showAvailable(samplePayload)
    updateStore.status = 'downloaded'
    flushSync()
    const { container } = render(UpdateStatusPill)
    await tick()
    const pill = container.querySelector('.update-pill') as HTMLElement | null
    expect(pill).not.toBeNull()
    expect(pill!.classList.contains('pill-downloaded')).toBe(true)
    expect(pill!.textContent).toContain('重启更新')
  })

  test('error 态显示错误样式 + 文本', async () => {
    updateStore.showAvailable(samplePayload)
    updateStore.status = 'error'
    updateStore.errorMessage = 'signature invalid'
    flushSync()
    const { container } = render(UpdateStatusPill)
    await tick()
    const pill = container.querySelector('.update-pill') as HTMLElement | null
    expect(pill).not.toBeNull()
    expect(pill!.classList.contains('pill-error')).toBe(true)
    expect(pill!.textContent).toContain('更新失败')
    expect(pill!.getAttribute('aria-label')).toMatch(/signature invalid/)
  })
})

describe('UpdateStatusPill 交互', () => {
  test('available 态点击 pill 展开 popover', async () => {
    updateStore.showAvailable(samplePayload)
    flushSync()
    const { container } = render(UpdateStatusPill)
    await tick()
    const pill = container.querySelector('.update-pill') as HTMLButtonElement
    expect(container.querySelector('.update-popover')).toBeNull()
    await fireEvent.click(pill)
    flushSync()
    expect(container.querySelector('.update-popover')).not.toBeNull()
    // popover 内三按钮均存在
    const popover = container.querySelector('.update-popover')!
    expect(popover.querySelector('.btn-primary')).not.toBeNull()
    expect(popover.querySelector('.btn-secondary')).not.toBeNull()
    expect(popover.querySelector('.btn-tertiary')).not.toBeNull()
  })

  test('downloaded 态点击 pill 直接调 relaunch（不展开 popover）', async () => {
    updateStore.showAvailable(samplePayload)
    updateStore.status = 'downloaded'
    flushSync()
    const { container } = render(UpdateStatusPill)
    await tick()
    const pill = container.querySelector('.update-pill') as HTMLButtonElement
    await fireEvent.click(pill)
    flushSync()
    await tick()
    expect(vi.mocked(relaunch)).toHaveBeenCalledTimes(1)
    expect(container.querySelector('.update-popover')).toBeNull()
  })

  test('D3b idle race：popover 已展开期间 store 切 idle 时 popover 关闭', async () => {
    updateStore.showAvailable(samplePayload)
    flushSync()
    const { container } = render(UpdateStatusPill)
    await tick()
    const pill = container.querySelector('.update-pill') as HTMLButtonElement
    await fireEvent.click(pill)
    flushSync()
    expect(container.querySelector('.update-popover')).not.toBeNull()

    // 外部把 store 切到 idle（典型：updateStore.dismiss()）
    updateStore.dismiss()
    flushSync()
    await tick()

    // pill SHALL 从 chrome 移除；popover SHALL 关闭
    expect(container.querySelector('.update-pill')).toBeNull()
    expect(container.querySelector('.update-popover')).toBeNull()
  })

  test('downloading 态 popover 内无"取消下载"按钮（BREAKING 2）', async () => {
    updateStore.showAvailable(samplePayload)
    flushSync()
    const { container } = render(UpdateStatusPill)
    await tick()
    await fireEvent.click(container.querySelector('.update-pill') as HTMLButtonElement)
    flushSync()
    // 切到 downloading 状态
    updateStore.status = 'downloading'
    updateStore.contentLength = 100
    updateStore.downloaded = 10
    flushSync()
    await tick()
    const popover = container.querySelector('.update-popover')
    expect(popover).not.toBeNull()
    // 仅显示进度条 + note，无取消按钮
    expect(popover!.textContent).toMatch(/下载启动后无法中断/)
    expect(popover!.querySelector('.btn-primary')).toBeNull()
  })
})
