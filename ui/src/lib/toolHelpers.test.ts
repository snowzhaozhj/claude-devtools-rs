import { describe, expect, test } from 'vitest'
import type { ToolExecution } from './api'
import { getLanguageFromPath, getToolDurationMs, isToolPending, toolErrorText } from './toolHelpers'

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

describe('getLanguageFromPath', () => {
  test('普通扩展名映射', () => {
    expect(getLanguageFromPath('Main.java')).toBe('java')
    expect(getLanguageFromPath('App.kt')).toBe('kotlin')
    expect(getLanguageFromPath('script.py')).toBe('python')
    expect(getLanguageFromPath('/abs/path/styles.scss')).toBe('scss')
  })

  test('特殊文件名（无扩展名）', () => {
    expect(getLanguageFromPath('Dockerfile')).toBe('dockerfile')
    expect(getLanguageFromPath('/repo/Makefile')).toBe('makefile')
    expect(getLanguageFromPath('Rakefile')).toBe('ruby')
    expect(getLanguageFromPath('Gemfile')).toBe('ruby')
    expect(getLanguageFromPath('Jenkinsfile')).toBe('groovy')
  })

  test('Dockerfile 前缀变体保留 dockerfile 高亮（修 codex CR Bug 1）', () => {
    expect(getLanguageFromPath('Dockerfile.dev')).toBe('dockerfile')
    expect(getLanguageFromPath('Dockerfile.prod.alpine')).toBe('dockerfile')
    expect(getLanguageFromPath('Containerfile.test')).toBe('dockerfile')
    expect(getLanguageFromPath('Jenkinsfile.staging')).toBe('groovy')
  })

  test('Jenkinsfile.kts 走 kotlin 而非 groovy（修 codex CR 二轮 Bug 3）', () => {
    // 显式扩展名（在 EXT_LANG 里有真映射）必须优先于前缀匹配——
    // Jenkins Pipeline DSL 的 Kotlin 变体是 .kts，应该 kotlin 高亮
    expect(getLanguageFromPath('Jenkinsfile.kts')).toBe('kotlin')
    expect(getLanguageFromPath('Dockerfile.sh')).toBe('bash')
  })

  test('Windows batch 不映射到 powershell（修 codex CR Bug 2）', () => {
    expect(getLanguageFromPath('build.bat')).toBe('plaintext')
    expect(getLanguageFromPath('deploy.cmd')).toBe('plaintext')
    expect(getLanguageFromPath('script.ps1')).toBe('powershell')
  })

  test('未知扩展回退 text', () => {
    expect(getLanguageFromPath('weird.xyzunknown')).toBe('text')
    expect(getLanguageFromPath('no-extension')).toBe('text')
  })

  test('大小写不敏感', () => {
    expect(getLanguageFromPath('FOO.JAVA')).toBe('java')
    expect(getLanguageFromPath('dockerfile')).toBe('dockerfile')
  })
})
