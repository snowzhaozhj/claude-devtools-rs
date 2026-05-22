---
name: debug-first
description: 排查 bug 的节拍——9 条防"静态推理瞎猜"的硬约束。**只要**用户报"X 不工作 / X 有 bug / 排查 X / 复现 / 不是说好修了吗 / 还是有问题 / 调一下 / 看下为什么 / 定位下 / debug"或显式 `/debug-first`，**都用这个 skill** 先过一遍——避免落入"光读代码猜原因 / 假设缺工具但其实环境齐 / 单次复现就下结论 / 误把残留进程当代码 bug / 没开 debug log 追代码追半天 / 顺路 bug 拖入 scope / 测试加完不反转验证 / 不标置信度让用户当监工 / 把架构活混进 bug 修复 PR"九个本仓血泪坑（来自 PR #205 SSH bugs 调查全程）。
---

# debug-first

> 触发：`/debug-first`；或用户报 bug / 排查信号词
> 输出：按 6 步节拍走，每步有"硬约束 + 触发条件 + 跳出条件"
> 不修改业务代码——只**约束诊断流程**，结论给用户拍板后再动手

## 为什么有这个 skill

排查 bug 时 Claude 会习惯性"读两眼代码直接猜原因 → 给方案 → 让用户监工验证"。三个常见失败模式：

1. **静态推理 vs 真复现**：代码逻辑能列可能性，但只有真数据告诉你**实际**走的是哪条路径。例：PR #205 第一轮我推 "前端 inflight race 是 Bug 1 root cause"，跑端到端 docker SSH 后真因是后端 SFTP channel 死亡——前端推理全错。
2. **环境假设**：假设"复现需要 X 我没有"就停手让用户跑，常被打脸："之前不是配过吗"。例：PR #205 调查时我以为"缺 SSH 远端"，用户提醒 docker 已配 → 一查 `scripts/verify-ssh-docker-e2e.sh` + `cdt-ssh-test` 容器现成。
3. **诊断浮于表面**：单次复现就下结论 / 不标置信度 / 把不相关 bug 顺手修扩大 scope。这些坑让 reviewer 的 codex 二审反复抓 critical race（PR #205 codex 抓了两轮 critical）。

## 6 步节拍（按顺序执行）

### Step 1: 复现优于推理

涉及"多端交互 / 状态机 / 时间窗口 / 并发"的 bug，**SHALL 真复现 + 看真数据**再开始猜原因。

- 优先用 `e2e-http-verify` skill（已配 `cdt-cli` HTTP server + 浏览器 `?http=1` 端到端真后端）
- SSH 类走 `just verify-ssh-docker` / docker `cdt-ssh-test` 容器（端口 2222，user `devuser`）
- 浏览器交互类直接 chrome-devtools mcp + `evaluate_script` 看真 store 状态

**跳出条件**：纯文档 / typo / 一行明显逻辑 bug 可跳过；其他默认必复现。

### Step 2: 假设环境缺失前 SHALL 先 grep 基础设施

开始动手前 30 秒查现有工具，避免"假设缺 X 让用户跑"：

```bash
# 跑过哪些 e2e / docker / fixture
grep -rn "docker\|e2e\|fixture\|verify" justfile scripts/ 2>/dev/null | head -20
# 看 just recipes 全集
just --list 2>&1 | head -30
# 找已有 skill
ls .claude/skills/
```

发现工具齐备就直接用；真缺再让用户拉环境。**绝不**把"我没复现工具"当结论甩给用户。

### Step 3: 跨端数据不一致先看进程归属

浏览器拿 N、curl 拿 M、cdt-cli 显示 K——三个不一致**第一动作**是排进程残留，不是追代码：

```bash
ps aux | grep -E "target.*cdt|cdt-cli" | grep -v grep | head -3
lsof -iTCP -sTCP:LISTEN 2>/dev/null | grep -E "cdt|3456|5173" | head -5
```

例：PR #205 调查中浏览器拿 16 / curl 拿 0，差点误判前端 bug。其实是旧 cdt-cli 残留 + 新 cdt-cli 一起跑，vite proxy 接到旧的死 SSH session。`pkill -f 'target/.*/cdt$'` + 重启 → 干净复现。

