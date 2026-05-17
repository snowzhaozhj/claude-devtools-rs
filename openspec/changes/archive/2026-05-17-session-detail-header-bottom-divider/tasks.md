# Tasks — session-detail-header-bottom-divider

## 1. 修订 spec Scenario

- [x] 1.1 spec delta `MODIFIED` Scenario "SessionDetail 顶部不与 TabBar 行底 border 重叠"，放宽 SHALL：禁止仍限于与 TabBar 行底紧贴的 border；明确允许 top-bar 下方 border-bottom 用于分隔 top-bar 与 conversation
- [x] 1.2 `openspec validate session-detail-header-bottom-divider --strict` 通过

## 2. SessionDetail 视觉调整（已在 commit 1 完成）

- [x] 2.1 移除装饰竖条 `.top-rail`（DOM + CSS）
- [x] 2.2 `.top-bar` 加 `border-bottom: 1px solid var(--color-border)`
- [x] 2.3 `.top-bar` 左 padding 从 28px 改 24px（原 28px 给竖条留空间）
- [x] 2.4 移除未使用的 `.top-bar-ongoing` 类（无 CSS 规则）
- [x] 2.5 `pnpm --dir ui run check`：0 errors / 0 warnings
- [x] 2.6 `pnpm --dir ui run test:unit`：237 passed / 1 skipped

## N. 发布

- [ ] N.1 push 分支 + 开 PR（已开 PR #141，本 commit 增量 push）
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（design.md + spec delta 的二审；已跑 commit 1 二审找到本 spec 违反问题，本 change 是其修复）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
