## 1. Propose artifacts

- [x] 1.1 读取 issue #333 与现有 `sidebar-navigation` / `tab-management` 主 spec，确认 36 个 Requirement 与迁移候选
- [x] 1.2 创建 proposal.md，声明纯 spec 重组 scope、capability 修改范围与无运行时代码影响
- [x] 1.3 创建 design.md，记录 D-1 字符级保持、D-2 用户视角分组、D-3 跨 cap 迁移映射、D-4 Worktree 灰区裁定
- [x] 1.4 创建 `sidebar-navigation` 与 `tab-management` spec delta 初稿

## 2. Design review

- [x] 2.1 运行 `openspec validate reorganize-sidebar-navigation --strict`
- [x] 2.2 启动 codex design 二审，重点检查迁移映射、owner 唯一、Purpose 是否引入新 SHALL/MUST、spec-purity baseline 风险
- [x] 2.3 根据 design 二审修正 proposal / design / spec delta，并重新 validate

## 3. Spec reorganization apply

- [ ] 3.1 archive 同 commit 内完成 `sidebar-navigation` 35 个保留 Requirement 的用户行为分组排序，保持 Scenario 子句字符级不变
- [x] 3.2 将 4 个 Tab owner Scenario 迁入 `tab-management` 唯一 owner Requirement
- [ ] 3.3 清理本次 MODIFIED / 迁移覆盖范围内明显内部实现视角的 Scenario 标题，不改 WHEN / THEN / AND 子句
- [x] 3.4 校验迁移前后 Scenario 数守恒：sidebar 减少数 + tab-management 增加数 = 原 sidebar Scenario 数

## 4. Local validation

- [x] 4.1 运行 `openspec validate reorganize-sidebar-navigation --strict`
- [x] 4.2 运行 `bash scripts/check-spec-purity.sh`，必要时同步 baseline
- [x] 4.3 运行 `just preflight`
- [x] 4.4 提交业务改动 commit

## N. 发布

- [x] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
