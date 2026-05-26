import { describe, expect, test } from 'vitest'
import type { ToolExecution } from './api'
import { cleanDisplayText, getLanguageFromPath, getToolDurationMs, isToolPending, shouldPrefetchOnChunkExpand, stripAnsi, toolErrorText, toolOutputText, viewerUsesOutput } from './toolHelpers'

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

  test('Edit isError=true 且无 errorMessage 时需要 output 兜底显示错误详情', () => {
    expect(viewerUsesOutput(exec({ toolName: 'Edit', isError: true }))).toBe(true)
  })

  test('Edit isError=true 但顶层 errorMessage 已存在时不依赖 output（避免 lazy 拉失败时阻塞展开）', () => {
    expect(viewerUsesOutput(exec({ toolName: 'Edit', isError: true, errorMessage: 'old_string not found' }))).toBe(false)
  })

  test('Edit isError=true 且 errorMessage 仅含空白被视为缺省，仍依赖 output', () => {
    expect(viewerUsesOutput(exec({ toolName: 'Edit', isError: true, errorMessage: '  ' }))).toBe(true)
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

/**
 * stripAnsi \u884C\u4E3A\u57FA\u7EBF + \u5347\u7EA7\u76EE\u6807\u3002
 *
 * \u6D4B\u8BD5\u5206\u4E09\u7EC4\uFF1A
 * 1. EXPECTED-PASS\uFF1A\u5F53\u524D\u7B80\u964B regex\uFF08\u4EC5\u5339\u914D SGR `...m`\uFF09\u80FD strip \u5E72\u51C0\u7684\u2014\u2014
 *    nextest / git log \u7B49\u53EA\u7528 SGR \u7740\u8272\u7684\u5DE5\u5177\u5C5E\u4E8E\u6B64\u7C7B\u3002
 * 2. BASELINE-LEAK\uFF1A\u5F53\u524D regex \u6545\u610F\u6F0F strip \u7684 ECMA-48 \u5B50\u96C6\u2014\u2014
 *    cargo build erase line / DEC private mode / OSC \u6807\u9898 / `\r` \u884C\u8986\u76D6\u7B49\u3002
 *    \u65AD\u8A00"\u5F53\u524D\u786E\u5B9E\u6F0F\u4E86"\uFF0C**\u4E0D\u662F\u65AD\u8A00"\u5E94\u8BE5\u6F0F"**\u2014\u2014\u5347\u7EA7 regex \u540E\u8FD9\u4E9B\u65AD\u8A00\u4F1A
 *    fail\uFF0C\u6210\u4E3A\u63D0\u6848 regex \u5347\u7EA7\u7684\u786C\u8BC1\u636E\u3002\u4FEE\u8FD9\u4E9B\u65AD\u8A00\u5230"\u5DF2\u88AB strip"\u7684\u72B6\u6001
 *    \u53EA\u80FD\u4E0E stripAnsi \u5B9E\u73B0\u5347\u7EA7\u5728\u540C\u4E00 PR \u91CC\u3002
 * 3. STRUCTURE\uFF1A\u4FDD\u7559\u6B63\u5E38\u5B57\u8282\uFF08\u6362\u884C / \u666E\u901A\u6587\u672C / \u7A7A\u5B57\u7B26\u4E32\uFF09\uFF0C\u4EFB\u4F55 regex
 *    \u5347\u7EA7\u90FD\u4E0D\u80FD\u7834\u574F\u3002
 */
describe('stripAnsi', () => {
  describe('EXPECTED-PASS\uFF08\u5F53\u524D\u7B80\u964B regex \u5DF2\u80FD\u5904\u7406 SGR \u7740\u8272\uFF09', () => {
    test('nextest PASS \u884C\uFF08\u622A\u56FE\u771F\u5B9E\u6837\u672C\uFF09\u5265\u6210\u7EAF\u6587\u672C', () => {
      // \u539F\u59CB\u5B57\u8282\u76F4\u63A5\u6765\u81EA\u622A\u56FE\uFF1A32;1m=\u7C97\u7EFF\u300135;1m=\u7C97\u7D2B\u300136m=\u9752\u30010m=\u91CD\u7F6E\u3002
      const input = '\x1b[32;1m       PASS\x1b[0m [   0.011s] (1285/1322) \x1b[35;1mcdt-watch\x1b[0m \x1b[36mwatcher::tests\x1b[0m \x1b[34;1mparse_event_keeps_legacy_two_level_behavior\x1b[0m'
      expect(stripAnsi(input)).toBe('       PASS [   0.011s] (1285/1322) cdt-watch watcher::tests parse_event_keeps_legacy_two_level_behavior')
    })

    test('git log --color \u7684 SGR \u5E8F\u5217\uFF08\u542B\u7A7A\u53C2\u6570 reset `\\x1b[m`\uFF09\u5265\u5E72\u51C0', () => {
      const input = '\x1b[33mcommit 60b381c\x1b[m\n\x1b[1;31mAuthor:\x1b[m \x1b[36mfoo\x1b[m'
      expect(stripAnsi(input)).toBe('commit 60b381c\nAuthor: foo')
    })

    test('\u65E0 ESC \u7684 SGR \u6B8B\u7559\uFF08session \u5B58\u50A8\u8DEF\u5F84\u622A\u65AD ESC \u5B57\u8282\uFF09\u515C\u5E95\u5265\u5E72\u51C0', () => {
      // \u539F\u4EE3\u7801\u6CE8\u91CA\u63D0\u5230\u8E29\u8FC7\u8FD9\u79CD\u60C5\u51B5\u2014\u2014\u67D0\u4E9B session \u5B58\u50A8\u94FE\u8DEF\u4FDD\u7559 `[0m` \u5B57\u9762\u4F46\u622A\u6389 \x1b\u3002
      const input = '[32;1mPASS[0m done'
      expect(stripAnsi(input)).toBe('PASS done')
    })
  })

  describe('BASELINE-LEAK\uFF08\u5F53\u524D regex \u6F0F\u7684 ECMA-48 \u5B50\u96C6\uFF09', () => {
    test('cargo build erase line `\\x1b[2K` \u5F53\u524D\u4E0D\u5265\uFF08K \u7EC8\u6B62\u5B57\u7B26\u4E0D\u88AB SGR regex \u5339\u914D\uFF09', () => {
      const input = '\x1b[2K\r   Compiling cdt-core v0.4.10\n'
      // \u5F53\u524D\u884C\u4E3A\uFF1A\u4FDD\u7559 \x1b[2K \u4E0E \r\uFF0C\u4EC5 SGR \u4E0D\u5728\u573A\u6240\u4EE5 regex \u4E0D\u52A8\uFF1B\u8F93\u51FA\u539F\u6837\u3002
      expect(stripAnsi(input)).toBe('\x1b[2K\r   Compiling cdt-core v0.4.10\n')
    })

    test('DEC private mode `\\x1b[?25l`\uFF08\u9690\u5149\u6807\uFF09\u5F53\u524D\u4E0D\u5265', () => {
      const input = '\x1b[?25l progress \x1b[?25h done'
      expect(stripAnsi(input)).toBe('\x1b[?25l progress \x1b[?25h done')
    })

    test('OSC \u7EC8\u7AEF\u6807\u9898 `\\x1b]0;...\\x07` \u5F53\u524D\u4E0D\u5265', () => {
      const input = '\x1b]0;cargo build\x07Compiling foo'
      expect(stripAnsi(input)).toBe('\x1b]0;cargo build\x07Compiling foo')
    })

    test('\u88F8 `\\r` \u884C\u8986\u76D6\uFF08curl progress \u98CE\u683C\uFF09\u5F53\u524D\u4E0D\u5265', () => {
      const input = '50%\r99%\r100%\n'
      // \u5F53\u524D regex \u4E0D\u52A8 \r\uFF0Chljs \u6E32\u67D3\u65F6\u6309\u884C\u7B97\u4F1A\u51FA\u73B0"50%99%100%"\u6324\u4E00\u884C
      // \u6216\u66F4\u7CDF\u7684\u6E32\u67D3\uFF08\u53D6\u51B3\u4E8E\u6D4F\u89C8\u5668\u5BF9 \r \u7684\u5904\u7406\uFF09\u3002\u5347\u7EA7 regex \u540E\u5E94\u53EA\u4FDD\u7559 100%\u3002
      expect(stripAnsi(input)).toBe('50%\r99%\r100%\n')
    })

    test('\u88F8 `\\x1bD`\uFF08\u5411\u4E0B\u79FB\u52A8\u5355\u5B57\u8282 ESC\uFF09\u5F53\u524D\u4E0D\u5265', () => {
      const input = 'before\x1bDafter'
      expect(stripAnsi(input)).toBe('before\x1bDafter')
    })
  })

  describe('STRUCTURE\uFF08\u4EFB\u4F55 regex \u5347\u7EA7\u90FD SHALL \u4FDD\u6301\u7684\u4E0D\u53D8\u91CF\uFF09', () => {
    test('\u7A7A\u5B57\u7B26\u4E32\u539F\u6837\u8FD4\u56DE', () => {
      expect(stripAnsi('')).toBe('')
    })

    test('\u65E0 ANSI \u6587\u672C\u539F\u6837\u8FD4\u56DE', () => {
      expect(stripAnsi('hello world')).toBe('hello world')
      expect(stripAnsi('\u666E\u901A\u4E2D\u6587')).toBe('\u666E\u901A\u4E2D\u6587')
    })

    test('\u6362\u884C\u7ED3\u6784\u4FDD\u7559', () => {
      expect(stripAnsi('line1\nline2\nline3')).toBe('line1\nline2\nline3')
      expect(stripAnsi('\n\n')).toBe('\n\n')
    })

    test('Windows CRLF \u4E0D\u88AB\u8BEF\u5224\u4E3A\u88F8 \\r \u884C\u8986\u76D6', () => {
      // \u5173\u952E\uFF1A\u5347\u7EA7 regex \u5F15\u5165 `\r` \u5904\u7406\u65F6\uFF0CCRLF (`\r\n`) SHALL \u4FDD\u7559\u4E3A\u5B8C\u6574\u884C\u5C3E\uFF0C
      // \u4E0D\u80FD\u628A `xx\r\nyy` \u5F53\u6210"\r \u4E4B\u524D\u7684\u5185\u5BB9\u88AB\u8986\u76D6"\u3002
      const input = 'line1\r\nline2\r\nline3'
      expect(stripAnsi(input)).toBe('line1\r\nline2\r\nline3')
    })

    test('\u5E26\u65B9\u62EC\u53F7\u4F46\u975E SGR \u7684\u5B57\u9762\u6587\u672C\u4E0D\u88AB\u8BEF\u5265', () => {
      // \u515C\u5E95 regex `/\[(\d+;)*\d*m/g` \u4EC5\u5339\u914D `[\u7EAF\u6570\u5B57;\u6570\u5B57m` \u5F62\u5F0F\uFF0C
      // \u666E\u901A\u65B9\u62EC\u53F7\u6587\u672C\uFF08\u5982 markdown link `[link](url)`\u3001shell `[ -f x ]`\uFF09SHALL \u4E0D\u52A8\u3002
      expect(stripAnsi('[link](url)')).toBe('[link](url)')
      expect(stripAnsi('[ -f /etc/hosts ]')).toBe('[ -f /etc/hosts ]')
      expect(stripAnsi('array[0]')).toBe('array[0]')
    })

    test('\u6570\u5B57\u5B57\u9762\u91CF\u7ED3\u5C3E\u5E26 m \u7684\u5408\u6CD5\u6587\u672C\uFF08\u5982 `200m` \u8DDD\u79BB\uFF09\u4E0D\u88AB\u8BEF\u5265', () => {
      // \u515C\u5E95 regex \u98CE\u9669\u70B9\uFF1A`[200m` \u5B57\u9762\u53EF\u80FD\u88AB\u8BEF\u5224\u4E3A SGR \u6B8B\u7559\u3002\u4F46\u56E0\u4E3A\u524D\u5BFC\u5FC5\u987B\u662F
      // `[` \u5F00\u5934\uFF0C"200m"\uFF08\u6CA1 [\uFF09\u4E0D\u4F1A\u88AB\u5403\u6389\uFF1B"[200m" \u5B57\u9762\u4F1A\u88AB\u5403\u2014\u2014\u8FD9\u662F\u5DF2\u77E5 tradeoff\uFF0C
      // \u515C\u5E95 regex \u547D\u4E2D\u771F\u5B9E session \u6570\u636E\u7684\u6982\u7387\u8FDC\u9AD8\u4E8E\u547D\u4E2D"\u7528\u6237\u5199 [200m \u5F53\u5B57\u9762\u91CF"\u7684\u6982\u7387\u3002
      expect(stripAnsi('\u8DDD\u79BB 200m \u5904')).toBe('\u8DDD\u79BB 200m \u5904')
      expect(stripAnsi('time: 30s; freq: 60Hz')).toBe('time: 30s; freq: 60Hz')
    })
  })
})

/**
 * toolOutputText \u96C6\u6210\u6D4B\u8BD5\u2014\u2014\u4FDD\u8BC1 stripAnsi \u771F\u88AB\u4E32\u5230\u6240\u6709 tool viewer \u6E32\u67D3\u8DEF\u5F84\u3002
 *
 * \u8C03\u7528\u77E9\u9635\uFF1ABashToolViewer / DefaultToolViewer / EditToolViewer / ReadToolViewer
 * \u90FD\u901A\u8FC7 `toolOutputText(exec.output)` \u6D3E\u751F\u663E\u793A\u6587\u672C\uFF0C\u6240\u4EE5\u8FD9\u91CC\u53EA\u9700\u6D4B toolOutputText
 * \u672C\u8EAB\u7684\u4E09 kind \u5206\u652F\u5373\u53EF\u8986\u76D6\u56DB viewer \u7684\u6210\u529F\u8F93\u51FA\u6E32\u67D3\u3002
 */
describe('toolOutputText \u4E32\u63A5 stripAnsi', () => {
  test('text kind \u8D70 stripAnsi \u6E05\u6D17\uFF08\u622A\u56FE nextest \u771F\u5B9E\u6837\u672C\uFF09', () => {
    const ansi = '\x1b[32;1m       PASS\x1b[0m [   0.011s] \x1b[35;1mcdt-watch\x1b[0m'
    expect(toolOutputText({ kind: 'text', text: ansi })).toBe('       PASS [   0.011s] cdt-watch')
  })

  test('text kind \u65E0 ANSI \u65F6\u539F\u6837\u8FD4\u56DE\uFF08\u786E\u8BA4 stripAnsi \u662F\u65E0\u526F\u4F5C\u7528\u901A\u8DEF\uFF09', () => {
    expect(toolOutputText({ kind: 'text', text: 'plain output' })).toBe('plain output')
    expect(toolOutputText({ kind: 'text', text: '\u4E2D\u6587\u8F93\u51FA\n\u7B2C\u4E8C\u884C' })).toBe('\u4E2D\u6587\u8F93\u51FA\n\u7B2C\u4E8C\u884C')
  })

  test('structured kind \u8D70 JSON.stringify \u4E0D\u88AB stripAnsi \u89E6\u78B0', () => {
    expect(toolOutputText({ kind: 'structured', value: { ok: true, count: 3 } }))
      .toBe('{\n  "ok": true,\n  "count": 3\n}')
  })

  test('missing kind \u8FD4\u56DE\u7A7A\u5B57\u7B26\u4E32', () => {
    expect(toolOutputText({ kind: 'missing' })).toBe('')
  })
})
