# Background PR prompt 骨架（占位符填空）

bg session 自己会读 `.claude/rules/{rust,perf,opsx-apply-cadence,codex-usage,bg-task-dispatch}.md` + `CLAUDE.md`——**不要**在 prompt 里重抄规则内容。本骨架只列任务格式 + 关键不变量。

填空后直接 inline 起：

```bash
just bg-pr <name> '<填好的 prompt>'
```

或长 prompt 落文件：`just bg-pr <name> .claude/perf-prompts/<name>.md`

---

EnterWorktree name=`<worktree-name>` → `git checkout -b <feat|fix>/<slug>`

任务：
1. `<文件:行号或路径>` — 现状 / 修法 / 影响
2. `...`

走 openspec：`<是/否，理由>`。是 → 先 `/opsx:propose <slug>` 写 design.md + tasks + delta + validate，再 apply。

跑完整流水线：`just preflight` → push → codex 二审（重点查 `<怀疑点 1,2,3>`）→ wait-ci 全绿 → **不 merge** → 最终回复带 `result:`

- 性能 PR：PR 描述加 Perf impact 段（见 `.claude/rules/perf.md`）
- openspec change：archive 是 PR 最后一个 commit（见 `.claude/rules/opsx-apply-cadence.md` 发布尾段）
- 卡 30+ 分钟 / 需要异构视角 → 主动调 `Agent({ subagent_type: "codex:codex-rescue", ... })`
