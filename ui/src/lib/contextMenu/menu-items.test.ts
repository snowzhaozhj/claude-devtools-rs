// menu-items 函数库单测（Task 4.7）。
//
// 硬约束（spec menu-items 函数库 Scenario "factory 返回纯数据"）：
// - 单测**不**依赖 jsdom `window.getSelection` polyfill
// - factory 内部不读 DOM；selection 通过 ctx.selectionText 传入
// - mock dispatch 后调用 action 仅触发 mock，不发真 IPC

import { describe, expect, test, vi } from 'vitest'
import {
  buildUserMessageItems,
  buildAssistantMessageItems,
  buildBashToolItems,
  buildFileToolItems,
  buildWorktreeChipItems,
  buildProjectCardItems,
  buildSelectionItems,
  buildMarkdownBlockItems,
  type MenuItemContext,
  type MenuItemDispatch,
} from './menu-items'
import type { UserChunk, AIChunk, ToolExecution } from '../api'
import type { ContextMenuItem } from './types'

// ---- 测试基础 ----

const ZERO_METRICS = {
  inputTokens: 0,
  outputTokens: 0,
  cacheCreationTokens: 0,
  cacheReadTokens: 0,
  toolCount: 0,
  costUsd: null,
}

function makeMockDispatch(): MenuItemDispatch & { _calls: Record<string, unknown[][]> } {
  const calls: Record<string, unknown[][]> = {
    copyToClipboard: [],
    openInEditor: [],
    openInTerminal: [],
    revealInDir: [],
    openUrl: [],
  }
  return {
    copyToClipboard: vi.fn((text: string) => {
      calls.copyToClipboard.push([text])
      return Promise.resolve()
    }),
    openInEditor: vi.fn((path: string, line?: number, column?: number) => {
      calls.openInEditor.push([path, line, column])
      return Promise.resolve()
    }),
    openInTerminal: vi.fn((path: string) => {
      calls.openInTerminal.push([path])
      return Promise.resolve()
    }),
    revealInDir: vi.fn((path: string) => {
      calls.revealInDir.push([path])
      return Promise.resolve()
    }),
    openUrl: vi.fn((url: string) => {
      calls.openUrl.push([url])
      return Promise.resolve()
    }),
    _calls: calls,
  }
}

function makeCtx(overrides: Partial<MenuItemContext> = {}): MenuItemContext {
  return {
    sessionId: 'session-1',
    projectId: 'project-1',
    settings: {
      externalEditor: 'vs_code',
      searchEngine: { type: 'google' },
      terminalApp: 'terminal',
    },
    selectionText: '',
    dispatch: makeMockDispatch(),
    ...overrides,
  }
}

function makeUserChunk(content = 'hello'): UserChunk {
  return {
    kind: 'user',
    chunkId: 'u1:0',
    uuid: 'u1',
    timestamp: 'ts',
    durationMs: null,
    content,
    metrics: ZERO_METRICS,
  }
}

function makeAIChunk(): AIChunk {
  return {
    kind: 'ai',
    chunkId: 'a1:0',
    timestamp: 'ts',
    durationMs: null,
    responses: [],
    metrics: ZERO_METRICS,
    semanticSteps: [{ kind: 'text', text: 'AI reply', timestamp: 'ts' }],
    toolExecutions: [],
    subagents: [],
    slashCommands: [],
  }
}

function makeBashExec(overrides: Partial<ToolExecution> = {}): ToolExecution {
  return {
    toolUseId: 't1',
    toolName: 'Bash',
    input: { command: 'ls', cwd: '/tmp' },
    output: { kind: 'text', text: 'output' },
    isError: false,
    startTs: 'ts',
    endTs: 'te',
    sourceAssistantUuid: 'a1',
    ...overrides,
  }
}

function makeFileExec(toolName: 'Read' | 'Edit' | 'Write', path = '/foo/bar.ts'): ToolExecution {
  return {
    toolUseId: 't1',
    toolName,
    input: { file_path: path },
    output: { kind: 'text', text: 'content' },
    isError: false,
    startTs: 'ts',
    endTs: 'te',
    sourceAssistantUuid: 'a1',
  }
}

function labels(items: ContextMenuItem[]): string[] {
  return items.map((it) => (it.separator ? '---' : (it.label ?? '')))
}

// ---- factory 纯函数 + 不读 DOM ----

