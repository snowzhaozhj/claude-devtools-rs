---
name: debug-first
description: 已开始排查 bug 时的诊断节拍——4 步主流程 + 3 条贯穿约束防"光读代码瞎猜"。**只要**用户报"X 不工作 / 还是有问题 / 上次修的还在 / 为什么挂 / 怎么回事 / 跑不通 / 跑不起来 / fail 在哪 / 报错 / 异常 / 出问题了 / 不正常 / 行为不对 / 调一下 / 调查一下 / 看下为什么 / 定位下 / debug / code-review 发现 / PR 还有问题"或显式 `/debug-first`，**都用这个 skill** 先过一遍。**与 preflight 互补**：`修 bug / 实现 X / 帮我做 Y` 等开工信号走 `preflight`（开工 4 件套）；`为什么 / 排查 / 定位 / 不工作 / 还是有问题` 等诊断信号走本 skill（不动业务代码，只约束诊断流程）。
allowed-tools: Bash, Read, Grep, Glob
---

# debug-first

> 触发：`/debug-first` 或诊断信号词
> 输出：按 4 步节拍走，每步给"硬约束 + 跳出条件"
> 不修改业务代码——只**约束诊断流程**，结论给用户拍板后再动手

## 为什么有这个 skill

排查 bug 时模型会习惯性"读两眼代码直接猜原因 → 给方案 → 让用户监工验证"。三个常见失败模式：

1. **静态推理 vs 真复现**：代码逻辑能列可能性，但只有真数据告诉你**实际**走的是哪条路径。曾经第一轮我推 "前端 inflight race 是真因"，跑端到端真后端后发现根本是后端 SFTP channel 死掉了 — 推理全错。
2. **环境假设**：以为"复现需要 X 我没有"就停手让用户跑，常被反问"之前不是配过吗"。十次有八次现成 docker / fixture / e2e 脚本就在 `scripts/` 里。
3. **诊断浮于表面**：单次复现就下结论 / 不标置信度 / 把不相关 bug 顺手修扩大 scope，让 reviewer 反复抓 critical race。

## 4 步主流程（按顺序执行）

| Step | 关键动作 | 跳出条件 |
|---|---|---|
| 1 | 复现 + 调高 log + 多时间点 trial | 无外部状态依赖（无 IPC / 文件 I/O / 并发 / 网络 / SSH）的本地纯函数 bug |
| 2 | 跨端数据不一致先 `ps`/`lsof` 排进程残留 | 单进程 / 单 runtime 的 bug |
| 3 | 假设环境缺失前先 `grep` 基础设施 | 已确认环境齐 |
| 4 | 诊断报告标置信度 + 方案分级 | 单 bug 100% 确认 + 一行 fix |

### Step 1: 复现 + 调 log + 多时间点 trial

涉及"多端交互 / 状态机 / 时间窗口 / 并发 / 网络 / SSH"的 bug，**SHALL 真复现 + 看真数据**再猜原因。三件事一起做：

**真复现**：
- 优先用 `e2e-http-verify` skill（已配 `cdt-cli` HTTP server + 浏览器 `?http=1` 端到端真后端）
- SSH 类走 `just verify-ssh-docker` / docker `cdt-ssh-test` 容器
- 浏览器交互类用 chrome-devtools mcp + `evaluate_script` 看真 store 状态

**调高 log level 比读代码快 10×**：怀疑哪个 module 就 `RUST_LOG="<module>=debug"`：

```bash
RUST_LOG="cdt_api=info,cdt_ssh=debug,cdt_watch=info" cargo run -q -p cdt-cli --bin cdt > /tmp/cdt-debug.log 2>&1 &
```

加一次 debug log 后 log 直接打出真因（如 "session closed / projects root does not exist"）—— 比追代码追半天快。前端调试同理（`?debug=1` query / `localStorage.debug = '*'`）。

**多时间点 trial**：状态/时间相关 bug **不要**信单次复现。三种 trial 时机：

- 立即（T+0）
- 触发条件后（T+90s / T+N 个 polling 周期 / T+ idle timeout）
- 主动制造 race（如 `docker pkill sshd` + 立即 reconnect）

