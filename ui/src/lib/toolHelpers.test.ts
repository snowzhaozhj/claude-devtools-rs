import { describe, expect, test } from 'vitest'
import type { ToolExecution } from './api'
import { cleanDisplayText, getLanguageFromPath, getToolDurationMs, isToolPending, shouldPrefetchOnChunkExpand, toolErrorText, viewerUsesOutput } from './toolHelpers'

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

describe('viewerUsesOutput', () => {
  test('Read / Bash / Default 路径 viewer 消费 exec.output', () => {
    expect(viewerUsesOutput(exec({ toolName: 'Read' }))).toBe(true)
    expect(viewerUsesOutput(exec({ toolName: 'Bash' }))).toBe(true)
    expect(viewerUsesOutput(exec({ toolName: 'bash' }))).toBe(true)
    expect(viewerUsesOutput(exec({ toolName: 'Grep' }))).toBe(true)
    expect(viewerUsesOutput(exec({ toolName: 'WebFetch' }))).toBe(true)
  })

  test('Edit 成功路径渲染 input，不消费 output', () => {
    expect(viewerUsesOutput(exec({ toolName: 'Edit' }))).toBe(false)
  })

  test('Edit isError=true 走 ERROR 段，需要 output 兜底显示错误详情', () => {
    expect(viewerUsesOutput(exec({ toolName: 'Edit', isError: true }))).toBe(true)
  })

  test('Write 成功路径渲染 input，不消费 output', () => {
    expect(viewerUsesOutput(exec({ toolName: 'Write' }))).toBe(false)
  })

  test('Write isError=true 走 DefaultToolViewer，需要 output 显示错误详情', () => {
    expect(viewerUsesOutput(exec({ toolName: 'Write', isError: true }))).toBe(true)
  })

  test('Read isError=true 走 DefaultToolViewer，仍消费 output', () => {
    expect(viewerUsesOutput(exec({ toolName: 'Read', isError: true }))).toBe(true)
  })
})

describe('shouldPrefetchOnChunkExpand', () => {
  test('Read 工具 outputOmitted 命中 prefetch', () => {
    expect(shouldPrefetchOnChunkExpand(exec({ toolName: 'Read', outputOmitted: true }))).toBe(true)
  })

  test('Read 工具 outputOmitted=false 不再 prefetch（已有 output）', () => {
    expect(shouldPrefetchOnChunkExpand(exec({ toolName: 'Read', outputOmitted: false }))).toBe(false)
  })

  test('Read 工具 isError=true 不 prefetch（走 DefaultToolViewer，由 toggle 单点拉）', () => {
    expect(shouldPrefetchOnChunkExpand(exec({ toolName: 'Read', isError: true, outputOmitted: true }))).toBe(false)
  })

  test('Bash / Default / Write 工具 SHALL NOT 被 chunk 展开 prefetch（避免并发 IPC 卡顿）', () => {
    expect(shouldPrefetchOnChunkExpand(exec({ toolName: 'Bash', outputOmitted: true }))).toBe(false)
    expect(shouldPrefetchOnChunkExpand(exec({ toolName: 'Grep', outputOmitted: true }))).toBe(false)
    expect(shouldPrefetchOnChunkExpand(exec({ toolName: 'WebFetch', outputOmitted: true }))).toBe(false)
    expect(shouldPrefetchOnChunkExpand(exec({ toolName: 'Write', outputOmitted: true }))).toBe(false)
    expect(shouldPrefetchOnChunkExpand(exec({ toolName: 'Edit', outputOmitted: true }))).toBe(false)
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

describe('cleanDisplayText 空内容防空气泡', () => {
  test('纯噪声 tag → ""', () => {
    expect(cleanDisplayText('<system-reminder>noise</system-reminder>')).toBe('')
    expect(cleanDisplayText('<task-notification><task-id>x</task-id></task-notification>')).toBe('')
  })

  test('零宽 / BiDi 控制符 / BOM 单独存在或夹杂空白 → ""', () => {
    expect(cleanDisplayText('\u200B')).toBe('')
    expect(cleanDisplayText('\u200B\u200C\u200D')).toBe('')
    expect(cleanDisplayText('\uFEFF')).toBe('')
    expect(cleanDisplayText('  \u2060  ')).toBe('')
    expect(cleanDisplayText('<system-reminder>x</system-reminder>\u200B ')).toBe('')
    // BiDi 控制符（LRM/RLM/LRE/RLE/PDF/LRO/RLO/LRI/RLI/FSI/PDI）单独存在视觉等同空
    expect(cleanDisplayText('\u200E\u200F')).toBe('')
    expect(cleanDisplayText('\u202A\u202B\u202C\u202D\u202E')).toBe('')
    expect(cleanDisplayText('\u2066\u2067\u2068\u2069')).toBe('')
  })

  test('HTML 注释单独存在 → ""，但内容里夹注释保留原文', () => {
    expect(cleanDisplayText('<!-- placeholder -->')).toBe('')
    expect(cleanDisplayText(' <!-- a --> \n <!-- b --> ')).toBe('')
    // 含可见字符时 HTML 注释保留——拆分判空与渲染（修 codex CR Bug 3，PR #126 r2）。
    // 否则 markdown code block 里 `<!-- example -->` 会被静默删破坏用户内容。
    expect(cleanDisplayText('hello <!-- side note -->')).toBe('hello <!-- side note -->')
    expect(cleanDisplayText('```html\n<!-- keep -->\n```')).toBe('```html\n<!-- keep -->\n```')
  })

  test('正常内容保留（含 ZWJ emoji 合字 / BiDi 嵌入）', () => {
    expect(cleanDisplayText('hello')).toBe('hello')
    expect(cleanDisplayText('  text  ')).toBe('text')
    // 含可见字符的内容整体保留——不再强行全局 strip 不可见控制符，
    // 否则 emoji ZWJ 合字（如 family \u{1F468}\u200D\u{1F469}\u200D\u{1F467}）会被掰散，
    // RTL 文本里嵌入的 LRM/RLM 也会改变阅读方向。修 codex CR Bug 1（PR #126 r1）。
    expect(cleanDisplayText('hi\u200Bworld')).toBe('hi\u200Bworld')
    expect(cleanDisplayText('\u{1F468}\u200D\u{1F469}\u200D\u{1F467}')).toBe(
      '\u{1F468}\u200D\u{1F469}\u200D\u{1F467}'
    )
    expect(cleanDisplayText('a\u200Eb')).toBe('a\u200Eb')
  })

  test('空 stdout 命令 → ""', () => {
    expect(cleanDisplayText('<local-command-stdout></local-command-stdout>')).toBe('')
    expect(cleanDisplayText('<local-command-stdout>\u200B</local-command-stdout>')).toBe('')
  })})
