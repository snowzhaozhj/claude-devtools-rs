---
name: preflight
description: 开工 30 秒自检——分支 / OpenSpec 适用性 / 探索方式三件套。用户 `/preflight` 或自然语言"开始干 / 帮我做 X"等开工信号时主动跑一遍，避免落入"在 main 直接编辑 / 跳过 OpenSpec / 不用 Explore subagent"三个高频 friction。
---

# preflight

> 触发：`/preflight` 命令；或用户开工信号（"开始做 X / 帮我实现 Y / 先做 A 再做 B"）
> 输出：3 个检查结果 + 给用户的简短确认问题
> 不修改任何文件——只读 + 决策。

## 为什么有这个 skill

`/insights` 报告反复点名同一类 friction：
- "skipping branch creation" → 在 main 上编辑（已配 hook 拦截，但仍有路径白名单空隙）
- "skipping OpenSpec evaluation" → 行为契约改动事后补 spec
- "skipping Explore subagent" → 主动 grep / Read 污染主上下文

CLAUDE.md 第 1-2 条已经写了这套规则，但 Claude 在"看上去很急"的请求下会跳。这个 skill 强制把"跳"成本提前——开工先回答 3 个问题，再触工具。

## 工作步骤

### Step 1：跑诊断命令（并行）

```bash
git branch --show-current
git status --short
ls openspec/changes/ 2>/dev/null | grep -v "^archive$"
```

输出三块：当前分支 / 未跟踪/未提交文件 / 进行中（未 archive）的 openspec change 列表。

### Step 2：回答 3 个问题（自检后给用户看）

**Q1. 当前在 feature 分支吗？**
- 当前分支非 `main` / `master` → ✅ 进入 Q2
- 当前分支 = `main` 或 `master` → ❌ 报告"开工前需要 `git checkout -b feat/<slug>` / `fix/<slug>`"，给一个 slug 建议（基于用户描述的任务），等用户确认后再切

**Q2. 这个任务该走 OpenSpec 流程吗？**

判断标准（任一命中就走 OpenSpec）：
- IPC 字段 / 后端算法 / 状态判定 / 数据 omit 策略 / Tauri command 协议
- 跨 capability 的行为契约改动
- 性能 / 节流 / 缓存 / 并发改动

**直接 commit 的场景**（不走 OpenSpec）：
- 纯视觉对齐 / 单点样式修复 / Trigger CRUD / 文案修正
- bump version / docs / chore

→ 走 OpenSpec：执行 `/opsx:propose <slug>` 或建议用户跑
→ 直接 commit：进入 Q3

**已有进行中 change 时**：检查 Step 1 的列表里有没有相关 slug——有的话提醒"`<slug>` 已 propose，是否在它内部继续 apply 而非新开"

**判断不准默认走 openspec**（CLAUDE.md 第 5 条）。

**Q3. 探索环节用 Explore subagent 吗？**

判断标准（任一命中就用 Explore subagent）：
- 任务需要 grep / Read 超过 3 个文件来定位入口
- 需要跨 crate / 跨层（前后端）摸代码
- 任务描述里提到"调研 / 了解 / 看下 / 怎么做的"等探索动词

→ 用 Explore：发 `Agent({ subagent_type: "Explore", prompt: ... })`，把上下文卸到子代理
→ 不用 Explore：目标文件已知（用户给了文件名 / 行号 / 明确符号），直接 Read

### Step 3：把结果给用户，等他确认后才进入实现

格式（给用户的回复）：

```
## Preflight check

- 分支：<branch> ✅/❌
- OpenSpec：<是/否>，理由：<一句>
- Explore：<是/否>，理由：<一句>

下一步：<具体行动>

确认开始？
```

用户回 "ok / 开始 / 嗯" 等明确信号 → 才发实质性 Edit/Write/Bash 工具。

用户给修正（"不用 OpenSpec / 直接做"）→ 按修正走。

## 不要做

- 不要 Edit/Write 任何文件（这是只读决策 skill）
- 不要把 preflight 输出过长——3 行 + 1 个问题，不超 100 字
- 不要在用户已经在 feature 分支 + 任务明显简单时把 preflight 当仪式跑——"分支没问题、不走 OpenSpec、不用 Explore"3 行带过就行