**跳出条件（硬）**：bug **无外部状态依赖**（无 IPC / 文件 I/O / 并发 / 网络 / SSH 远端）且是本地纯函数逻辑——可跳过本步直接读代码定位。其余默认必复现。

### Step 2: 跨端数据不一致先 ps/lsof 排进程残留

浏览器拿 N、curl 拿 M、cdt-cli 显示 K——三个不一致**第一动作**是排进程残留，不是追代码：

```bash
ps aux | grep -E "target.*cdt|cdt-cli" | grep -v grep | head -3
lsof -iTCP -sTCP:LISTEN 2>/dev/null | grep -E "cdt|3456|5173" | head -5
```

典型陷阱：旧 cdt-cli 残留 + 新 cdt-cli 一起跑，vite proxy 接到旧的死 SSH session → 浏览器拿到部分老数据，差点误判前端 bug。`pkill -f 'target/.*/cdt$'` + 重启 → 干净复现。

### Step 3: 假设环境缺失前先 grep 基础设施

开始动手前 30 秒查现有工具，避免"假设缺 X 让用户跑"被反问：

```bash
# 看 e2e / docker / fixture 工具
grep -rn "docker\|e2e\|fixture\|verify" justfile scripts/ 2>/dev/null | head -20
# 看 just recipes 全集
just --list 2>&1 | head -30
# 找已有 skill
ls .claude/skills/
```

发现工具齐备就直接用；真缺再让用户拉环境。**绝不**把"我没复现工具"当结论甩给用户。

### Step 4: 诊断报告标置信度 + 方案分级

给用户的报告必须按模板：

```markdown
## Bug X（100% 确认 / 高怀疑 60% / 推测）

**触发链**：[T+0 → T+N 时序]
**root cause**：[文件:行 + 一段引文]
**修复方案**：[一行 / 多文件 / 架构层 — 风险递增]
```

- 每个 bug 标**置信度**：`100% 确认 / 高怀疑 60% / 推测`
- 每个方案标**风险等级**：`一行 / 多文件 / 需走 openspec`
- 多 bug 时**分别**给结论，不混在一起

让用户按置信度决定"直接修 vs 先 spike vs 走 openspec"，**不要**替用户拍板。

## 3 条贯穿全程的约束（cross-cutting）

### 顺路抓到的不相关 bug：开 GitHub Issue（默认 `bug` label），不立刻修

调查中发现 N 个相关但非本次任务的 bug → **立即 `gh issue create --label bug --title "..." --body "..."`** + 不就地修。PR scope 越小越好审。例外：与当前 bug 同一用户感知路径的小修可顺路做，但不要把"需要正经 design"的工作顺手扩到 PR 里。归宿规则详 `CLAUDE.md::遗留事项归宿`。

### 加测试后 SHALL 反转 fix 验证抓得到回归

加完防回归测试，把 fix 暂时改回 buggy 状态跑一次，确认测试真 fail；再改回 fix。否则可能写了恒 pass 的伪测试。完整 cycle：
1. 改回 buggy → 跑测试 → 应 fail（"红"）
2. 改回 fix → 跑测试 → 应 pass（"绿"）
3. 才 commit

### 需要架构设计的工作进 followups，**不**拖当前 PR

调查中识别出"这条要正经写 design.md + 跑 openspec"的工作 → 写 followups + 列 D1/D2/D3 开放问题 + **保持当前 PR scope 不变**。短期可在 PR 内加最小自愈让用户不被困住，根治方案独立 change 慢慢做。

## 与其他 skill 的边界

| skill | 何时用 |
|---|---|
| **debug-first**（本 skill） | 已确认开工后进入**诊断阶段**的节拍 |
| `preflight` | **开工前** 4 件套（fetch / 分支 / openspec / Explore），与本 skill 互斥（开工信号 vs 诊断信号）|
| `e2e-http-verify` | 本 skill Step 1 的复现工具之一（HTTP server + 浏览器 `?http=1`）|
| `perf-bench` | 性能类 bug 的 Step 1 跑数据入口（SessionDetail 首屏 / IPC payload）|
| `wait-ci` | 诊断完进入实施后 push PR 等 CI 时用 |

诊断完进入实施阶段后，按 `.claude/rules/opsx-apply-cadence.md` 推进；codex 二审默认按 `.claude/rules/codex-usage.md`。
