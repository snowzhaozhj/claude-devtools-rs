---
name: preflight
description: 开工 30 秒自检——fetch + 分支 / OpenSpec 适用性 / 探索方式三件套。**只要**用户发出开工信号（"开始干 / 帮我做 X / 先做 A 再做 B / 实现 Y / 修 bug"）或显式 `/preflight`，**都用这个 skill** 先跑一遍——避免落入"在 main 直接编辑 / 跳过 OpenSpec / 不用 Explore subagent / 从过期 origin/main SHA 起 worktree"四个 CLAUDE.md 已固化为硬约束的高频 friction。
---

# preflight

> 触发：`/preflight`；或用户开工信号（"开始做 X / 帮我实现 Y / 先做 A 再做 B / 修这个 bug"）
> 输出：4 个检查结果 + 给用户的简短确认问题
> 不修改任何文件——只读 + 决策 + 可能跑一次 `git fetch`。

## 为什么有这个 skill

CLAUDE.md "What to do first in a fresh session" 把 4 件事列成开工硬约束。但 Claude 在"看上去很急"的请求下会跳：

- 在 main 上编辑（已配 hook 拦截，但仍有路径白名单空隙）
- 行为契约改动事后补 spec（OpenSpec 流程）
- 主动 grep / Read 污染主上下文（应该用 Explore subagent）
- **本地 origin/main SHA 过期就直接起 worktree**——EnterWorktree 默认 `worktree.baseRef=fresh` 用本地 origin/main 指针，不 fetch → 从过期 SHA 起 → PR 一上来 conflict（PR #122 案例：本地落后 24h+ 错过多个 PR）

这个 skill 强制把"跳"成本提前——开工先回答 4 个问题，再触工具。

## 工作步骤

### Step 1：跑诊断命令（并行）

```bash
git fetch origin main --quiet
git branch --show-current
git rev-list --left-right --count HEAD...origin/main
git status --short
ls openspec/changes/ 2>/dev/null | grep -v "^archive$"
```

输出四块：
- 当前分支
- 本地与 origin/main 的偏离（`A B` = 本地多 A 个 commit / 远端多 B 个）
- 未跟踪 / 未提交文件
- 进行中（未 archive）的 openspec change 列表

`git fetch` 是硬约束——CLAUDE.md L 列入"What to do first" 第 1 条。如果 fetch 失败（无网络 / 鉴权），照样继续，但在 Q1 报告里告知"未 fetch，origin/main 指针可能过期"。

### Step 2：回答 4 个问题（自检后给用户看）

**Q1. origin/main 是否已 fetch + 本地是否落后？**

- fetch 成功 + 本地 = origin/main 或仅领先 → ✅ 进入 Q2
- 本地落后（右数 > 0）→ ⚠️ 报告"本地比 origin/main 落后 N 个 commit"，建议在切分支前 `git checkout main && git pull` 同步——尤其是要起 worktree 时（EnterWorktree 用本地 origin/main 指针，落后会从过期 SHA 起）
- fetch 失败 → ⚠️ 报告"未能 fetch，可能离线；如有网络请先 fetch 再开工"

**Q2. 当前在 feature 分支吗？**

- 当前分支非 `main` / `master` → ✅ 进入 Q3
- 当前分支 = `main` 或 `master` → ❌ 报告"开工前需要 `git checkout -b feat/<slug>` / `fix/<slug>`"，给一个 slug 建议（基于用户描述的任务），等用户确认后再切

**Q3. 这个任务该走 OpenSpec 流程吗？**

判断标准（任一命中就走 OpenSpec）：
- IPC 字段 / 后端算法 / 状态判定 / 数据 omit 策略 / Tauri command 协议
- 跨 capability 的行为契约改动
- 性能 / 节流 / 缓存 / 并发改动
- UI 视觉 / 规范 / a11y / typography / 重写组件视觉等关键词命中——CLAUDE.md "What to do first" 第 2 条要求先 invoke `impeccable` skill（不是 OpenSpec，但同样属于"动手前查约定"）

**直接 commit 的场景**（不走 OpenSpec、不走 impeccable）：
- 纯视觉对齐 / 单点样式修复 / Trigger CRUD / 文案修正
- bump version / docs / chore

→ 走 OpenSpec：执行 `/opsx:propose <slug>` 或建议用户跑
→ 视觉规范：先调 `impeccable` skill
→ 直接 commit：进入 Q4

**已有进行中 change 时**：检查 Step 1 的列表里有没有相关 slug——有的话提醒"`<slug>` 已 propose，是否在它内部继续 apply 而非新开"

**判断不准默认走 openspec**（CLAUDE.md "What to do first" 第 2 条）。

**Q4. 探索环节用 Explore subagent 吗？**

