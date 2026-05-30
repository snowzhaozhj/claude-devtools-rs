// WorkflowCard 渲染单测——聚焦运行态（manifest 缺失降级）的新增行为：
// spec `session-display/spec.md::WorkflowCard 渲染` 的 Running 场景：
// - header 显示 `N agents (M done)` 计数 + spinner
// - 空 label 合成 agent 展开显示 "Agent N"（1-based）
// - 无假进度条 / 百分比
// - Tier 1 phases 静态列表（有 phases 时按 phase 分组）
// - 完成态/Empty 既有行为不回归

import { afterEach, describe, expect, test } from 'vitest'
import { tick } from 'svelte'
import { render, cleanup, fireEvent } from '@testing-library/svelte'

import WorkflowCard from './WorkflowCard.svelte'
import type { WorkflowItem } from '../lib/api'

afterEach(() => {
  cleanup()
})

function runningTier0(): WorkflowItem {
  return {
    runId: 'wf_a04767d2-4f1',
    name: 'analysis-pipeline',
    status: 'running',
    phases: [],
    agents: [
      { label: '', phaseIndex: 0, state: 'completed' },
      { label: '', phaseIndex: 0, state: 'running' },
      { label: '', phaseIndex: 0, state: 'running' },
    ],
  }
}

async function expand(container: HTMLElement) {
  const header = container.querySelector('.wf-header') as HTMLElement
  await fireEvent.click(header)
  await tick()
}

describe('WorkflowCard 运行态（Tier 0 降级）', () => {
  test('header 显示 N agents (M done) 计数 + spinner', () => {
    const { container } = render(WorkflowCard, { props: { sessionId: "test-session", workflow: runningTier0() } })
    const summary = container.querySelector('.wf-summary')
    expect(summary?.textContent).toBe('3 agents (1 done)')
    // running 态 header 有 spinner（唯一动画元素）
    expect(container.querySelector('.wf-spinner')).not.toBeNull()
    expect(container.querySelector('.wf-status')?.textContent).toContain('Running')
  })

  test('展开后空 label 合成 agent 显示 "Agent N"（1-based）', async () => {
    const { container } = render(WorkflowCard, { props: { sessionId: "test-session", workflow: runningTier0() } })
    await expand(container)
    const labels = [...container.querySelectorAll('.wf-chip-label')].map(e => e.textContent)
    expect(labels).toEqual(['Agent 1', 'Agent 2', 'Agent 3'])
  })

  test('运行态绝不渲染假进度条/百分比', async () => {
    const { container } = render(WorkflowCard, { props: { sessionId: "test-session", workflow: runningTier0() } })
    await expand(container)
    expect(container.querySelector('progress')).toBeNull()
    expect(container.textContent).not.toMatch(/%/)
  })

  test('展开区 agent chip status dot 静态着色（completed 绿 / running 中性），无动画类', async () => {
    const { container } = render(WorkflowCard, { props: { sessionId: "test-session", workflow: runningTier0() } })
    await expand(container)
    const dots = [...container.querySelectorAll('.wf-chip-dot')]
    expect(dots).toHaveLength(3)
    expect(dots[0].classList.contains('wf-dot-done')).toBe(true)
    expect(dots[1].classList.contains('wf-dot-running')).toBe(true)
    // chip 内不出现 spinner（动画只在 header）
    const body = container.querySelector('.wf-body') as HTMLElement
    expect(body.querySelector('.wf-spinner')).toBeNull()
  })

  test('running + 无 journal（agents 空）显示最小态而非空白', async () => {
    const empty: WorkflowItem = { runId: 'wf_x', name: 'p', status: 'running', phases: [], agents: [] }
    const { container } = render(WorkflowCard, { props: { sessionId: "test-session", workflow: empty } })
    await expand(container)
    expect(container.querySelector('.wf-running-minimal')).not.toBeNull()
    expect(container.querySelector('.wf-empty')).toBeNull()
  })
})

describe('WorkflowCard 运行态 Tier 1（含 phases）', () => {
  test('phases 作静态列表展示在 chips 之上，agent 扁平排列（不按 phase 分组）', async () => {
    const item: WorkflowItem = {
      runId: 'wf_t1',
      name: 'p',
      status: 'running',
      phases: [{ index: 0, title: 'Assess' }, { index: 1, title: 'Synthesize' }],
      agents: [
        // 合成 agent 全 phaseIndex 0（journal 无 phase 标记）
        { label: '', phaseIndex: 0, state: 'running' },
        { label: '', phaseIndex: 0, state: 'completed' },
      ],
    }
    const { container } = render(WorkflowCard, { props: { sessionId: "test-session", workflow: item } })
    await expand(container)
    // 静态 phase pill 列表（标题），无「当前 phase」高亮，无 phase 分组容器
    const pills = [...container.querySelectorAll('.wf-phase-pill')].map(e => e.textContent)
    expect(pills).toEqual(['Assess', 'Synthesize'])
    expect(container.querySelector('.wf-phase-title')).toBeNull()
    // agent 扁平排列：2 个 chip + 匿名 label
    const labels = [...container.querySelectorAll('.wf-chip-label')].map(e => e.textContent)
    expect(labels).toEqual(['Agent 1', 'Agent 2'])
  })
})

describe('WorkflowCard 既有态不回归', () => {
  test('完成态 header 显示 phase·agent 计数 + 具名 agent label', async () => {
    const item: WorkflowItem = {
      runId: 'wf_done',
      name: 'deploy',
      status: 'completed',
      phases: [{ index: 0, title: 'Build' }],
      agents: [{ label: 'builder-1', phaseIndex: 0, state: 'completed', tokens: 100 }],
      totalTokens: 100,
      durationMs: 1000,
    }
    const { container } = render(WorkflowCard, { props: { sessionId: "test-session", workflow: item } })
    expect(container.querySelector('.wf-summary')?.textContent).toBe('1 phase · 1 agent')
    await expand(container)
    expect(container.querySelector('.wf-chip-label')?.textContent).toBe('builder-1')
  })

  test('Empty（非 running + agents 空）展开显示 No subagents', async () => {
    const item: WorkflowItem = {
      runId: 'wf_e',
      name: 'empty',
      status: 'completed',
      phases: [{ index: 0, title: 'Init' }],
      agents: [],
    }
    const { container } = render(WorkflowCard, { props: { sessionId: "test-session", workflow: item } })
    await expand(container)
    expect(container.querySelector('.wf-empty')?.textContent).toContain('No subagents')
  })
})
