---
target: PaneResizeHandle.svelte
total_score: 26
p0_count: 0
p1_count: 0
timestamp: 2026-06-23T16-35-54Z
slug: ui-src-components-layout-paneresizehandle-svelte
---
## Design Health Score

| # | Heuristic | Score | Key Issue |
|---|-----------|-------|-----------|
| 1 | Visibility of System Status | 3 | idle/hover/active clear; no resize result feedback |
| 2 | Match System / Real World | 4 | Exact VS Code / JetBrains split editor convention |
| 3 | User Control and Freedom | 3 | Free drag; no keyboard alternative |
| 4 | Consistency and Standards | 2 | Sidebar handle has ARIA + keyboard; this one lacks both |
| 5 | Error Prevention | 3 | MIN_FRACTION clamp prevents collapse |
| 6 | Recognition Rather Than Recall | 3 | Visible line + cursor = discoverable; no tooltip |
| 7 | Flexibility and Efficiency | 1 | Mouse-only; no keyboard path |
| 8 | Aesthetic and Minimalist Design | 4 | Every pixel earns its place |
| 9 | Error Recovery | 2 | No undo; no double-click-to-equalize |
| 10 | Help and Documentation | 1 | No aria-label, no tooltip |
| **Total** | | **26/40** | **Acceptable** |

## Anti-Patterns Verdict

**LLM**: Clean. 1px ::after is functional boundary, not a side-stripe accent. No AI slop patterns.
**Detector**: 0 findings.

## Priority Issues

- **[P2] Missing ARIA and keyboard support**: Sidebar handle has role="separator", tabindex, aria-label, keyboard resize. PaneResizeHandle has none.
- **[P3] Hover highlight color inconsistency**: Sidebar uses oklch accent-blue; this uses srgb border-emphasis.
- **[P3] No double-click-to-equalize**: VS Code convention missing.

## Strengths

1. Token usage correct: --color-border-emphasis auto-adapts themes
2. State coverage complete: idle/hover/active with 150ms transitions
3. IDE convention precise: col-resize + visible line + hover highlight

## Minor Observations

- color-mix(in srgb) vs project convention oklch
- svelte-ignore a11y comment acknowledges gap without fixing