判断标准（任一命中就用 Explore subagent）：
- 任务需要 grep / Read 超过 3 个文件来定位入口
- 需要跨 crate / 跨层（前后端）摸代码
- 任务描述里提到"调研 / 了解 / 看下 / 怎么做的"等探索动词

→ 用 Explore：发 `Agent({ subagent_type: "Explore", prompt: ... })`，把上下文卸到子代理
→ 不用 Explore：目标文件已知（用户给了文件名 / 行号 / 明确符号），直接 Read

### 触发词表（本节是项目内唯一权威源）

根 `CLAUDE.md::What to do first` 第 0 条引用本段判断"是否第一时间跑 preflight"。其它地方**不要**再维护副本——历史上同一概念三处独立维护改一处漏两处的坑见 commit b77fdd7 的 codex 二审报告。

**开工信号词**（任一命中视为用户开工）：
- 命令类："开始 / 做一下 / 帮我实现 / 帮我做 / 先做 X 再做 Y / 修这个 bug / 加个功能 / 重构 X"
- 任务类："实现 X / 加 X / 修 X / 改 X / 优化 X / 调整 X / 删除 X / 增加 X / 替换 X"
- 描述类（含具体目标）："让 X 支持 Y / 把 X 改成 Y / X 应该 Y / X 有 bug，Y / X 不工作，Y"
- 显式 skill：`/<任意 skill>` 含项目执行语义（如 `/release-runbook` / `/bump-version`）

**停手词**（任一命中视为用户**不**想立刻开工，跳过 preflight + 后续流水线）：
- 探询类："看一下 X / 了解一下 X / 解释一下 X / X 是什么 / 怎么做 / 为什么"
- 评估类："这个方案怎么样 / 这样改 OK 吗 / 你觉得 / 你认为"
- 只读类："列一下 X / 找一下 X / 搜一下 X / show me X / 给我看 X"
- 审查类："审查 X / review X / 验证 X / 检查 X / audit X / 评估 X"——只读评估意图
- 对照类："对比 X 和 Y / 比较 X/Y / 核对 X / diff 一下 X"——只读判断
- 追问类："是否 X / 有没有 X / 确认一下 X / 帮我判断 X / X 对吗"——不一定要动手
- 诊断类："定位原因 / 排查一下 / debug 看看 / 查为什么失败 / 看下报错"——终点是"给原因 + 修法建议后停手"，**不等于**修代码
- 显式停止类："先别做 / 暂时不要 / 只回答 / 仅讨论 / 不要改代码"

**审查 / 诊断类的开工升级 escape clause**（避免误判）：以上停手词若与**明确动手词**同句出现才升级为开工——动手词清单：`直接修 / 修改代码 / 修掉 / 改一下 / 提交 / push / 发 PR / 跑 PR`。**仅**说"修法建议"或"给我修复方向"**不**升级。例：
- "审查 PR + 修掉里面的 bug" → 升级开工
- "审查 PR 给我修法建议" → 停手（讨论后等用户决策）
- "定位原因 + 把 bug 修掉" → 升级开工
- "定位原因 + 告诉我怎么修" → 停手

**未触发任一方**：默认按开工信号处理（CLAUDE.md "What to do first" 第 0 条"判断不准默认走 openspec"同源原则）。

**流水线终点判断**（preflight Q4 输出"下一步"时引用）：
- 开工信号词 + 无停手词 → 终点是 "PR push → CI 全绿 → codex 通过 → archive（如 openspec change）→ 文本总结"
- 停手词 → 终点是 "回答完用户提问 / 给出方案后停手等用户决策"

### Step 3：把结果给用户，等他确认后才进入实现

格式（给用户的回复）：

```
## Preflight check

- origin/main 同步：<已 fetch / 落后 N / 离线> ✅/⚠️
- 分支：<branch> ✅/❌
- OpenSpec：<是/否>，理由：<一句>
- Explore：<是/否>，理由：<一句>

下一步：<具体行动>

确认开始？
```

用户回 "ok / 开始 / 嗯" 等明确信号 → 才发实质性 Edit/Write/Bash 工具。

用户给修正（"不用 OpenSpec / 直接做"）→ 按修正走。

## 不要做

- 不要 Edit/Write 任何文件（这是只读决策 skill；唯一例外是 `git fetch`）
- 不要把 preflight 输出过长——4 行 + 1 个问题，不超 150 字
- 不要在用户已经在 feature 分支 + 任务明显简单 + origin/main 同步的时候把 preflight 当仪式跑——"全绿，不走 OpenSpec / Explore"3 行带过就行
- 不要因为 `git fetch` 失败就阻塞——离线照样能改本地代码，只是 worktree 起点可能过期
