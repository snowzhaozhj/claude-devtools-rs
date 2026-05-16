import { describe, expect, test } from 'vitest'

import { renderMarkdown } from './render'

describe('renderMarkdown code highlighting', () => {
  test('声明语言代码块继续输出 highlight.js token', () => {
    const html = renderMarkdown('```rust\nfn main() { let value = 1; }\n```')

    expect(html).toContain('class="hljs-keyword"')
    expect(html).toContain('fn')
  })

  test('未声明语言代码块按 plaintext 转义，不自动猜测语言', () => {
    const html = renderMarkdown('```\nfn main() { let value = 1; }\n```')

    expect(html).toContain('<pre><code class="hljs">')
    expect(html).toContain('fn main()')
    expect(html).not.toContain('hljs-keyword')
  })

  test('大块未声明语言代码块不自动猜测语言', () => {
    const code = Array.from({ length: 600 }, (_, i) => `const value${i} = "${i}";`).join('\n')
    const html = renderMarkdown(`\`\`\`\n${code}\n\`\`\``)

    expect(html).toContain('const value599')
    expect(html).not.toContain('hljs-keyword')
    expect(html).not.toContain('hljs-string')
  })
})
