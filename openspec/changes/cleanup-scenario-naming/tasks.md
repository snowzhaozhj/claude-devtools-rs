# 实施任务

## 1. spec delta 重写 8 cap / 15 Requirement Scenario 标题

- [x] 1.1 落 spec delta（每 cap 一份 MODIFIED 段，重写含改名 Scenario 的 Requirement body 全文，仅替换 Scenario 标题）
- [x] 1.2 跑 `openspec validate cleanup-scenario-naming --strict` 通过
- [x] 1.3 各 cap Scenario 数量不变、行为契约 SHALL/MUST/WHEN/THEN/AND 句完全等价

## 2. 本地校验

- [x] 2.1 `bash scripts/check-spec-purity.sh` 通过（标题级清理本身不改 baseline，但跑一遍确认无意外退化）
- [x] 2.2 `just preflight`（fmt + lint + test + spec-validate 一把梭）

## N. 发布

- [x] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过 + spec-guide-reviewer 自审通过
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
