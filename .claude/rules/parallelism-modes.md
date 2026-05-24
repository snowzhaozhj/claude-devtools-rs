# 并行执行形态分派规则

> 覆盖三种并行执行形态：**subagent / agent team / bg**。任何"派多个 claude 干活"或"主 session 之外起执行单元"前 SHALL 按本文评估。

## 三形态精确定位

| 形态 | 机制 | 通信 | 生命周期 | context | 适合 |
|---|---|---|---|---|---|
| **Subagent**（Agent tool） | 主 session 一次性 spawn 子代理 | 只回报给主 session；多轮用 SendMessage 接续；subagent **之间不能互通** | 一次性，跑完返回摘要 | 摘要回注主 session（会污染主 context） | 单次查询 / 并行多视角审查 / Explore 探查 |
| **Agent team**（实验性 v2.1.32+） | 跨进程独立 Claude Code 实例，需 `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` | **Mailbox 互通** + 共享 task list，teammate 之间 peer 直发消息不经 lead | 持续在线直到 lead 关闭 | 每个 teammate 独立 context，**不污染 lead** | **单个**长期 / 复杂 / 多角色协作的大 PR |
| **bg job**（`claude --bg`） | 独立进程独立 session 独立 worktree | 完全脱钩，主 session 不参与 | 自主推进到流水线终点 | 完全独立 | **N≥2 个独立完整 PR** 并行启动 |

**关键区别**：subagent 之间无法互通必须靠 lead 当传声筒（lead context 会被堆满）；agent team 是 peer-to-peer 直接通信（lead 保持精简）；bg 是另一台机器跑完整 PR。

## 形态选择决策树

按改动**规模 + 协作复杂度**选。判断不准默认从左往右降级（subagent → 主 session 自跑）：

| 改动场景 | 选哪个 | 理由 |
|---|---|---|
| 单点 PR（< 半天，单 capability，单 surface） | **主 session 自跑** + subagent 按需调（codex / Explore / impeccable critique 一次性） | 启动开销值不回 |
| 中等 PR（半天-2 天，前后端混合但流水线线性） | 主 session + **多 subagent 并行 review**（一个 message 多 Agent tool call） | 多视角并行审值回；不需要 teammate 长协作 |
| **大改动**：`> 2 天工作量 AND (多角色协作 OR 视觉重构 OR 跨 capability)` 中任一特征命中 | **Agent team**（lead + 设计师 + 前端 + 后端 + QA，Mailbox 互通） | lead 不当传声筒；多角色独立 context 不互相污染 |
| **N≥2 个独立完整 PR 同时推**（每个 ≥ 半天，主 session 想脱钩做别的） | **N 个 bg job** | bg = 完整独立 PR 自治启动器 |
| 业务在主 session 写完，剩 push / wait-ci / codex / archive | **主 session 直接跑**（不开 bg） | 尾段 5 分钟搞定；开 bg 反要重读 context |

**禁止**：
- 用 subagent 跑**整条 PR 推进流水线**（preflight → 实现 → push → wait-ci → archive）——长阻塞会卡主 session 或 context 爆。例外：codex-rescue 二审本身是 subagent 调用、单轮通常 ≤ 几分钟、`SendMessage` 接续多轮也是允许用法（见 `codex-usage.md`）；这里禁的是把"整条流水线"打包给一个 subagent 跑
- 把单个 PR 拆"前端 bg + 后端 bg" —— 破坏 PR 原子性，该用 agent team
- 把"业务做完后的尾段推进"丢给 bg —— 尾段开销 < 启动开销

## 拆分前判断框架（4 个 ✓ 全满足才拆 PR）

适用于决定"该不该拆 N 个 bg job"。社区共识：拆 PR 不是免费的。每多一个 PR 多一份 codex / CI / review / 合并开销。拆分目的是"并行省 wall time"+"功能独立 review 聚焦"，没这两个目的就别拆。

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

## Agent team 启用与限制

启用：`CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` 已在本仓生效（shell env 或 `settings.json` 任一即可，需 Claude Code v2.1.32+）。lead 自然语言起 team，无需手写配置。

硬限制（评估值不值得开 team 时看）：
1. 同时只能存在 1 个 team
2. 不能嵌套（teammate 不能再 spawn 子 team）
3. 代价：每多一个 teammate ≈ 1× context 成本，且不支持 session resume

**Agent team 与 bg job 共存**：bg job 用于"独立完整 PR 在另一个 worktree 自治推进"，agent team 用于"单个 PR 内多角色协作"。二者可并存（bg 跑 PR-X 同时主 session 起 team 跑 PR-Y），但**不能同 PR 内混用**——同 PR 多 worktree 会破坏分支原子性。

**teammate 资产**：可复用 teammate 定义沉淀在 `.claude/agents/`：
- 实施型（写代码）：`designer` / `frontend-engineer` / `backend-engineer`
- 验证型（常驻 team 跑端到端真数据）：`qa-engineer`
- 审查型（只读 ad-hoc 调，不立 teammate）：`rust-conventions-reviewer` / `spec-fidelity-reviewer` / `tauri-config-reviewer` / `ui-reviewer` / `windows-compat-reviewer`

QA 与 reviewer 不重叠：reviewer 静态审 PR diff（lead 按域 ad-hoc 触发）；`qa-engineer` 在 team 内常驻**会跑**测试（`e2e-http-verify` / Playwright / 真启 `just dev` 桌面端 smoke），抓 mockIPC fixture ≠ 真后端数据这种伪覆盖。

具体角色组合由 lead 按 change 性质裁剪。teammate 之间 Mailbox 直发不经 lead，省 lead context；典型通信路径已在各 teammate 文件 `## 协作` 段定义。

## bg job 启动 / 监控

```bash
just bg-pr <name> '<inline prompt>'   # 起 bg session 跑完整 PR 流水线（quote() 安全编码 backtick / 双引号 / $）
just bg-status                         # 状态摘要（不直接读 ANSI raw log）
just bg-stop-all                       # 停所有
just bg-clean <id>                     # 停 + 删 worktree
```

prompt 骨架（含占位符 + 关键不变量）：`.claude/templates/bg-pr-pipeline.md`——**禁止**手写 `claude --bg "..."` 绕过 `bg-pr`（inline 引号嵌套被 shell 吃过坑）。

## bg job 已踩坑速查

1. **不要加 `--permission-mode bypassPermissions`**——classifier 拒，需 explicit 授权；默认模式已能跑完整流水线
2. **prompt 一次写全**——bg session 起后无法非交互注入指令（`claude attach` 是 TUI 不接 stdin）；prompt 列任务范围 + 起点 + 怀疑点 + 完成条件 + 走不走 openspec 即可，规则与 spec bg session 自己读
3. **log 是 ANSI 流难解析**——用 `just bg-status` 提炼摘要不直接读 raw log
4. **不要 merge 留用户**——bg 跑到 codex + wait-ci 全绿即止，merge 是 destructive shared state 留用户拍板
