# tasks — openspec-slim-M-tier

## 1. 配置 / 通用

- [x] 1.1 把 8 个 cap 从主 spec 拷出当前态作 baseline
- [x] 1.2 写 design.md（D1 6 尺子 / D2 不变量 / D3 cap 侧重 / D-Impl-1..6 决策审计）
- [x] 1.3 写本 tasks.md

## 2. 8 个 cap 的 spec delta（每个一个 commit）

- [x] 2.1 configuration-management：合并三个 enum 字段 per-value scenario；剔除 `serde(default)` 之类 attribute 描述
- [x] 2.2 fs-abstraction：H1-H6 enforce 路径 / 测试文件名 / 12 方法内部细节移 design
- [x] 2.3 tool-execution-linking：工具块右键菜单两 Requirement 收敛为引用 frontend-context-menu
- [ ] 2.4 chunk-building：teammate 5 步实现移 design；删 EMBED_TEAMMATES 回滚 Scenario
- [ ] 2.5 project-discovery：合并 Windows 路径 4 Scenario；剔除源码引用
- [ ] 2.6 http-data-api：三组路由表合并；完整路由表移 design
- [ ] 2.7 keyboard-shortcuts：normalizeBindingToMod 11 步算法 fold 4 条契约
- [ ] 2.8 frontend-context-menu：8 factory + 3 层 handler 收敛

## 3. 验证

- [ ] 3.1 `openspec validate openspec-slim-M-tier --strict` 通过
- [ ] 3.2 8 个 cap delta 各自 `openspec show <slug> --type change` 验证 Requirement body 含 SHALL/MUST
- [ ] 3.3 反引号密度 / 行数缩减目标值检查（按 D3 表）
- [ ] 3.4 archive 后跑 `bash scripts/check-spec-purity.sh --baseline > scripts/spec-purity-baseline.txt` 刷新 ratchet baseline 入同 PR（`scripts/check-spec-purity.sh` 默认拒绝"计数下降但 baseline 未同步刷新"）

## 4. codex design 二审

- [ ] 4.1 起 `codex:codex-rescue` subagent 跑 design.md + 8 cap delta 二审（跨 ≥ 2 capability 强制命中）
- [ ] 4.2 修 codex 反馈 bug（每个一 commit）
- [ ] 4.3 二审通过

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex PR 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
