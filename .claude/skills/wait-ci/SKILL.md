---
name: wait-ci
description: Poll `gh pr checks <pr>` 直到全绿或失败；失败时自动 `gh run view --log-failed` 过滤 FAILED/panicked/error 行提炼真正的错误。**只要**刚 `git push` 完 / 用户说"等 CI / CI 过了没 / 看 CI 结果 / CI 跑没跑完 / 跑完没"或显式 `/wait-ci [pr]`，**都用这个 skill**——CLAUDE.md "What to do first" 第 6 条已把"PR push 后 SHALL `/wait-ci` 直到全绿"列为硬约束（`scripts/check-openspec-archives.sh` 等 CI-only check 本地 preflight 拦不下来），不能 push 完就走人。
---

# wait-ci

**触发**（任一即用）：
- 用户显式 `/wait-ci` 或 `/wait-ci 6`
- 自然语言"等 CI / CI 过了没 / 看 CI 结果 / 跑完没 / build 怎么样"
- Claude 自己刚 `git push` 后——SHALL 主动调（CLAUDE.md "What to do first" 第 6 条 + `.claude/rules/opsx-apply-cadence.md` 发布尾段 N.2）

不调用 = 默认违约——push 完就走人是 CLAUDE.md feedback_pr_must_be_mergeable 明确禁止的。

## 输入

- 可选 PR number。若省略：
  1. 先 `gh pr view --json number -q .number`（当前分支关联的 PR）
  2. 若无（未开 PR）：报告"当前分支没关联 PR"并退出

## 工作步骤

### 1. 确定 PR number

```bash
pr="${ARG:-}"
if [ -z "$pr" ]; then
  pr=$(gh pr view --json number -q .number 2>/dev/null || true)
fi
if [ -z "$pr" ]; then
  echo "No PR associated with current branch. Run: gh pr create"
  exit 1
fi
```

### 2. 看第一眼状态（不阻塞）

```bash
gh pr checks "$pr" 2>&1 | head -15
```

根据输出判断：
- 所有 `pass` → 报告"CI 全绿"并退出
- 有 `fail` → 跳到 Step 4
- 有 `pending` / `in_progress` / `queued` → 进入 Step 3 poll

### 3. 起后台 watch，等 harness 自动通知（首选方式）

`gh pr checks` 自带 `--watch`，内部 polling 直到所有 check 收敛——**全绿 exit 0、任一 fail exit 8**。配合 Bash 工具 `run_in_background: true`，命令完成时 harness 自动 task-notification 触发主 session：

```bash
gh pr checks "$pr" --watch --fail-fast --interval 30 2>&1 | tail -30
```

调用方式：Bash 工具 `run_in_background: true` + `timeout: 900000`（15 min 上限兜底 Tauri 矩阵 + perf bench 偶发慢跑），**不要**主动 poll bg 输出、**不要** `while sleep`、**不要** ScheduleWakeup 节奏。

**为什么是首选**：主 session 整个等待过程只多消耗 2 个 turn（起命令 + 处理 task-notification）。对比 ScheduleWakeup 每 270 s 重跑完整 prompt 一次（一次 CI 10-15 min = 3-5 个完整 turn 重读上下文 + 跑 `gh pr checks`），省 50-80 % token。

参数说明：
- `--watch`：流式监控直到收敛
- `--fail-fast`：任一 fail 立刻退出，不再等其他 check 跑完
- `--interval 30`：gh 内部 polling 间隔，默认 10 s 偏 burst；30 s 平衡延迟/请求量
- `tail -30`：watch 输出含 ANSI 重绘，只保留末尾终态行避免 transcript 噪声

收到通知后**一次性**读 output file 末尾几十行解析终态（不要重复读 inflate transcript）。退出码：
- exit 0 → CI 全绿，跳 Step 6
- exit 8 → 有 fail，跳 Step 4
- 其他（gh 异常 / Bash timeout）→ 报告状态让用户决定是否 retry

### 3 alt. ScheduleWakeup 回退路径