describe('factory 纯函数：相同输入 → 相同输出', () => {
  test('buildUserMessageItems 决定性', () => {
    const ctx = makeCtx()
    const chunk = makeUserChunk()
    const a = buildUserMessageItems(chunk, ctx)
    const b = buildUserMessageItems(chunk, ctx)
    expect(labels(a)).toEqual(labels(b))
  })

  test('factory 内部不调 window.getSelection（覆盖率为 0 即为通过）', () => {
    // 我们不能直接 spy jsdom 的 getSelection；但纯函数要求只看 ctx.selectionText
    // 行为：selectionText="" 时不插"复制选中文本"，非空时插
    const ctx0 = makeCtx({ selectionText: '' })
    const ctxS = makeCtx({ selectionText: 'selected' })
    const a = buildUserMessageItems(makeUserChunk(), ctx0)
    const b = buildUserMessageItems(makeUserChunk(), ctxS)
    expect(labels(a)).not.toContain('复制选中文本')
    expect(labels(b)).toContain('复制选中文本')
  })
})

// ---- 选区融合（spec：有选区时融合"复制选中文本"） ----

describe('selectionText 融合', () => {
  test('UserMessage：有选区时插入"复制选中文本"在首项', () => {
    const ctx = makeCtx({ selectionText: 'highlighted text' })
    const items = buildUserMessageItems(makeUserChunk(), ctx)
    expect(items[0].label).toBe('复制选中文本')
    expect(items[0].shortcut).toBe('⌘C')
  })

  test('"复制选中文本" action 调 dispatch.copyToClipboard 传选区文本', async () => {
    const ctx = makeCtx({ selectionText: 'selected slice' })
    const items = buildUserMessageItems(makeUserChunk(), ctx)
    items[0].action?.()
    await Promise.resolve()
    expect(ctx.dispatch.copyToClipboard).toHaveBeenCalledWith('selected slice')
  })

  test('AssistantMessage：有选区时也融合', () => {
    const ctx = makeCtx({ selectionText: 'x' })
    const items = buildAssistantMessageItems(makeAIChunk(), ctx)
    expect(items[0].label).toBe('复制选中文本')
  })

  test('Selection menu：空 selectionText 返回空数组', () => {
    expect(buildSelectionItems('', makeCtx())).toEqual([])
  })

  test('Selection menu：非空 selectionText 返回完整菜单', () => {
    const ctx = makeCtx({ selectionText: 'foo' })
    const items = buildSelectionItems('foo', ctx)
    const ls = labels(items)
    expect(ls).toContain('复制')
    expect(ls).toContain('复制为引用 Markdown')
    expect(ls).toContain('在浏览器搜索')
  })
})

// ---- separator 自动插入 ----

describe('separator 自动插入', () => {
  test('UserMessage 无选区：仅 copy 段，无 separator', () => {
    const ctx = makeCtx()
    const items = buildUserMessageItems(makeUserChunk(), ctx)
    const ls = labels(items)
    expect(ls).toContain('复制纯文本')
    expect(ls).toContain('复制为 Markdown')
    expect(ls).not.toContain('---')
    expect(items[0].separator).toBeFalsy()
    expect(items[items.length - 1].separator).toBeFalsy()
  })

  test('Bash with cwd + error: copy / external 三段都有，正确插 separator', () => {
    const ctx = makeCtx()
    const exec = makeBashExec({
      isError: true,
      output: { kind: 'text', text: 'error: file not found' },
    })
    const items = buildBashToolItems(exec, ctx)
    const ls = labels(items)
    // 至少有一个 separator（copy → external 切换）
    expect(ls.filter((l) => l === '---').length).toBeGreaterThan(0)
    expect(ls).toContain('复制命令')
    expect(ls).toContain('在终端打开')
    expect(ls).toContain('在浏览器搜索错误')
  })

  test('FileTool：复制类紧接外部类，中间一个 separator', () => {
    const ctx = makeCtx()
    const exec = makeFileExec('Read')
    const items = buildFileToolItems(exec, ctx)
    const ls = labels(items)
    expect(ls.filter((l) => l === '---').length).toBe(1)
    expect(ls).toContain('复制路径')
    expect(ls).toContain('在编辑器打开')
  })

  test('单一 kind 不应产生 separator', () => {
    const ctx = makeCtx()
    // worktree chip 仅有 path 时只有 copy + external 两段——下面 case 把 path 设空：
    const items = buildWorktreeChipItems({ path: '', name: 'foo' }, ctx)
    // path 为空时仅 copy 段（"复制路径" disabled）
    const seps = items.filter((it) => it.separator).length
    expect(seps).toBe(0)
  })
})

