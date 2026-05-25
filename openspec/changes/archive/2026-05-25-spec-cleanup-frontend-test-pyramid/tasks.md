## 1. spec delta 应用到主 spec

- [x] 1.1 确认 delta 7 个 MODIFIED Requirement 语义与主 spec 一致（行为不变）
- [x] 1.2 `openspec validate spec-cleanup-frontend-test-pyramid --strict` 通过

## 2. 刷新 spec-purity baseline

- [ ] 2.1 跑 `bash scripts/check-spec-purity.sh --baseline > scripts/spec-purity-baseline.txt` 刷新计数
- [ ] 2.2 确认 `frontend-test-pyramid` 计数从 48 显著降低（目标 ≤ 5）
- [ ] 2.3 确认其他 capability 计数未变

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过
- [ ] N.4 archive change
