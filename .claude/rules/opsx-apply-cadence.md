# PR 推进节拍（硬约束）

任何多步改动按固定流水线推进，**不得**把 PostToolUse clippy hook 的沉默当作"可以停手"信号。

覆盖**所有 PR**——既包括 openspec change 走 `/opsx:apply` 的（含 N.4 archive），也包括"直接 commit"的常规 PR。两条路径**只在 N.4 是否 archive 上分叉**，前 N.1-N.3 完全相同。

## 默认契约：提需求 = 默认走完整流水线

用户首条 message 含**开工信号词且未含明确停手词** → SHALL 默认一路推到底：

```
preflight → 实现 → 本地验证 → commit → push → PR → wait-ci 与 codex 并行 → 二者都通过 → (openspec change 才有的) archive → 文本总结
```

开工信号词 / 停手词权威定义见 `.claude/skills/preflight/SKILL.md::Q4`，本文不维护副本。

**做完业务改动不接续走 N.1+ 是已被否决的下策**——把"该不该 push"决策权丢给用户当监工，违背"自动化"原则。

## 节拍

### Propose → Apply 之间：三道二审 / 钩子（可并行）

`/opsx:propose` 写完 design.md / spec delta / tasks.md + `openspec validate <slug> --strict` 过之后，进 `/opsx:apply` 之前 SHALL 完成以下三道动作（彼此独立，可并行触发）：

#### 1. codex design 二审

按 `.claude/rules/codex-usage.md` 第 3 节判断。任一命中即调（IPC 字段改 / 跨 capability / 性能关键 / 状态机 / UI 重构 / BREAKING）。codex 报问题 → 修 design / spec / tasks 三处 → re-validate → 才进 apply。

#### 2. impeccable visual contract 钩子（含 UI 改动时强制）

design.md 涉及**新增/重构 UI 组件**（新建 `.svelte` 文件 / 改 ≥ 2 个核心面板 / 加 Settings tab / 新 modal）→ SHALL 跑 `/impeccable shape <feature>`，把产物按下面分配进 design.md：

- **关键视觉决策** → 写进 design.md 已有的 `## Decisions` 段，与现有 D1/D2 编号并列，用 `D-V<n>` 前缀（V = Visual）标记，例如 `D-V1：Surface 选 Diagnostics tab 而非独立 menu item，因 ...`。这样视觉选择和 IPC / 算法选择共享同一审计 + 反转规则
- **以下 4 段写进新增的 `## Visual Contract` 顶级段**（这是 checklist / 规约，不是单点决策，不进 D 编号）：
  - `### Surface Decision` —— 入口选择论证（链回 `PRODUCT.md` anti-references / Design Principles）
  - `### Visual Layer` —— 新组件的视觉决定，**引用** `DESIGN.md` 的 Named Rule 名称（如 `DESIGN.md::Components::Cards and settings rows`、`DESIGN.md::Colors::Named Rules`）；段号会随重排漂移，Named Rule 是稳定锚点
  - `### State Coverage` —— 新组件所有状态（loading / empty / error / disabled / hover）及实现位置
  - `### DESIGN.md delta plan` —— 这次引入值得沉淀的 token / 组件，archive 前跑 `/impeccable extract` 提进 `DESIGN.md` 作为同 PR 一部分落地

**禁止**：design.md 里抄 `PRODUCT.md` / `DESIGN.md` 已有内容（per-change 文档 ≠ 项目级设计契约）；与 `DESIGN.md` 不一致的视觉选择必须显式作为 `D-V<n>` 决策记录，并选定 "改 `DESIGN.md` 还是这次例外"。

`PRODUCT.md` / `DESIGN.md` 缺失或为占位时，先按 impeccable skill 的 `setup` 流程跑 `/impeccable teach` 或 `/impeccable document` 补齐，再回到 D-V / Visual Contract 产出。

#### 3. 形态升级判断（按 `.claude/rules/parallelism-modes.md`「形态选择决策树」）

评估改动规模 + 协作复杂度，决定 apply 阶段用主 session / agent team / N 个 bg。**大改动判定**：`> 2 天工作量 AND (多角色协作 OR 视觉重构 OR 跨 capability)` 中任一特征命中 → SHALL 改用 **Agent team**（lead + 设计师 + 前端 + 后端 + QA）；切忌 lead 单线程一把梭或用 subagent 串行——会撑爆主 context。

**Mid-apply 升级路径**（apply 中途才发现需 agent team 的回退）：在主 session 把当前进度落成 `tasks.md` checkbox + 一段 `progress note`（关键已做 / 阻塞点 / 下一步），`git commit -m "WIP: ..."` 暂存改动，然后启用 agent team 让 lead 接续。**禁止**直接抛弃主 session 进度起 team——丢失上下文比开 team 慢。

**design 阶段拦下问题的回炉成本是 apply 阶段的 10×**——代码扩散后再回炉很痛。视觉契约在 propose 阶段冻结的成本是 apply 阶段救火的几分之一。

### 业务推进段

