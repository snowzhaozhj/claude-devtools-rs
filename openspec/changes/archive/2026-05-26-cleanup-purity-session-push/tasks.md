# Tasks: cleanup-purity-session-push

## 1. 清理 session-parsing spec
- [ ] 重写 6 个命中 Requirement body

## 2. 清理 push-events spec
- [ ] 重写 2 个命中 Requirement body

## 3. 验证
- [ ] openspec validate 通过
- [ ] spec-purity 两个 cap 真实 hits 降为 0（余下均为 false positive）
- [ ] 更新 baseline

## N. 发布
- [ ] N.1 push + PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过
- [ ] N.4 archive change
