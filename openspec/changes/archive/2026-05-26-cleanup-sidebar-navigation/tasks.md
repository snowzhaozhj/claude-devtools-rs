# 实施任务

## 1. spec delta 重写 sidebar-navigation 14 Requirement + RENAMED 1

- [x] 1.1 落 spec delta（MODIFIED 14 Requirement + RENAMED 1，按 D-1 / D-2 / D-2c 抽象路径 + 移除 PR 引用 + 数字三分）
- [x] 1.2 跑 `openspec validate cleanup-sidebar-navigation --strict` 通过
- [x] 1.3 跑 `openspec archive cleanup-sidebar-navigation --dry-run` 校验 sync 路径（CLI 无 dry-run flag，改为肉眼对照 14 MOD + 1 RENAMED Req 标题与主 spec 同名匹配）

## 2. baseline 刷与跨 cap 数字校核

- [x] 2.1 跑 `bash scripts/check-spec-purity.sh --baseline` 看 sidebar-navigation 新数（round 2 修后实测 7，与目标 7 一致）
- [x] 2.2 业务 commit 内 sync 主 spec + 同 commit 刷 baseline.txt：`spec/sidebar-navigation 44 → 7`；同 commit 加临时 active change 行 `change/cleanup-sidebar-navigation/sidebar-navigation 7`（archive commit 时再删该行）
- [x] 2.3 跑 `bash scripts/check-spec-purity.sh` 确认本地 ratchet 通过（不严格 mode）
- [x] 2.4 `SPEC_PURITY_STRICT=1 bash scripts/check-spec-purity.sh` 双向 ratchet 二次校验

## N. 发布

- [x] N.1 push 分支 + 开 PR
- [x] N.2 wait-ci 全绿（round 2 通过）
- [x] N.3 codex 二审通过（round 1: 1 MAJOR / 1 NIT 已修；round 2: 0 BLOCKING / 0 MAJOR / 2 MINOR 数字未同步随 archive 合修）+ spec-guide-reviewer 0 hard / 0 warn
- [x] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
