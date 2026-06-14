// User story: subagent 与 workflow 的执行链（都经 ExecutionTrace 渲染）内的工具
// 展开块（Thinking / Output / User message）SHALL 能右键弹出"该块"的复制菜单，
// 复制内容为该块自身文本，且不冒泡到父消息 chunk 菜单。
//
// 这是 PR #516（仅覆盖 SessionDetail 主会话流的工具展开块）的后续——subagent /
// workflow 执行链经独立的 ExecutionTrace.svelte 渲染，此前漏挂 use:contextMenu。
//
// Spec：openspec/specs/session-display/spec.md §"Subagent 内联展开 ExecutionTrace"
//   （Scenario "右键 subagent ExecutionTrace 内 Output 块弹该块菜单" /
//    "右键 ExecutionTrace 内 Thinking / User message 块弹该块菜单" /
//    "右键 workflow agent ExecutionTrace 内块弹该块菜单"）
//   openspec/specs/frontend-context-menu/spec.md §"menu-items 函数库"（buildMarkdownBlockItems）
//
// 判别力关键：右键 trace 块复制的内容 == 该块文本 != 父 AI 消息文本 → 证明
// 右键落在块上、复制块内容、未冒泡到父 chunk 菜单（buildAssistantMessageItems）。

import { expect, test, type Page } from '@playwright/test'

// 拦截 navigator.clipboard.writeText（避开 e2e clipboard 权限 flake）。必须在 goto 前调。
async function interceptClipboard(page: Page): Promise<void> {
  await page.addInitScript(() => {
    ;(window as unknown as { __copied: string[] }).__copied = []
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        writeText: (t: string) => {
          ;(window as unknown as { __copied: string[] }).__copied.push(t)
          return Promise.resolve()
        },
      },
    })
  })
}

function readCopied(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as unknown as { __copied: string[] }).__copied)
}

async function openTab(page: Page, sessionId: string, projectId: string, label: string) {
  await page.waitForFunction(() => '__cdtTest' in window, { timeout: 5_000 })
  await page.evaluate(
    ([s, p, l]) => {
      ;(
        window as unknown as {
          __cdtTest: { openTab: (s: string, p: string, l: string) => void }
        }
      ).__cdtTest.openTab(s, p, l)
    },
    [sessionId, projectId, label] as const,
  )
}

type Loc = ReturnType<Page['locator']>

// 展开 base-item 块（点 header）后返回其 prose 容器。
// blockTextPrefix 匹配折叠态可见文本（summary）；Thinking 块无 summary，需用
// label "Thinking" 定位（见 ExecutionTrace BaseItem 配置：thinking 无 summary prop）。
async function expandBlockProse(scope: Loc, blockTextPrefix: string) {
  const item = scope.locator('.base-item').filter({ hasText: blockTextPrefix }).first()
  await expect(item).toBeVisible({ timeout: 5_000 })
  await item.locator('.base-item-header').first().click()
  const prose = item.locator('.prose').first()
  await expect(prose).toBeVisible({ timeout: 5_000 })
  return prose
}

// 右键 prose → 点指定 copy item → 返回剪贴板内容数组。
async function rightClickCopy(page: Page, prose: Loc, copyLabel: string) {
  await prose.click({ button: 'right' })
  // 仅 1 个菜单 instance：不会因冒泡叠加成两个菜单
  await expect(page.locator('[role="menu"]')).toHaveCount(1, { timeout: 2_000 })
  const menu = page.locator('[role="menu"]').first()
  await expect(menu).toContainText('复制为 Markdown')
  await expect(menu).toContainText('复制纯文本')
  await menu.getByText(copyLabel).click()
}

