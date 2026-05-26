## 1. spec delta 应用到主 spec

- [x] 1.1 确认 delta 14 个 MODIFIED Requirement 语义与主 spec 一致（行为不变；92 Scenario 全保留含等价改写）
- [x] 1.2 `openspec validate ssh-remote-context-cleanup --strict` 通过

## 2. 刷新 spec-purity baseline

- [ ] 2.1 跑 `bash scripts/check-spec-purity.sh --baseline > scripts/spec-purity-baseline.txt` 刷新计数
- [ ] 2.2 确认 `ssh-remote-context` 计数从 78 显著降低（目标 0，已在 propose 阶段验证 delta 文件 = 0；archive 后主 spec 应同步降到 0）
- [ ] 2.3 确认其他 capability 计数未变

## 3. spec-guide-reviewer 自审

- [ ] 3.1 调 `Agent({ subagent_type: "spec-guide-reviewer", ... })` 审本 PR delta；目标 0 hard finding
- [ ] 3.2 warn / info 视情况修复或写入 follow-up

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（不走 design 二审；走 PR 二审）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
