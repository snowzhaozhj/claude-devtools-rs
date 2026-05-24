// Chunk → Markdown 反序 helper 单测（Task 5.5）。
//
// 覆盖：
// - userChunkToMarkdown：string content / ContentBlock[] 含 image 跳过
// - aiChunkToMarkdown：semanticSteps 多 text step 拼接 / 仅 text 类型不取 thinking
// - toolExecToMarkdown：Bash / Read / Edit / Write / 其它工具
// - chunkToPlainText：strip markdown 格式

import { describe, expect, test } from 'vitest'
import {
  userChunkToMarkdown,
  aiChunkToMarkdown,
  toolExecToMarkdown,
  chunkToPlainText,
} from './markdown'
import type { UserChunk, AIChunk, ToolExecution, SystemChunk } from '../api'

const ZERO_METRICS = {
  inputTokens: 0,
  outputTokens: 0,
  cacheCreationTokens: 0,
  cacheReadTokens: 0,
  toolCount: 0,
  costUsd: null,
}

function makeUserChunk(content: UserChunk['content']): UserChunk {
  return {
    kind: 'user',
    chunkId: 'u1:0',
    uuid: 'u1',
    timestamp: '2026-01-01T00:00:00Z',
    durationMs: null,
    content,
    metrics: ZERO_METRICS,
  }
}

function makeAIChunk(steps: AIChunk['semanticSteps']): AIChunk {
  return {
    kind: 'ai',
    chunkId: 'a1:0',
    timestamp: '2026-01-01T00:00:01Z',
    durationMs: null,
    responses: [],
    metrics: ZERO_METRICS,
    semanticSteps: steps,
    toolExecutions: [],
    subagents: [],
    slashCommands: [],
  }
}

describe('userChunkToMarkdown', () => {
  test('string content 直接返回 cleanDisplayText', () => {
    const chunk = makeUserChunk('hello **world**')
    expect(userChunkToMarkdown(chunk)).toBe('hello **world**')
  })

  test('清洗 system-reminder XML noise', () => {
    const chunk = makeUserChunk('<system-reminder>noise</system-reminder>real content')
    expect(userChunkToMarkdown(chunk)).toBe('real content')
  })

  test('ContentBlock[] 仅取 text block 拼接', () => {
    const chunk = makeUserChunk([
      { type: 'text', text: 'first paragraph' },
      { type: 'image', source: { type: 'base64', media_type: 'image/png', data: '' } },
      { type: 'text', text: 'second paragraph' },
    ])
    expect(userChunkToMarkdown(chunk)).toBe('first paragraph\n\nsecond paragraph')
  })

  test('ContentBlock[] 全为 image 时返回空', () => {
    const chunk = makeUserChunk([
      { type: 'image', source: { type: 'base64', media_type: 'image/png', data: '' } },
    ])
    expect(userChunkToMarkdown(chunk)).toBe('')
  })

  test('空字符串返回空', () => {
    expect(userChunkToMarkdown(makeUserChunk(''))).toBe('')
  })
})

describe('aiChunkToMarkdown', () => {
  test('单个 text step', () => {
    const chunk = makeAIChunk([
      { kind: 'text', text: 'hello AI', timestamp: '2026-01-01T00:00:00Z' },
    ])
    expect(aiChunkToMarkdown(chunk)).toBe('hello AI')
  })

  test('多个 text step 用 \\n\\n 拼接', () => {
    const chunk = makeAIChunk([
      { kind: 'text', text: 'first', timestamp: 't1' },
      { kind: 'text', text: 'second', timestamp: 't2' },
    ])
    expect(aiChunkToMarkdown(chunk)).toBe('first\n\nsecond')
  })

  test('thinking / tool_execution / interruption step 跳过', () => {
    const chunk = makeAIChunk([
      { kind: 'thinking', text: 'inner thought', timestamp: 't0' },
      { kind: 'text', text: 'visible reply', timestamp: 't1' },
      { kind: 'tool_execution', toolUseId: 'x', toolName: 'Bash', timestamp: 't2' },
      { kind: 'interruption', text: '[Request interrupted]', timestamp: 't3' },
    ])
    expect(aiChunkToMarkdown(chunk)).toBe('visible reply')
  })

  test('空 semanticSteps 返回空', () => {
    expect(aiChunkToMarkdown(makeAIChunk([]))).toBe('')
  })
})