test.describe('Subagent ExecutionTrace 工具展开块右键复制', () => {
  // 父 AI 消息正文（若误冒泡到父 chunk 菜单，复制内容会是它）。
  const PARENT_AI_TEXT = '我来帮你检查 LocalDataApi 的字段命名。'
  const PROMPT_TEXT = '审计 IPC 字段映射的 fixture 覆盖'
  const OUTPUT_TEXT = 'Fixture needs a subagent_spawn semantic step'

  // 打开 sess-rust-active → 展开工具区 → 展开 SubagentCard → 展开 Execution Trace；
  // 返回 ExecutionTrace 容器作为 scope，避免误匹配 trace 外的同名块。
  async function openSubagentTrace(page: Page): Promise<Loc> {
    await page.goto('/?mock=1&fixture=multi-project-rich')
    await openTab(page, 'sess-rust-active', 'mock-rich-rust', 'IPC 字段重构')

    const toolsToggle = page.locator('button[aria-label="展开工具调用列表"]').first()
    await expect(toolsToggle).toBeVisible({ timeout: 5_000 })
    await toolsToggle.click()

    const saCard = page.locator('.sa-card').filter({ hasText: 'Audit IPC field' }).first()
    await expect(saCard).toBeVisible({ timeout: 5_000 })
    await saCard.locator('.sa-header').first().click()

    const traceHeader = saCard.locator('.sa-trace-header').first()
    await expect(traceHeader).toBeVisible({ timeout: 5_000 })
    await traceHeader.click()
    const trace = saCard.locator('.execution-trace').first()
    await expect(trace).toBeVisible({ timeout: 5_000 })
    return trace
  }

  test('右键 User message 块 →「复制为 Markdown」写该块文本而非父 AI 消息', async ({ page }) => {
    await interceptClipboard(page)
    const trace = await openSubagentTrace(page)
    const prose = await expandBlockProse(trace, PROMPT_TEXT)
    await rightClickCopy(page, prose, '复制为 Markdown')
    await expect.poll(() => readCopied(page), { timeout: 2_000 }).toContain(
      '审计 IPC 字段映射的 fixture 覆盖：对比 SubagentProcess 各字段与 mock 渲染需求，列出缺失项。',
    )
    expect(await readCopied(page)).not.toContain(PARENT_AI_TEXT)
  })

  test('右键 Thinking 块 →「复制纯文本」写该块内容（不冒泡）', async ({ page }) => {
    await interceptClipboard(page)
    const trace = await openSubagentTrace(page)
    // Thinking 块无 summary，折叠态只见 label "Thinking"，按 label 定位。
    const prose = await expandBlockProse(trace, 'Thinking')
    await rightClickCopy(page, prose, '复制纯文本')
    await expect
      .poll(() => readCopied(page), { timeout: 2_000 })
      .toContain('Need compare SubagentProcess fields with mock fixture rendering requirements.')
    expect(await readCopied(page)).not.toContain(PARENT_AI_TEXT)
  })

  test('右键 Output 块 →「复制为 Markdown」写该块内容（不冒泡）', async ({ page }) => {
    await interceptClipboard(page)
    const trace = await openSubagentTrace(page)
    const prose = await expandBlockProse(trace, OUTPUT_TEXT)
    await rightClickCopy(page, prose, '复制为 Markdown')
    await expect
      .poll(() => readCopied(page), { timeout: 2_000 })
      .toContain('Fixture needs a subagent_spawn semantic step plus a matching SubagentProcess.')
    expect(await readCopied(page)).not.toContain(PARENT_AI_TEXT)
  })

  // 覆盖本 PR 净新增 wiring：buildBlockMenuCtx 在右键瞬间读 window.getSelection()，
  // 有选区时 factory 融合「复制选中文本」。整块选中后右键（点击落在选区内，避免
  // Chromium 右键选区外折叠选区）→ 菜单含「复制选中文本」项（仅当 selectionText
  // 非空才出现，证明选区读取生效）→ 点击复制选区文本。
  test('块内有选区时融合「复制选中文本」并复制选区', async ({ page }) => {
    await interceptClipboard(page)
    const trace = await openSubagentTrace(page)
    const prose = await expandBlockProse(trace, PROMPT_TEXT)

    const selected = await prose.evaluate((el) => {
      const sel = window.getSelection()
      sel?.removeAllRanges()
      sel?.selectAllChildren(el)
      return sel?.toString() ?? ''
    })
    expect(selected.length).toBeGreaterThan(0)

    await prose.click({ button: 'right' })
    const menu = page.locator('[role="menu"]').first()
    await expect(menu).toBeVisible({ timeout: 2_000 })
    await expect(menu).toContainText('复制选中文本')
    await menu.getByText('复制选中文本').click()
    await expect.poll(() => readCopied(page), { timeout: 2_000 }).toContain(selected)
  })
})

test.describe('Workflow agent ExecutionTrace 工具展开块右键复制', () => {
  // 父 AI 消息正文（workflow 卡片所在 AIChunk 的 lastOutput）。
  const PARENT_AI_TEXT = 'Starting workflow execution with deploy-pipeline and integration-suite.'
  const PROMPT_TEXT = 'Build the project and report the bundle size delta'
  const OUTPUT_TEXT = 'Build succeeded; bundle grew by 4.2 KB'

  // 打开 sess-wf-1 → 展开工具区 → 展开 WorkflowCard → 点 builder-1 chip drilldown；
  // 返回 drilldown 内的 ExecutionTrace 容器作为 scope。
  async function openWorkflowAgentTrace(page: Page): Promise<Loc> {
    await page.goto('/?mock=1&fixture=workflow-rich')
    await openTab(page, 'sess-wf-1', 'mock-wf-project', 'Workflow rendering test')

    const toolsToggle = page.locator('button[aria-label="展开工具调用列表"]').first()
    await expect(toolsToggle).toBeVisible({ timeout: 5_000 })
    await toolsToggle.click()

    const wfCard = page.locator('.wf-card').filter({ hasText: 'deploy-pipeline' }).first()
    await expect(wfCard).toBeVisible({ timeout: 5_000 })
    await wfCard.locator('.wf-header').first().click()

    // builder-1 agent chip（带 sessionId，可 drilldown）
    const chip = wfCard.locator('.wf-chip').filter({ hasText: 'builder-1' }).first()
    await expect(chip).toBeVisible({ timeout: 5_000 })
    await chip.click()
    const trace = wfCard.locator('.execution-trace').first()
    await expect(trace).toBeVisible({ timeout: 5_000 })
    return trace
  }

  test('右键 User message 块 →「复制为 Markdown」写该块文本而非父 AI 消息', async ({ page }) => {
    await interceptClipboard(page)
    const trace = await openWorkflowAgentTrace(page)
    const prose = await expandBlockProse(trace, PROMPT_TEXT)
    await rightClickCopy(page, prose, '复制为 Markdown')
    await expect
      .poll(() => readCopied(page), { timeout: 2_000 })
      .toContain('Build the project and report the bundle size delta after the new dependency.')
    expect(await readCopied(page)).not.toContain(PARENT_AI_TEXT)
  })

  test('右键 Output 块 →「复制纯文本」写该块内容（不冒泡）', async ({ page }) => {
    await interceptClipboard(page)
    const trace = await openWorkflowAgentTrace(page)
    const prose = await expandBlockProse(trace, OUTPUT_TEXT)
    await rightClickCopy(page, prose, '复制纯文本')
    await expect
      .poll(() => readCopied(page), { timeout: 2_000 })
      .toContain('Build succeeded; bundle grew by 4.2 KB after adding the new dependency.')
    expect(await readCopied(page)).not.toContain(PARENT_AI_TEXT)
  })
})