1. `Edit` 源文件（可并行）
2. `cargo clippy --workspace --all-targets -- -D warnings`（**不是**靠 hook 单文件回显）
3. `cargo fmt --all`
4. `cargo test -p <crate>`（或 `--workspace`）
5. `pnpm --dir ui run check`（如改了 `ui/` 下文件）
6. `openspec validate <change> --strict`（如有 openspec change）
7. 勾任务清单的业务项：
   - **改动包含 `openspec/changes/<slug>/tasks.md`**：勾 tasks.md 业务 checkbox（**不勾**发布尾段 N.1-N.4）
   - **改动不涉及任何 openspec change 的 tasks.md**：用 `TaskUpdate` 把 `TaskCreate` 入队任务标 completed
   - **禁止**在常规 PR 里偷偷改 openspec change 文件而不走 openspec 路径——hybrid 场景 SHALL 升级到 openspec 路径

### 发布尾段

**openspec change 的 PR** SHALL 在 tasks.md 末尾固定预留：

```markdown
## N. 发布
- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
```

**常规 PR（非 openspec change）** 没 tasks.md，仍 SHALL 走 N.1-N.3 当作 `TaskCreate` 队列最后三项手工维护，只省掉 N.4。

按以下顺序勾 + 操作，**禁止**跳步；但 N.2 / N.3 的等待动作 SHALL 并行启动，避免串行空等：

8. 勾 N.1 → `git push -u origin <branch>` + `gh pr create`
9. PR 创建后立刻并行启动两件事：
   - **CI watch**：`/wait-ci <pr>` 或后台 `gh pr checks <pr> --watch --fail-fast --interval 30`
   - **codex 二审**：`Agent({ subagent_type: "codex:codex-rescue", ... })`（prompt 模板见 `.claude/templates/codex-prompt-pr-review.md`）
10. 收敛规则：
   - CI 红了 → 立刻 `gh run view --log-failed` 定位 + 修；如果 codex 仍在跑，等它返回后把两边问题合并成**一个修复 commit**，再 push 并回到 step 9
   - codex 报 bug → 修 + 本地验证；如果 CI 仍在跑，继续等 CI 结果，把 CI 问题一起合并修；修完 push 后回到 step 9
   - 二者都通过 → 才认为 N.2 / N.3 通过
11. checkbox 落地规则：**不要为 N.2 / N.3 单独发 checkbox-only commit**（会白跑整套 CI）。N.2 / N.3 勾选只随下一次实质修复 commit 一起提交；若已无实质改动，保持未勾到 N.4 archive 原子提交前即可。
12. **N.4 是原子操作**——二者都通过后直接跑 `openspec archive <slug> -y`（一步同时完成 mv + sync），随后 `git add -A` + `git commit -m "chore(opsx): archive <slug>"` + push。**不要**先单独 commit "勾 N.4" 再跑 archive——会触发 CI 拦截窗口（见下）。**常规 PR 跳过此步直接到 step 14**
13. archive commit push 后再走一次 wait-ci 全绿
14. 发最终文本总结

## 自检规则

- 每轮 tool call 结束前自检："这批之后要么发下批工具、要么发最终文本，二者必居其一"
- 只发 Edit 没有后续计划 = 禁止
- 开工时把 tasks.md 的每个 `##` section 作为 `TaskCreate` 入队
- 完成一个 `TaskUpdate completed` 一个，给自己留显式的"下一步指针"
- **opsx:apply 完成 ≠ 流程完成**：业务 task 全勾 + spec validate 通过仅是"业务推进段"完成，发布尾段 N.1-N.4 SHALL 显式跑完才算这一轮 PR 真正完成
- **不要单独 push 纯流程勾选 / 非阻塞注释**：checkbox、PR 描述、非阻塞建议只随实质修复或 archive 原子提交落地，避免无意义触发整套 CI

## archive 时机 vs CI 拦截

**问题**：
- archive 应作为 PR 最后一个 commit（让 reviewer 看主 spec 最终态）
- 但 `scripts/check-openspec-archives.sh` 在 CI 拦"已完成但未 archive"的 change
- codex 二审可能要求改 spec delta，archive 太早跑后还要 `git revert` 重新 archive

**解法**：CI 拦截的**必要条件**是 `(change 在 changes/<slug>/ 下 active) AND (tasks 全勾)`。tasks.md 末尾固定预留 N.1-N.4 不勾，从首次 push 到 archive 之间每次 CI run 都至少有一个条件不成立：

| Push 时刻 | active？ | tasks 全勾？ | CI 结果 |
|---|---|---|---|
| Push 1（业务 + N.1 勾，N.2-N.4 不勾） | 是 | 否 | ✓ 不挂 |
| Push 2..M（codex 修复轮次） | 是 | 否（N.3-N.4 仍不勾） | ✓ 不挂 |
| 最后一次 push（含 archive commit） | **否**（已 mv 到 archive 目录，`openspec list --json` 不列 archive 内容） | tasks.md 已被 mv 走 | ✓ 不挂 |

**关键不变量**：`openspec archive` 是**原子操作**——同一刻完成"mv 目录 + sync spec"。git 历史里**不会**出现"全勾 + 还在 active 目录"的中间态——上一个 commit 是"N.4 没勾 + 在 active"（不挂），下一个 commit 直接是"目录已 mv + 主 spec 已 sync"（不挂）。

**反例**（PR #91，2026-05-16）：
- ❌ codex 二审通过后单独 commit "tick tasks.md after codex review"（把所有 tasks 勾完但 change 还 active）→ push → CI 立挂（active + 全勾）
- ✓ 正确做法：codex 审完直接跑 `openspec archive`（一步原子完成 mv + sync），不要先单独勾 N.4