仅当 Step 3 首选方式不适用时用，例如：
- Bash run_in_background 在当前环境不可用
- 跑的不是 CI 而是**远端长任务** harness 看不到完成信号（远端队列、外部 webhook 触发的 workflow 等）
- 上一次 bg watch 真挂了 / 被 timeout kill，且重启 bg 不解决

回退用法：270 s 兜底唤醒一次（避开 5 min 缓存 TTL 边界），fire 时复跑 `gh pr checks "$pr" 2>&1 | head -20` 看一眼。判断关键词：
- `fail\|FAIL` → Step 4
- 无 `pending\|in_progress\|queued` → 全绿，Step 6
- 仍 pending → 再 ScheduleWakeup 一次（最多 3 轮，再不收敛报告用户）

### 4. 失败时拉日志 + 提炼错误行 + 自己定位 + 修

CI 红了 SHALL 自己 `gh run view --log-failed` 定位 + 修 + 再 push（CLAUDE.md feedback_pr_must_be_mergeable）——不要甩给用户当监工。

对每个 `fail` 的 job：
```bash
# 从 gh pr checks 输出解析出 job URL，末段是 run_id/job/job_id
gh run view <run_id> --log-failed --job <job_id> 2>&1 \
  | grep -iE "FAILED|panicked at|error\[|error:|assertion.*failed|TS\d+|svelte-check" \
  | head -30
```

过滤真正的错误行（跳过 `Downloaded / Compiling / Checking` 等正常噪音）。

输出给用户：
- 哪个 job 挂了（platform + step 名，如 `test (windows-latest) / cargo test --workspace`）
- 提炼的失败 test / panic 位置 / compile error
- 自己定位的根因（不止"哪个文件挂"——要给出"为什么挂"的判断）
- **自己动手修**——除非：
  - 修法需要业务决策（不只是机械修字段名 / 路径 / 字符串）
  - 修法可能影响别的 PR / 别的 capability，需要用户确认范围
  - 失败疑似 flaky（同一 job 重跑可能过）——这种情况报告用户，建议 `gh run rerun <run_id>` 或在 PR 描述里 hold-for-rerun 标记

### 5. 修完再走一轮

修完 push 第二个 commit 后 SHALL 再次走 Step 2-5——不要假设"应该过了"。

### 6. 全绿时报告

```
✅ PR #<N> CI 全绿（<M> job 全 pass）
下一步：codex 二审 / merge / archive change / 其他（按 opsx-apply-cadence.md 发布尾段节拍）
```

## 实施细节

### 结果传递

bg watch 完成的通知里包含 exit 码；output file 含 gh 流式重绘后末尾的最终状态行。首选**一次性 tail**：

```bash
tail -30 <output-file-path-from-notification>
```

回退路径下，把每轮 ScheduleWakeup poll 的状态写进 turn 文本让用户看到推进，例如：

```
[wait-ci #6] 第 1 轮：7 job pending, 0 fail → 270s 后再看
[wait-ci #6] 第 2 轮：4 pass, 3 pending → 270s 后再看
[wait-ci #6] 第 3 轮：test (windows-latest) FAIL → 拉日志
```

### 边界

- 若 `gh` 未安装或未登录：报错"gh CLI not available"并退出
- 若 PR 已 merge / closed：报告并退出
- 用户手动中断（下一个 turn 是其他指令）：bg 命令 / ScheduleWakeup 都会被 harness 自动清理

## 相关

- 本项目 CI matrix：`.github/workflows/ci.yml`（fmt 单跑 Ubuntu；clippy / test 跨 ubuntu-latest / windows-latest / macos-14）
- `.github/workflows/perf.yml`（perf 基线 gate，PR + push to main 跑 `scripts/run-perf-bench.sh`——可能因 baseline schema 误改而 fail）
- `scripts/check-openspec-archives.sh`（CI-only，本地 `just preflight` 不拦——opsx change 漏 archive 会在这里挂）
- 常见 fail 类型：clippy pedantic 违规 / test 跨平台 flake / Windows 路径陷阱（见 windows-compat-reviewer subagent）/ openspec archive 漏勾 / svelte-check / vitest contract test
- Release workflow 也可用本 skill 监控：`gh run watch <run-id>`（但那是单 workflow 不是 PR 关联，节奏更直接）