### Step 4: 复现前先调高 log level

读代码追逻辑 vs 加 `RUST_LOG=cdt_ssh=debug` 后跑一遍——后者 10 秒给答案：

```bash
RUST_LOG="cdt_api=info,cdt_ssh=debug,cdt_watch=info" cargo run -q -p cdt-cli --bin cdt > /tmp/cdt-debug.log 2>&1 &
```

例：PR #205 加 `cdt_ssh::lifecycle=debug` 后 log 直接打 "session closed / projects root does not exist"，root cause 自现。

**经验法则**：怀疑哪个 module 就 `<module>=debug`；前端调试同理（`?debug=1` query / `localStorage.debug = '*'`）；不要先读 500 行代码。

### Step 5: 状态/时间相关 bug 多次不同时间点复现

"修好了没"的判断 SHALL 多次 trial 不同时间点，**不要**信单次复现：

- 立即 trial：T+0
- 等触发条件后 trial：T+90s / T+几个 polling 周期 / T+ idle timeout
- race 类 bug：手动制造 race（如 `docker pkill sshd` + 立即 reconnect）

例：PR #205 第一次 fresh 跑能拿 42 SSH groups，等 90s 再跑变 0——单次复现说"已修"会漏掉时间窗口的真 bug。

### Step 6: 诊断报告标置信度 + 方案标风险

给用户的报告必须：

- 每个 bug 标**置信度**：`100% 确认 / 高怀疑 60% / 推测`
- 每个方案标**风险等级**：`一行改动 / 架构改动 / 需走 openspec`
- 多 bug 时 **分别** 给结论，不混在一起

模板：

```markdown
## Bug X（100% 确认 / 高怀疑 / 推测）

**触发链**：[T+0 → T+N 时序]
**root cause**：[文件:行 + 一段引文]
**修复方案**：[1 行 / 多文件 / 架构层]
```

让用户按置信度决定"直接修 vs 先 spike vs 走 openspec"，**不要**替用户拍板。

## 三条贯穿全程的约束

### 顺路抓到的不相关 bug：记 followups.md，**不**立刻修

调查中发现 N 个相关但非本次任务的 bug → **立即写 `openspec/followups.md`** 条目 + 不就地修。PR scope 越小越好审。

例：PR #205 调查 SSH bug 时顺路发现 `PushEvent` 缺 `ContextChanged` variant + `transport.ts` 字段名错配——本次顺手修了（属同一用户感知路径），但 SSH keepalive 根治方案独立条目写进 followups。

### 加测试后 SHALL 反转 fix 验证抓得到回归

加完防回归测试，把 fix 暂时改回 buggy 状态跑一次，确认测试真 fail；再改回 fix。否则可能写了恒 pass 的伪测试。

例：PR #205 Bug 2 sidebar finally guard 加完 vitest 后反转 fix → 测试果然 fail（".sidebar-status-inline" 还在显示）→ 改回 fix → 测试 pass ✓ 才合 commit。

### 需要架构设计的工作进 followups，**不**拖当前 PR

调查中识别出"这条要正经写 design.md + 跑 openspec"的工作 → 立刻写 followups + 列 D1/D2/D3 开放问题 + **保持当前 PR scope 不变**。

例：PR #205 真因有"SSH/SFTP keepalive 根治"的架构需求，写 followups 留独立 `ssh-keepalive-liveness` change，当前 PR 只做短期自愈（polling 连错自动 disconnect）让用户不再卡 stale active。

## 与其他 skill 的边界

| 这个 skill | 别的 skill |
|---|---|
| 排查 bug 的诊断节拍 | `preflight` 是开工节拍（fetch / 分支 / openspec / Explore） |
| 6 步约束诊断流程不浮于表面 | `e2e-http-verify` 是 Step 1 复现工具之一（HTTP server + 浏览器） |
| 决策"修 vs followups vs openspec" | `perf-bench` 是性能类 bug 的 Step 1 跑数据入口 |
| 不动业务代码 | 修业务代码走对应 skill / 直接 Edit |

诊断完进入实施阶段后，按 `.claude/rules/opsx-apply-cadence.md` 推进。
