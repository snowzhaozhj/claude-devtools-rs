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

一行 just recipe（推荐）：
```bash
just bg-pr <name> .claude/perf-prompts/<prompt-file>.md
```

或裸命令（subshell 隔离主 session cwd）：
```bash
(cd /path/to/repo-root && \
  claude --bg --name "<PR 名>" --effort high \
    "$(cat .claude/perf-prompts/<prompt-file>.md)")
```

监控 / 清理：
```bash
just bg-status        # 列所有 bg session 状态摘要
just bg-stop-all      # 停所有
just bg-clean <id>    # 停 + 删 worktree
```

prompt 模板：`.claude/templates/bg-pr-pipeline.md`（通用，填空即用，不绑业务）。

## 已踩坑速查

详见 `~/.claude/projects/<encoded>/memory/feedback_bg_claude_dispatch.md`。简版 6 条：

1. **不要加 `--permission-mode bypassPermissions`**——classifier 拒，需 explicit 授权；默认模式已能跑完整流水线
2. **prompt 文件 SHALL 在 `.claude/*` 子目录下**——hook 拦 main 分支 Edit `/tmp/*` 等非白名单路径
3. **prompt 一次写全**——bg session 起后无法非交互注入指令（`claude attach` 是 TUI 不接 stdin）
4. **prompt 漏维度 = 所有 PR 都漏**——SHALL 用模板填空，硬约束默认带上不省略
5. **log 是 ANSI 流难解析**——用 `just bg-status` 提炼摘要不直接读 raw log
6. **不要 merge 留用户**——bg 跑到 codex + wait-ci 全绿即止，merge 是 destructive shared state 留用户拍板
