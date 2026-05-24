// User story: sidebar 骨架行视觉契约——`.metadata-pending` 必须静态 opacity 占位（无动画），
// 真值到达后子元素 opacity 走 150ms transition fade-in。
//
// 对应 spec `sidebar-navigation/spec.md::Metadata 占位字段视觉渐显`：
// - 骨架行 SHALL 应用静态 opacity 占位（不含 infinite 动画 / background-position 等 paint-only 周期重绘）
// - `.session-title-text` / `.session-meta` 的 transition-property SHALL 含 opacity，让真值 fade-in 真生效
//
// 涉及 issue #256（shimmer paint 反模式根除）+ DESIGN.md:198 视觉契约对齐。
//
// 测试策略：直接扫 stylesheet 验证 CSS 契约本身——避免依赖 svelte scoped CSS 的
// hash class 与运行时 mock fixture 的耦合（svelte 5 的 `:global()` 与 nested
// child selector 在某些组合下 computed style 不稳定，且 fixture 默认无骨架
// session）。CSS 契约级断言比"强加 class 测 computed style"更接近真相源
// （主 spec 的 SHALL 句直接对应 CSS rule 形态）。

import { expect, test } from '@playwright/test'

async function ensureSidebarLoaded(page: import('@playwright/test').Page) {
  await page.goto('/?mock=1&fixture=multi-project-rich')
  await page.locator('.dash-row, .dash-card', { hasText: 'rust-port' }).first().click()
  await expect(page.locator('.session-item').first()).toBeVisible({ timeout: 5_000 })
}

test.describe('sidebar 骨架行视觉契约（静态 opacity，无 shimmer 动画）', () => {
  test('Sidebar.svelte CSS 不含 metadata-pending-shimmer animation 或 @keyframes', async ({ page }) => {
    await ensureSidebarLoaded(page)
    const findings = await page.evaluate(() => {
      const animationRules: string[] = []
      const keyframesNames: string[] = []
      for (const sheet of Array.from(document.styleSheets)) {
        try {
          for (const rule of Array.from(sheet.cssRules)) {
            if (rule instanceof CSSStyleRule) {
              const text = rule.selectorText
              if (text.includes('metadata-pending')) {
                const anim = rule.style.animation || rule.style.animationName
                if (anim && anim !== 'none' && anim !== '') {
                  animationRules.push(`${text} { animation: ${anim} }`)
                }
              }
            }
            if (rule instanceof CSSKeyframesRule) {
              if (rule.name.toLowerCase().includes('shimmer')) {
                keyframesNames.push(rule.name)
              }
            }
          }
        } catch {
          // 跨域 stylesheet（如 third-party CDN）会抛 SecurityError，忽略
        }
      }
      return { animationRules, keyframesNames }
    })
    expect(findings.animationRules).toEqual([])
    expect(findings.keyframesNames).toEqual([])
  })

  test('.session-title-text / .session-meta 的 transition-property 含 opacity', async ({ page }) => {
    await ensureSidebarLoaded(page)
    const transitions = await page.evaluate(() => {
      const titleText = document.querySelector('.session-item .session-title-text') as HTMLElement | null
      const meta = document.querySelector('.session-item .session-meta') as HTMLElement | null
      if (!titleText || !meta) return null
      return {
        titleProperty: getComputedStyle(titleText).transitionProperty,
        metaProperty: getComputedStyle(meta).transitionProperty,
        titleDuration: getComputedStyle(titleText).transitionDuration,
        metaDuration: getComputedStyle(meta).transitionDuration,
      }
    })
    expect(transitions).not.toBeNull()
    if (!transitions) return
    expect(transitions.titleProperty).toContain('opacity')
    expect(transitions.metaProperty).toContain('opacity')
    // 150ms 与 spec 的 `100ms ≤ X ≤ 200ms` 区间一致；CSS 输出格式 "0.15s"
    expect(transitions.titleDuration).toMatch(/0\.15s|150ms/)
    expect(transitions.metaDuration).toMatch(/0\.15s|150ms/)
  })

  test('.metadata-pending 复合 selector 的规则声明 opacity 0.55 且 animation 缺省（无 infinite）', async ({ page }) => {
    await ensureSidebarLoaded(page)
    const findings = await page.evaluate(() => {
      const rules: Array<{ selector: string; opacity: string; hasAnimation: boolean }> = []
      for (const sheet of Array.from(document.styleSheets)) {
        try {
          for (const rule of Array.from(sheet.cssRules)) {
            if (rule instanceof CSSStyleRule && rule.selectorText.includes('metadata-pending')) {
              const animProps = ['animation', 'animationName', 'animationDuration', 'animationIterationCount']
              const hasAnimation = animProps.some((p) => {
                const v = (rule.style as unknown as Record<string, string>)[p]
                return v !== undefined && v !== '' && v !== 'none'
              })
              rules.push({
                selector: rule.selectorText,
                opacity: rule.style.opacity,
                hasAnimation,
              })
            }
          }
        } catch {
          // 跨域 stylesheet 忽略
        }
      }
      return rules
    })
    expect(findings.length).toBeGreaterThan(0)
    // 至少一条规则声明 opacity: 0.55（骨架占位）
    expect(findings.some((r) => r.opacity === '0.55')).toBe(true)
    // 任何 metadata-pending 规则都不得携带 animation
    expect(findings.every((r) => !r.hasAnimation)).toBe(true)
  })
})