// ---- IPC dispatch ----

describe('item action → dispatch 调用', () => {
  test('"复制纯文本" → copyToClipboard', () => {
    const ctx = makeCtx()
    const chunk = makeUserChunk('hello world')
    const items = buildUserMessageItems(chunk, ctx)
    const copy = items.find((it) => it.label === '复制纯文本')!
    copy.action?.()
    expect(ctx.dispatch.copyToClipboard).toHaveBeenCalledWith('hello world')
  })

  test('"在编辑器打开"（FileTool）→ openInEditor 传 path', () => {
    const ctx = makeCtx()
    const exec = makeFileExec('Edit', '/foo/bar.ts')
    const items = buildFileToolItems(exec, ctx)
    const open = items.find((it) => it.label === '在编辑器打开')!
    open.action?.()
    expect(ctx.dispatch.openInEditor).toHaveBeenCalledWith('/foo/bar.ts')
  })

  test('"在终端打开父目录"（FileTool）→ openInTerminal 传 parentDir', () => {
    const ctx = makeCtx()
    const exec = makeFileExec('Read', '/foo/sub/bar.ts')
    const items = buildFileToolItems(exec, ctx)
    const open = items.find((it) => it.label === '在终端打开父目录')!
    open.action?.()
    expect(ctx.dispatch.openInTerminal).toHaveBeenCalledWith('/foo/sub')
  })

  test('"在 Finder 中显示"（FileTool）→ revealInDir', () => {
    const ctx = makeCtx()
    const exec = makeFileExec('Read', '/foo/bar.ts')
    const items = buildFileToolItems(exec, ctx)
    const reveal = items.find((it) => it.label === '在 Finder 中显示')!
    reveal.action?.()
    expect(ctx.dispatch.revealInDir).toHaveBeenCalledWith('/foo/bar.ts')
  })

  test('"在浏览器搜索"（Selection）→ openUrl 含 google search', () => {
    const ctx = makeCtx({ selectionText: 'rust async' })
    const items = buildSelectionItems('rust async', ctx)
    const search = items.find((it) => it.label === '在浏览器搜索')!
    search.action?.()
    const calls = (ctx.dispatch.openUrl as ReturnType<typeof vi.fn>).mock.calls
    expect(calls[0][0]).toContain('google.com/search?q=rust%20async')
  })

  test('"在浏览器搜索" 走 Custom 模板替换 {query}', () => {
    const ctx = makeCtx({
      selectionText: 'foo',
      settings: {
        externalEditor: 'vs_code',
        searchEngine: { type: 'custom', urlTemplate: 'https://example.com/?q={query}' },
        terminalApp: 'terminal',
      },
    })
    const items = buildSelectionItems('foo', ctx)
    const search = items.find((it) => it.label === '在浏览器搜索')!
    search.action?.()
    const calls = (ctx.dispatch.openUrl as ReturnType<typeof vi.fn>).mock.calls
    expect(calls[0][0]).toBe('https://example.com/?q=foo')
  })

})

// ---- pathLabel ----

describe('pathLabel 中段截断', () => {
  test('FileTool 长路径走 makePathLabel 截断', () => {
    const ctx = makeCtx()
    const exec = makeFileExec('Read', '/Users/zhao/RustroverProjects/Project/claude-devtools-rs/ui/src/lib/contextMenu/menu-items.ts')
    const items = buildFileToolItems(exec, ctx)
    const open = items.find((it) => it.label === '在编辑器打开')!
    expect(open.pathLabel).toBeDefined()
    expect(open.pathLabel!.short.startsWith('在编辑器打开 ~/')).toBe(true)
    expect(open.pathLabel!.short).toContain('…')
    expect(open.pathLabel!.full).toContain('/Users/zhao/RustroverProjects')
  })

  test('短路径 pathLabel 不截断（无 …）', () => {
    const ctx = makeCtx()
    const exec = makeFileExec('Read', '/tmp/foo.ts')
    const items = buildFileToolItems(exec, ctx)
    const open = items.find((it) => it.label === '在编辑器打开')!
    expect(open.pathLabel).toBeDefined()
    expect(open.pathLabel!.short).toBe('在编辑器打开 /tmp/foo.ts')
  })
})

// ---- 边界 ----

