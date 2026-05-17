# PR 推进节拍（硬约束）

port 内任何多步改动必须按固定流水线推进，**不得**把 PostToolUse clippy hook 的沉默当作"可以停手"的信号。

本文覆盖 **所有 PR**——既包括 openspec change 走 `/opsx:apply` 的（含 N.4 archive），也包括"直接 commit"的常规 PR（纯视觉对齐 / 单点 bug / Trigger CRUD / docs 等）。两条路径**只在 N.4 是否需要 archive 上分叉**，前面 N.1-N.3 完全相同。

## 默认契约：提需求 = 默认走完整流水线

用户首条 message 含开工动词（`优化 / 改 / 修 / 加 / 做 / 帮我 / 实现 / 重写 / 重构 / polish` 等）**且未含明确停手词** → SHALL 默认一路推到底：

```
preflight → 实现 → 本地验证 → commit → push → PR → codex 二审 → wait-ci 全绿 → (openspec change 才有的) archive → 文本总结
```

明确停手词清单见 `.claude/skills/preflight/SKILL.md::Step 2 Q4` 的表格——用户不在表格里说停 = 默认授权完整流水线。"觉得用户可能不想 push" 的猜测不是停手依据；让用户在 message 里写明才算。

判断准则：**做完业务改动不接续走 N.1+ 是已被否决的下策**——把"该不该 push"的决策权丢给用户当监工，违背"自动化"原则。如果不确定，按 preflight Q4 给的终点走；preflight 没跑就先跑 preflight。

## 节拍

### Propose → Apply 之间：design 阶段 codex 二审（硬约束）

`/opsx:propose` 写完 design.md / spec delta / tasks.md，validate strict 过之后，**进 `/opsx:apply` 之前** SHALL 按 `.claude/rules/codex-usage.md` 第 3 节判断条件决定是否调 codex 二审。任一命中即调（IPC 字段改 / 跨 capability / 性能关键 / 状态机 / UI 重构 / BREAKING）。codex 报问题 → 修 design / spec / tasks 三处文档 → re-validate strict → 才进 apply。**不要**靠 reviewer 在 PR 阶段发现 design 漏洞——那时代码已扩散，回炉成本是 design 阶段拦下的 10×。

### 业务推进段

1. `Edit` 源文件（可并行）
2. `cargo clippy --workspace --all-targets -- -D warnings` 汇总校验（**不是**靠 hook 单文件回显）
3. `cargo fmt --all`
4. `cargo test -p <crate>`（或 `--workspace`）
5. `pnpm --dir ui run check`（如改了 `ui/` 下的文件）
6. `openspec validate <change> --strict`（如有 openspec change）
7. 勾 `openspec/changes/<change>/tasks.md` 的**业务**checkbox（**不勾**发布尾段 N.1-N.4）

### 发布尾段

**openspec change 的 PR** SHALL 在 tasks.md 末尾固定预留 N.1-N.4：

```markdown
## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
```

**常规 PR（非 openspec change）** 没有 tasks.md，但**仍 SHALL 走 N.1-N.3**——把它们当作 `TaskCreate` 队列里的最后三项手工维护：

```
N.1 push 分支 + 开 PR
N.2 wait-ci 全绿
N.3 codex 二审通过
```

只是省掉 N.4。两条路径在前 N.1-N.3 上**完全等价**——业务推进段做完不能停在"本地全绿"那一刻，必须接续推完。

按以下顺序勾 + 操作，**禁止**跳步：

8. 勾 N.1 → `git push -u origin <branch>` + `gh pr create`
9. 勾 N.2 前先 wait-ci 全绿（用 `/wait-ci <pr>` 或后台 `gh pr checks <pr>` until-loop）；红了 SHALL 自己 `gh run view --log-failed` 定位 + 修 + 再 push 后重新等绿
10. 勾 N.3 前调 `Agent({ subagent_type: "codex:codex-rescue", ... })` 跑异构二审（按 `.claude/rules/codex-usage.md` 模板）；codex 报 bug → 修 → push → 回到 step 9 wait-ci 再走一轮 codex 验证；通过才勾 N.3
11. **N.4 是原子操作**——直接跑 `openspec archive <slug> -y`（一步同时完成"mv 目录到 `changes/archive/<date>-<slug>/`"+"sync delta 回主 spec"），随后 `git add -A` + `git commit -m "chore(opsx): archive <slug>"` + push。**不要**先单独 commit "勾 N.4" 再跑 archive——会触发 CI 拦截窗口（详见下方"循环依赖如何避免"）。**常规 PR 跳过此步直接到 step 13**
12. archive commit 推上去后再走一次 wait-ci 全绿
13. 发最终文本总结

## 自检规则

- 每轮 tool call 结束前自检："这批之后要么发下批工具、要么发最终文本，二者必居其一"
- 只发 Edit 没有后续计划 = 禁止
- 开工时把 tasks.md 的每个 `##` section 作为 `TaskCreate` 入队
- 完成一个 `TaskUpdate completed` 一个，给自己留显式的"下一步指针"
- **opsx:apply 完成 ≠ 流程完成**：业务 task 全勾 + spec validate 通过仅是"业务推进段"完成，发布尾段 N.1-N.4 SHALL 显式跑完才算这一轮 PR 真正完成；codex 二审无问题 + CI 全绿 + archive commit 已 push = 完成

## 循环依赖如何避免（archive 时机 vs codex 二审 vs CI 拦截）

**问题**：
- archive 应作为 PR 最后一个 commit（让 reviewer 看主 spec 最终态）
- 但 `scripts/check-openspec-archives.sh` 在 CI 拦"已完成但未 archive"的 change
- codex 二审可能要求改 spec delta，archive 太早跑后还要 `git revert` 重新 archive

**解法**：CI 拦截的**必要条件**是 `(change 在 changes/<slug>/ 下 active) AND (tasks 全勾)`。tasks.md 末尾固定预留 N.1-N.4 不勾，从首次 push 到 archive 之间的每次 CI run 都至少有一个条件不成立：

| Push 时刻 | active？ | tasks 全勾？ | CI 结果 |
|---|---|---|---|
| Push 1（业务 + N.1 勾，N.2-N.4 不勾） | 是 | 否 | ✓ 不挂 |
| Push 2..M（codex 修复轮次） | 是 | 否（N.3-N.4 仍不勾） | ✓ 不挂 |
| 最后一次 push（含 archive commit） | **否**（已 mv 到 archive 目录，`openspec list --json` 不列 archive 内容） | tasks.md 已被 mv 走 | ✓ 不挂 |

**关键不变量**：`openspec archive` 是**原子操作**——同一刻完成"mv 目录 + sync spec"。git 历史里**不会**出现"全勾 + 还在 active 目录"的中间态——上一个 commit 是"N.4 没勾 + 在 active"（不挂），下一个 commit 直接是"目录已 mv + 主 spec 已 sync"（不挂）。

**反例**（已踩，2026-05-16 PR #91）：
- ❌ codex 二审通过后单独 commit "tick tasks.md after codex review"（把所有 tasks 勾完但 change 还 active）→ push → CI 立挂（active + 全勾）
- ✓ 正确做法：codex 审完直接跑 `openspec archive`（一步原子完成 mv + sync），不要先单独勾 N.4
