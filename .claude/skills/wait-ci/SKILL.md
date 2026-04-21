---
name: wait-ci
description: Poll `gh pr checks <pr>` 直到全绿或失败；失败时自动 `gh run view --log-failed` 过滤 FAILED/panicked/error 行提炼真正的错误。用户显式 `/wait-ci [pr]` 或"等 CI / 看 CI 跑没跑完 / CI 结果"时触发。也可 Claude 在刚 push 代码后自发调用。
---

# wait-ci

**触发**：
- 用户显式 `/wait-ci` 或 `/wait-ci 6`
- 自然语言 "等 CI / CI 过了没 / 看 CI 结果"
- Claude 刚 `git push` 后主动调用（可选；不打扰）

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

### 3. Poll 直到结束

方式：调用 ScheduleWakeup 自我定时（推荐 60-90s 间隔），每次 fire 重跑 `gh pr checks`。

**不要**在 Bash 里 `while sleep` —— 那会阻塞 session context。用 ScheduleWakeup 让 runtime 替你等。

典型节奏：
- 首轮 60s 后醒
- 若还 pending 再等 90s
- 最长 15 分钟（Tauri 构建 + 跨三平台矩阵，约 10-15 min）
- 超时则打印当前状态让用户决定

每次 wakeup：
```bash
gh pr checks "$pr" 2>&1 | head -20
```

找关键词：
- `fail\|FAIL` → 直接到 Step 4 不再等
- 无 `pending\|in_progress\|queued` → 全绿，到 Step 5

### 4. 失败时拉日志 + 提炼错误行

对每个 `fail` 的 job：
```bash
# 从 gh pr checks 输出解析出 job URL，末段是 run_id/job/job_id
gh run view <run_id> --log-failed --job <job_id> 2>&1 \
  | grep -iE "FAILED|panicked at|error\[|error:|assertion.*failed" \
  | head -30
```

过滤真正的错误行（跳过 `Downloaded / Compiling / Checking` 等正常噪音）。

输出给用户：
- 哪个 job 挂了（platform + step 名，如 `test (windows-latest) / cargo test --workspace`）
- 提炼的失败 test / panic 位置 / compile error
- 给出最有可能的下一步建议（但不自己改代码；让用户决定）

### 5. 全绿时报告

```
✅ PR #<N> CI 全绿（<M> job 全 pass）
下一步：merge / 真机验证 / 其他
```

## 实施细节

### ScheduleWakeup 用法

```
ScheduleWakeup({
  delaySeconds: 60,  // 或 90 / 300，见下
  reason: "poll PR #6 CI status",
  prompt: "/wait-ci 6"  // 原样把自己再触发一次
})
```

**delaySeconds 选择**：
- 首轮 60s（快看一眼）
- 若还 pending 继续 90s（长轮询）
- **不要**选 300s —— 会命中 Anthropic prompt cache 5 分钟 TTL 边界（CLAUDE.md 有说）；用 270s 或直接 600s

### 结果传递

把 poll 结果写进 turn 的 text，让用户看到节奏推进：

```
[wait-ci #6] 第 1 轮: 7 job pending, 0 fail → 60s 后再看
[wait-ci #6] 第 2 轮: 4 pass, 3 pending → 90s 后再看
[wait-ci #6] 第 3 轮: test (windows-latest) FAIL → 拉日志
```

### 边界

- 若 `gh` 未安装或未登录：报错"gh CLI not available"并退出
- 若 PR 已 merge / closed：报告并退出
- 用户手动中断（下一个 turn 是其他指令）：ScheduleWakeup 自动失效；不清理

## 相关

- 本项目 CI matrix：`.github/workflows/ci.yml`（fmt 单跑 Ubuntu；clippy / test 跨 ubuntu-latest / windows-latest / macos-14）
- 常见 fail 类型：clippy pedantic 违规 / test 跨平台 flake / Windows 路径陷阱（见 windows-compat-reviewer subagent）
- Release workflow 也可用本 skill 监控：`gh run watch <run-id>`（但那是单 workflow 不是 PR 关联）
