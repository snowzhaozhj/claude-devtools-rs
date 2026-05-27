## 1. CopyButton 通用组件

- [x] 1.1 在 `ui/src/lib/icons.ts` 添加 Copy 和 Check 图标 SVG path 常量
- [x] 1.2 创建 `ui/src/lib/components/CopyButton.svelte`，支持 `text` + `mode`（inline/overlay）prop，点击调 `navigator.clipboard.writeText`，成功后 2s 图标切换反馈，失败静默
- [x] 1.3 vitest 单测验证 CopyButton 渲染两种模式 + 点击调用 clipboard API

## 2. WriteToolViewer 添加 copy 按钮

- [x] 2.1 在 WriteToolViewer header 区（write-badge 后）添加 inline CopyButton，text 为文件写入内容
- [x] 2.2 vitest 或手动验证 WriteToolViewer copy 按钮功能

## 3. BashToolViewer 添加 copy 按钮

- [x] 3.1 在 BashToolViewer header 区添加 inline CopyButton，text 为命令输出文本
- [x] 3.2 vitest 或手动验证 BashToolViewer copy 按钮功能

## 4. OutputBlock 添加 overlay copy

- [x] 4.1 OutputBlock 外层加 `position: relative` + hover 子选择器控制 CopyButton 显隐
- [x] 4.2 CopyButton overlay 模式的 text 传入 OutputBlock 的 `code` prop
- [x] 4.3 手动验证 hover 出现 / 点击复制 / 离开隐藏

## N. 发布
- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
