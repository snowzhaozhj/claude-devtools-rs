import { describe, expect, test } from 'vitest'
import { escapeCodeLine, lightHighlightLine } from './lightSyntax'

describe('lightSyntax', () => {
  test('转义未高亮语言的 HTML 内容', () => {
    expect(lightHighlightLine('<script>alert("x")</script>', 'text')).toBe(
      '&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;'
    )
  })

  test('使用轻量 token class 高亮常见代码行', () => {
    expect(lightHighlightLine('const value = "ok"', 'typescript')).toContain(
      '<span class="syntax-keyword">const</span>'
    )
    expect(lightHighlightLine('const value = "ok"', 'typescript')).toContain(
      '<span class="syntax-string">&quot;ok&quot;</span>'
    )
  })

  test('注释内容整体转义且不执行 HTML', () => {
    expect(lightHighlightLine('// <img src=x onerror=alert(1)>', 'javascript')).toBe(
      '<span class="syntax-comment">// &lt;img src=x onerror=alert(1)&gt;</span>'
    )
  })

  test('diff 文本渲染可使用纯文本转义而非语法高亮', () => {
    expect(escapeCodeLine('<b>changed</b>')).toBe('&lt;b&gt;changed&lt;/b&gt;')
  })
})
