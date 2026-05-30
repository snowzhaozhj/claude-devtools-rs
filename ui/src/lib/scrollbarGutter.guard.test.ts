/// <reference types="node" />
// 防回归 guard：竖向滚动容器必须显式表态滚动条占位策略。
//
// 背景：app.css 把 ::-webkit-scrollbar 设成 6px 的**经典滚动条**（占布局宽度，
// 非 overlay）。任何 `overflow-y: auto|scroll`（或 `overflow: auto|scroll` 简写）
// 的容器，当内容越过高度阈值滚动条弹出时，会吃掉 6px 宽度 → 内部内容横向重排 →
// 用户可感知的「跳变」。这个 bug 在 WorkflowCard / SessionDetail / Sidebar 上
// 反复复发过 3+ 次（git log: e746cf8 / f60bd58 / 3083000 / 本次 wf-agent-trace）。
//
// 根治：每个竖向滚动块 SHALL 二选一显式表态——
//   1. `scrollbar-gutter: stable;`  恒定预留 gutter，滚动条弹出不改变内容宽度；
//   2. `/* scrollbar-gutter-exempt: <原因> */`  显式豁免（浮层打开即定尺寸 /
//      等宽代码块横向滚动为主等，无生命周期内宽度跳变）。
// 二者都没有 = 违规，本测试 fail，逼迫新滚动容器作者作一次有意识的决策。
//
// 仅约束竖向轴（overflow-y / overflow 简写）；纯 `overflow-x: auto` 是横向轴，
// 跳变方向不同，不在本 guard 范围。

import { readdirSync, readFileSync, statSync } from 'node:fs'
import { dirname, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, test } from 'vitest'

const HERE = dirname(fileURLToPath(import.meta.url))
const SRC_ROOT = resolve(HERE, '..') // ui/src

function walkSvelte(dir: string): string[] {
  const out: string[] = []
  for (const entry of readdirSync(dir)) {
    if (entry === 'node_modules') continue
    const full = join(dir, entry)
    const st = statSync(full)
    if (st.isDirectory()) out.push(...walkSvelte(full))
    else if (entry.endsWith('.svelte')) out.push(full)
  }
  return out
}

// 抽取所有 <style> 块内容（容忍 lang/属性）。
function styleBlocks(source: string): string[] {
  const blocks: string[] = []
  const re = /<style[^>]*>([\s\S]*?)<\/style>/gi
  let m: RegExpExecArray | null
  while ((m = re.exec(source)) !== null) blocks.push(m[1])
  return blocks
}

// 把样式块拆成叶子规则（selector { body }）。body 不含 `{}`，因此 @media 这类
// 嵌套块的外层匹配会失败，但其内部叶子规则仍会被全局正则单独命中——足够覆盖。
function leafRules(css: string): { selector: string; body: string }[] {
  const rules: { selector: string; body: string }[] = []
  const re = /([^{}]+)\{([^{}]*)\}/g
  let m: RegExpExecArray | null
  while ((m = re.exec(css)) !== null) {
    rules.push({ selector: m[1].trim().replace(/\s+/g, ' '), body: m[2] })
  }
  return rules
}

// 竖向滚动：overflow-y: auto|scroll 或 overflow: auto|scroll（简写含竖向）。
// 含双值简写 `overflow: hidden auto`（第二个值是 Y 轴）。
// 明确排除纯 overflow-x。
const VERTICAL_SCROLL =
  /overflow-y\s*:\s*(?:auto|scroll)\b|overflow\s*:\s*(?:auto|scroll)\b|overflow\s*:\s*\S+\s+(?:auto|scroll)\b/
const HAS_GUTTER = /scrollbar-gutter\s*:\s*stable\b/
const HAS_EXEMPT = /scrollbar-gutter-exempt/

describe('scrollbar-gutter 防回归 guard', () => {
  const files = walkSvelte(SRC_ROOT)

  test('存在被扫描的 .svelte 文件（防 glob 失效空跑）', () => {
    expect(files.length).toBeGreaterThan(10)
  })

  test('每个竖向滚动容器都显式表态 scrollbar-gutter: stable 或 exempt 注释', () => {
    const violations: string[] = []
    for (const file of files) {
      const source = readFileSync(file, 'utf8')
      for (const block of styleBlocks(source)) {
        for (const { selector, body } of leafRules(block)) {
          if (!VERTICAL_SCROLL.test(body)) continue
          if (HAS_GUTTER.test(body) || HAS_EXEMPT.test(body)) continue
          violations.push(`${relative(SRC_ROOT, file)} → 选择器 \`${selector}\``)
        }
      }
    }

    expect(
      violations,
      [
        '以下竖向滚动容器未声明滚动条占位策略，会在滚动条弹出时压缩内容宽度造成横向跳变。',
        '请二选一：',
        '  · 内容随生命周期变化（列表/会话流/详情）→ 加 `scrollbar-gutter: stable;`',
        '  · 浮层/代码块首帧即定型 → 加 `/* scrollbar-gutter-exempt: <原因> */`',
        '违规项：\n' + violations.map((v) => '  - ' + v).join('\n'),
      ].join('\n'),
    ).toEqual([])
  })
})
