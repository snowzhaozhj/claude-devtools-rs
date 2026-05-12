import { describe, expect, test } from 'vitest'
import type { ToolExecution } from './api'
import { getToolDurationMs, isToolPending, toolErrorText } from './toolHelpers'

function exec(overrides: Partial<ToolExecution>): ToolExecution {
  return {
    toolUseId: 'tool-1',
    toolName: 'Bash',
    input: {},
    output: { kind: 'text', text: 'ok' },
    isError: false,
    startTs: '2026-05-12T00:00:00Z',
    endTs: '2026-05-12T00:00:01.250Z',
    sourceAssistantUuid: 'assistant-1',
    ...overrides,
  }
}

describe('toolHelpers', () => {
  test('getToolDurationMs 从 start/end 时间戳派生毫秒耗时', () => {
    expect(getToolDurationMs(exec({}))).toBe(1250)
  })

  test('isToolPending 在缺少 endTs 或 output missing 时返回 true', () => {
    expect(isToolPending(exec({ endTs: null }))).toBe(true)
    expect(isToolPending(exec({ output: { kind: 'missing' } }))).toBe(true)
    expect(isToolPending(exec({}))).toBe(false)
  })

  test('toolErrorText 优先展示后端 errorMessage', () => {
    expect(toolErrorText(exec({ isError: true, errorMessage: 'top-level failure', output: { kind: 'text', text: 'raw' } }))).toBe('top-level failure')
  })

  test('toolErrorText 展示文本错误并清洗噪声', () => {
    const message = '<system-reminder>noise</system-reminder>boom'
    expect(toolErrorText(exec({ isError: true, output: { kind: 'text', text: message } }))).toBe('boom')
  })

  test('toolErrorText 从结构化错误里提取 message/error/stderr', () => {
    expect(toolErrorText(exec({ isError: true, output: { kind: 'structured', value: { message: 'bad args' } } }))).toBe('bad args')
    expect(toolErrorText(exec({ isError: true, output: { kind: 'structured', value: { nested: { stderr: 'no file' } } } }))).toBe('no file')
  })

  test('toolErrorText 对无详情失败给出 fallback', () => {
    expect(toolErrorText(exec({ isError: true, output: { kind: 'missing' } }))).toBe('工具调用失败，但没有返回错误详情。')
  })
})