describe('toolExecToMarkdown', () => {
  test('Bash：命令 + output fence', () => {
    const exec: ToolExecution = {
      toolUseId: 't1',
      toolName: 'Bash',
      input: { command: 'ls -la' },
      output: { kind: 'text', text: 'total 0\nfoo\nbar' },
      isError: false,
      startTs: 'ts',
      endTs: 'te',
      sourceAssistantUuid: 'a1',
    }
    const md = toolExecToMarkdown(exec)
    expect(md).toContain('```bash\n$ ls -la\n```')
    expect(md).toContain('```\ntotal 0\nfoo\nbar\n```')
  })

  test('Bash：output 缺失时仅含命令 fence', () => {
    const exec: ToolExecution = {
      toolUseId: 't1',
      toolName: 'Bash',
      input: { command: 'echo hi' },
      output: { kind: 'missing' },
      isError: false,
      startTs: 'ts',
      endTs: null,
      sourceAssistantUuid: 'a1',
    }
    const md = toolExecToMarkdown(exec)
    expect(md).toBe('```bash\n$ echo hi\n```')
  })

  test('Read：path heading + lang fence', () => {
    const exec: ToolExecution = {
      toolUseId: 't1',
      toolName: 'Read',
      input: { file_path: '/foo/bar.rs' },
      output: { kind: 'text', text: 'fn main() {}' },
      isError: false,
      startTs: 'ts',
      endTs: 'te',
      sourceAssistantUuid: 'a1',
    }
    const md = toolExecToMarkdown(exec)
    expect(md).toContain('**/foo/bar.rs**')
    expect(md).toContain('```rust\nfn main() {}\n```')
  })

  test('Edit：output 转 diff fence', () => {
    const exec: ToolExecution = {
      toolUseId: 't1',
      toolName: 'Edit',
      input: { file_path: '/foo/bar.ts', old_string: 'a', new_string: 'b' },
      output: { kind: 'text', text: '- a\n+ b' },
      isError: false,
      startTs: 'ts',
      endTs: 'te',
      sourceAssistantUuid: 'a1',
    }
    const md = toolExecToMarkdown(exec)
    expect(md).toContain('**/foo/bar.ts**')
    expect(md).toContain('```diff\n- a\n+ b\n```')
  })

  test('Write：input.content 转 lang fence', () => {
    const exec: ToolExecution = {
      toolUseId: 't1',
      toolName: 'Write',
      input: { file_path: '/foo/bar.py', content: 'print("hi")' },
      output: { kind: 'text', text: '' },
      isError: false,
      startTs: 'ts',
      endTs: 'te',
      sourceAssistantUuid: 'a1',
    }
    const md = toolExecToMarkdown(exec)
    expect(md).toContain('**/foo/bar.py**')
    expect(md).toContain('```python\nprint("hi")\n```')
  })

  test('其它工具：input JSON fence', () => {
    const exec: ToolExecution = {
      toolUseId: 't1',
      toolName: 'Glob',
      input: { pattern: '**/*.ts' },
      output: { kind: 'text', text: 'a.ts\nb.ts' },
      isError: false,
      startTs: 'ts',
      endTs: 'te',
      sourceAssistantUuid: 'a1',
    }
    const md = toolExecToMarkdown(exec)
    expect(md).toContain('**Glob**')
    expect(md).toContain('"pattern"')
    expect(md).toContain('a.ts\nb.ts')
  })
})

describe('chunkToPlainText', () => {
  test('UserChunk strip 格式', () => {
    const chunk = makeUserChunk('# Heading\n\n**bold** and `code` and [link](url)')
    expect(chunkToPlainText(chunk)).toBe('Heading\n\nbold and code and link')
  })

  test('AIChunk strip 格式', () => {
    const chunk = makeAIChunk([
      { kind: 'text', text: '## Title\n\n- item 1\n- *italic* item 2', timestamp: 't' },
    ])
    expect(chunkToPlainText(chunk)).toBe('Title\n\nitem 1\nitalic item 2')
  })

  test('SystemChunk', () => {
    const sys: SystemChunk = {
      kind: 'system',
      chunkId: 's1:0',
      uuid: 's1',
      timestamp: 't',
      durationMs: null,
      contentText: 'plain output',
      metrics: ZERO_METRICS,
    }
    expect(chunkToPlainText(sys)).toBe('plain output')
  })

  test('fenced code block 保留内部代码', () => {
    const chunk = makeUserChunk('```rust\nfn main() {}\n```')
    expect(chunkToPlainText(chunk)).toBe('fn main() {}')
  })

  test('image markdown 转 alt 文本', () => {
    const chunk = makeUserChunk('![alt text](https://example.com/img.png)')
    expect(chunkToPlainText(chunk)).toBe('alt text')
  })

  test('blockquote / list marker 剥除', () => {
    const chunk = makeUserChunk('> quote\n\n* a\n* b\n\n1. one')
    expect(chunkToPlainText(chunk)).toBe('quote\n\na\nb\n\none')
  })
})
