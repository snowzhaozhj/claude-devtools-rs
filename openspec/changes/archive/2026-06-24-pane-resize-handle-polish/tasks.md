## 1. 视觉改进

- [x] 1.1 加 1px 常驻分隔线（`::after` pseudo-element，`--color-border-emphasis`），hover/active 时消隐
- [x] 1.2 统一 hover/active 高亮色为 `color-mix(in oklch, var(--color-accent-blue) 50%, transparent)`

## 2. ARIA 与键盘

- [x] 2.1 加 `role="separator"` + `tabindex="0"` + `aria-orientation="vertical"` + `aria-label` + `aria-valuemin/max/now`
- [x] 2.2 加 ArrowLeft/ArrowRight 键盘 resize（步长 0.05，Shift 0.15）+ Home/End
- [x] 2.3 加 `focus-visible` 视觉状态（与 hover/active 同色）

## 3. 验证

- [x] 3.1 svelte-check 0 error
- [x] 3.2 浏览器视觉验证：浅色 + 深色主题下 idle/hover/focus 三态截图确认
- [x] 3.3 键盘 resize 功能验证（ArrowLeft/Right/Home/End）

## 4. 发布

- [x] 4.1 push 分支 + 开 PR
- [x] 4.2 wait-ci 全绿
- [x] 4.3 codex + pr-review-toolkit 二审通过
- [ ] 4.4 archive change
