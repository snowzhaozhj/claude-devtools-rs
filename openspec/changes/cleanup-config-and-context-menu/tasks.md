## 1. spec delta 应用到主 spec

- [x] 1.1 `configuration-management` 6 个 Requirement MODIFIED 重写应用到主 spec（语义对等迁移，按 D-1 / D-2 / D-3 / D-4）
- [x] 1.2 `frontend-context-menu` 2 个 Requirement MODIFIED 重写应用到主 spec（按 D-2b 数字三分 + D-5 Scenario 标题命名）
- [x] 1.3 `openspec validate cleanup-config-and-context-menu --strict` 通过

## 2. 刷新 spec-purity baseline

- [x] 2.1 跑 `bash scripts/check-spec-purity.sh --baseline > scripts/spec-purity-baseline.txt` 刷新计数
- [x] 2.2 确认 `configuration-management` 计数从 10 → 0
- [x] 2.3 确认 `frontend-context-menu` 计数从 6 → 3（保留 3 处可断言用户感知阈值数字）
- [x] 2.4 确认其他 capability 计数未变

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过 + spec-guide-reviewer 自审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
