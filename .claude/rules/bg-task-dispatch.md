# Background 任务分派规则

用 `claude --bg` 起独立后台 session 跑并行任务。本文是**硬约束**——任何"派多个 claude 干活"前先按此规则评估。

## 拆分前判断框架（4 个 ✓ 全满足才拆）

社区共识：拆 PR 不是免费的。每多一个 PR 多一份 codex / CI / review / 合并开销。拆分目的是"并行省 wall time"+"功能独立 review 聚焦"，没这两个目的就别拆。

| 检查 | 内容 |
|---|---|
| ✓ 技术独立 | 不同模块/文件/函数，修改区域不重叠 |
| ✓ 独立可验证 | 每个拆后 PR 单独能 review / 测试 / revert |
| ✓ 工作量值得 | 总改动 > 1-2 小时；3 行改动凑成单独 PR 反而增加总开销 |
| ✓ wall time 价值 | 用户在等结果 / 多 reviewer 能同时审 |

**任一不满足 → 合并成 1 个 PR 更优**。反例：3 个同文件不同函数的优化拆成 3 个 PR，codex/CI 跑 3 次浪费 15-30 min vs 合并 1 PR 跑 1 次。

## 已验证的合并策略

| 模式 | 合并 / 独立 | 理由 |
|---|---|---|
| 同 crate 同文件不同函数 N 个小优化 | **合并 1 PR** | reviewer 一次看完上下文清晰，codex/CI 跑一次省 N 倍 |
| 同 crate 不同文件相关功能 | 视影响半径 | 同源（如启动路径）合并；分散则拆 |
| 新增模块 / 走 openspec | **独立 1 PR** | 行为契约级，单独审 spec delta + design.md 更专注 |
| 跨 crate 修改 | 视依赖 | 依赖紧（一处改另一处必改）合并；松（可独立 revert）拆 |

## bg vs subagent 选择

| 场景 | 用哪个 |
|---|---|
| N≥2 个 PR 完整流水线（实施→push→codex→wait-ci→archive） | `claude --bg`（独立进程 + worktree + 长流水线自治） |
| 单次查询 / 一次性任务 / 短返回 | Agent 工具（subagent） |
| 长时间 watch / 监控 / 异步轮询 | `claude --bg` |
| 主 session 上下文敏感（不想被淹） | `claude --bg`（完全隔离） |
| 需要主 session 立即用结果继续 | Agent 工具（直接 return） |

**禁止**：用 subagent 跑"含 wait-ci / 多轮 codex / 多分钟阻塞"的长流水线——subagent 会卡主 session 或 context 爆。

## 启动样板

`bg-pr` recipe 用 just `quote()` 把 NAME / PROMPT 编码为 shell-safe 单引号字面量，含 backtick / 双引号 / `$` / `$HOME` 等特殊字符也能原样传入：

```bash
just bg-pr <name> '<inline prompt>'
# 含特殊字符也安全：
just bg-pr fix-foo '修 `fn foo()` 的 bug，参考 "原版行为" 与 $HOME 路径'
```

prompt 列**任务范围 + 起点 + 怀疑点 + 完成条件**就够，bg session 自己会读 `.claude/rules` / `CLAUDE.md` / `openspec/changes/<slug>/`。**禁止**手写 `claude --bg "..."` 绕过 `bg-pr` —— 历史踩过 inline 双引号嵌套被 shell 吃的坑。

裸命令（subshell 隔离主 session cwd）：
```bash
(cd /path/to/repo-root && claude --bg --name "<name>" --effort high "<prompt>")
```

监控 / 清理：
```bash
just bg-status        # 列所有 bg session 状态摘要
just bg-stop-all      # 停所有
just bg-clean <id>    # 停 + 删 worktree
```

prompt 骨架：`.claude/templates/bg-pr-pipeline.md`（占位符 ~20 行；**不要**在 prompt 里重抄 `.claude/rules` / `CLAUDE.md` 内容——bg session 自己会读）。

## 已踩坑速查

详见 `~/.claude/projects/<encoded>/memory/feedback_bg_claude_dispatch.md`。简版：

1. **不要加 `--permission-mode bypassPermissions`**——classifier 拒，需 explicit 授权；默认模式已能跑完整流水线
2. **prompt 一次写全**——bg session 起后无法非交互注入指令（`claude attach` 是 TUI 不接 stdin）；列任务 + 起点 + 怀疑点 + 完成条件 + 走不走 openspec 即可，规则与 spec bg session 自己读
3. **log 是 ANSI 流难解析**——用 `just bg-status` 提炼摘要不直接读 raw log
4. **不要 merge 留用户**——bg 跑到 codex + wait-ci 全绿即止，merge 是 destructive shared state 留用户拍板