describe('边界 case', () => {
  test('Bash 缺少 cwd：不渲染"在终端打开"', () => {
    const ctx = makeCtx()
    const exec = makeBashExec({ input: { command: 'ls' } })
    const items = buildBashToolItems(exec, ctx)
    expect(labels(items)).not.toContain('在终端打开')
  })

  test('Bash output missing：不渲染"复制 stdout"', () => {
    const ctx = makeCtx()
    const exec = makeBashExec({ output: { kind: 'missing' } })
    const items = buildBashToolItems(exec, ctx)
    expect(labels(items)).not.toContain('复制 stdout')
    expect(labels(items)).not.toContain('复制 stderr')
  })

  test('FileTool 缺少 path：复制路径 disabled + 不渲染外部 actions', () => {
    const ctx = makeCtx()
    const exec: ToolExecution = {
      toolUseId: 't1',
      toolName: 'Read',
      input: {},
      output: { kind: 'text', text: 'x' },
      isError: false,
      startTs: 'ts',
      endTs: 'te',
      sourceAssistantUuid: 'a1',
    }
    const items = buildFileToolItems(exec, ctx)
    const copyPath = items.find((it) => it.label === '复制路径')!
    expect(copyPath.disabled).toBe(true)
    expect(labels(items)).not.toContain('在编辑器打开')
    expect(labels(items)).not.toContain('在 Finder 中显示')
  })

  test('Edit 工具：含"复制 Diff"item', () => {
    const ctx = makeCtx()
    const exec = makeFileExec('Edit')
    const items = buildFileToolItems(exec, ctx)
    expect(labels(items)).toContain('复制 Diff')
  })

  test('Read 工具：不含"复制 Diff"item（仅 Edit/Write 才有）', () => {
    const ctx = makeCtx()
    const exec = makeFileExec('Read')
    const items = buildFileToolItems(exec, ctx)
    expect(labels(items)).not.toContain('复制 Diff')
  })

  test('ProjectCard：含"复制项目名" + "复制路径"', () => {
    const ctx = makeCtx()
    const items = buildProjectCardItems({ path: '/repo/foo', name: 'foo' }, ctx)
    expect(labels(items)).toContain('复制路径')
    expect(labels(items)).toContain('复制项目名')
  })
})

// ---- buildMarkdownBlockItems（工具展开块：slash / Output / Thinking / User message） ----

describe('buildMarkdownBlockItems', () => {
  const MD = '# 标题\n正文 **粗体**'

  test('输出"复制纯文本" + "复制为 Markdown"两项', () => {
    const ls = labels(buildMarkdownBlockItems(MD, makeCtx()))
    expect(ls).toContain('复制纯文本')
    expect(ls).toContain('复制为 Markdown')
  })

  test('"复制为 Markdown" 写入原始 markdown 源', () => {
    const ctx = makeCtx()
    const items = buildMarkdownBlockItems(MD, ctx)
    items.find((it) => it.label === '复制为 Markdown')!.action?.()
    expect(ctx.dispatch.copyToClipboard).toHaveBeenCalledWith(MD)
  })

  test('"复制纯文本" strip markdown 标记（去 heading hash / 加粗星号）', () => {
    const ctx = makeCtx()
    const items = buildMarkdownBlockItems(MD, ctx)
    items.find((it) => it.label === '复制纯文本')!.action?.()
    const written = (ctx.dispatch.copyToClipboard as ReturnType<typeof vi.fn>).mock.calls[0][0] as string
    expect(written).not.toContain('#')
    expect(written).not.toContain('**')
    expect(written).toContain('标题')
    expect(written).toContain('粗体')
  })

  test('有选区时首项融合"复制选中文本"并传选区', () => {
    const ctx = makeCtx({ selectionText: 'sel slice' })
    const items = buildMarkdownBlockItems(MD, ctx)
    expect(items[0].label).toBe('复制选中文本')
    expect(items[0].shortcut).toBe('⌘C')
    items[0].action?.()
    expect(ctx.dispatch.copyToClipboard).toHaveBeenCalledWith('sel slice')
  })

  test('空文本返回空数组（调用方据此不弹菜单）', () => {
    expect(buildMarkdownBlockItems('', makeCtx())).toEqual([])
  })

  test('纯函数：相同输入 → 相同输出', () => {
    const ctx = makeCtx()
    expect(labels(buildMarkdownBlockItems(MD, ctx))).toEqual(labels(buildMarkdownBlockItems(MD, ctx)))
  })
})
