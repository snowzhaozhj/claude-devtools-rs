## Why

多 Pane 分屏时 `PaneResizeHandle` 视觉分隔不清晰——当前 6px 透明条仅 hover/拖拽时显色，两个 session detail 面板之间没有常驻视觉边界。impeccable critique 评分 26/40，主要扣分在：缺 ARIA 和键盘支持（P2）、hover 高亮色与 Sidebar resize handle 不一致（P3）。Sidebar handle 已有完整 `role="separator"` + 键盘 resize + oklch accent-blue hover，PaneResizeHandle 全缺——内部一致性缺口。

## What Changes

- 加 1px 常驻分隔线（`::after` pseudo-element，`--color-border-emphasis`），hover 态 5px 半透明高亮时分隔线融入消隐
- 加 `role="separator"` + `tabindex="0"` + `aria-orientation="vertical"` + `aria-label` + `aria-valuemin/max/now` + `focus-visible` 状态
- 加 ArrowLeft/ArrowRight 键盘 resize（步长 0.05 fraction，Shift 加速 0.15）+ Home/End 快捷键
- 统一 hover/active/focus-visible 高亮色为 `color-mix(in oklch, var(--color-accent-blue) 50%, transparent)`，与 Sidebar handle 一致

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `tab-management`: Pane resize 的 Requirement 新增键盘 resize 和 ARIA 语义的 Scenario，补充视觉反馈的常驻分隔线行为

## Impact

- `ui/src/components/layout/PaneResizeHandle.svelte`（单文件改动）
- 无 IPC / 后端 / 数据模型变化
- 无 breaking change
