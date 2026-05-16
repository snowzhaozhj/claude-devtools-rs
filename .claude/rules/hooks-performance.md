# Hook 性能规则（硬约束）

Claude Code 的每个工具调用都串行跑所有匹配的 hook。Bash 工具最高频，PreToolUse Bash hook 慢一点累计影响巨大。本文是**硬约束**——新加 / 改 hook 时按本文评估。

## 性能预算

| 场景 | 单 hook 预算 | 多 hook 总预算 |
|---|---|---|
| 命中预判（99% 调用，应直接 exit 0）| < 60 ms | < 250 ms |
| 真业务（1% 调用，跑 git / openspec / 解析等）| < 300 ms | < 800 ms |

PreToolUse Bash 链的总开销直接拖累每个 Bash 工具调用的 wall time。如果总和 > 500 ms，用户会感觉"Claude 慢"——而真实瓶颈在 hook 而非 LLM 推理。

**物理下限**（macOS bash 3.2.57）：
- bash 进程启动：~28 ms（不可控）
- `set -euo pipefail`：~3 ms
- `input=$(</dev/stdin)`：~25 ms（**SHALL** 不用 `$(cat)`——后者起 cat 子进程 +25ms）
- case 模式：0 ms
- 合计 cold path 极限 ~56 ms / 调用 / hook

进一步降低要么 (a) 用 bash 5.x（macOS 默认 3.2，需 `brew install bash` 改 shebang，跨开发机不便），要么 (b) 合并同 matcher 多 hook 成 1 个（见下方 followup）。

**回归阈值**：单 hook 99% 路径 > 60 ms / 1% 路径 > 300 ms 即拒（除非有强业务理由）。

## 硬约束（违反即拒）

### 1. matcher 已 gate 的字段不要再判

`settings.json` 的 `matcher` 已限制 `tool_name`。hook 内 `if [[ "$tool_name" != "Bash" ]]; then exit 0; fi` 这种检查是**冗余的 60ms python3**——matcher 已经过滤了。

```bash
# ❌ 反模式
tool_name=$(... python3 -c "json.load(sys.stdin).get('tool_name')")
if [[ "$tool_name" != "Bash" ]]; then exit 0; fi

# ✓ 正确：matcher 已 gate，直接进业务
input=$(cat)
case "$input" in *'"command"'*'git commit'*) ;; *) exit 0 ;; esac
```

### 2. 99% 调用必须 case 模式快速预判

PreToolUse Bash 在每个 Bash 工具调用都跑——但 99% 命令不是关键命令（git commit / push / 等）。SHALL 在 `cat` 后第一时间用 bash `case` 模式预判，不匹配立即 `exit 0`。`case` 是 bash 内置 0 fork。

```bash
# ✓ 正确（5ms 可放行 99% 调用）
input=$(cat)
case "$input" in
  *'"command"'*'git push'*) ;;  # 真感兴趣的模式
  *) exit 0 ;;
esac
```

预判可以粗（`grep -F` 或 `case`）—— corner case（命令文本里出现 `git commit` 但不是真 git commit）由后续严谨解析二次过滤。

### 3. JSON 解析用 jq，不用 python3

实测（macOS arm64）：

| 工具 | 启动 + 解析 JSON 耗时 |
|---|---|
| `python3 -c "import json,sys; ..."` | ~60 ms |
| `jq -r '.field'` | ~25 ms |
| `sed -nE 's/.../\1/p'` | ~5 ms（不严谨）|

**jq 是首选**（macOS / Linux / WSL 都常驻）。fallback 链：

```bash
field=$(printf '%s' "$input" | jq -r '.tool_input.command // ""' 2>/dev/null \
  || printf '%s' "$input" | sed -nE 's/.*"command"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/p' | head -1)
```

`python3` 仅在极特殊场景（jq 都拒不掉的复杂结构）作最后兜底。

### 4. 单次 jq 提取多字段，不要分多次

如果同一 hook 需要 tool_name + command + file_path 三个字段，**一次 jq 调用提取**——不要跑 3 次（省 50 ms × 2）。

```bash
# ✓ 正确：单次 jq 多输出
read -r cmd file_path <<< "$(printf '%s' "$input" | jq -r '[.tool_input.command, .tool_input.file_path] | @tsv' 2>/dev/null)"
```

### 5. 重命令（git / openspec / cargo）只在最后一步跑

`git branch` ~20ms / `git diff --cached` ~30ms / `openspec list --json` ~150ms / `cargo` 启动 200ms+。SHALL 在所有快速预判 + 字段解析过完才调，且只在确认要跑业务时调。

## 正确模板

```bash
#!/usr/bin/env bash
# PreToolUse Bash hook: <一句话目的>
#
# 性能预算：99% 路径 < 10ms；1% 路径 < 100ms
set -euo pipefail

input=$(cat)

# 1) case 预判（0 fork，~1ms）
case "$input" in
  *'"command"'*'git commit'*) ;;  # 关心的模式
  *) exit 0 ;;
esac

# 2) jq 严谨提取（~25ms，仅 1% 路径走到）
command=$(printf '%s' "$input" | jq -r '.tool_input.command // ""' 2>/dev/null \
  || printf '%s' "$input" | sed -nE 's/.*"command"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/p' | head -1)

if [[ "$command" != git\ commit* ]]; then
  exit 0  # 二次过滤 false positive
fi

# 3) 真业务（git / openspec / cargo）
... 此时已确认要跑，开销可接受 ...
```

## 防回归：hook bench

跑 `just bench-hooks` 列出所有 hook 的单次模拟耗时，对比预算。新加 hook 的 PR SHALL 跑一遍贴在 PR 描述里。

实现见 `justfile::bench-hooks` recipe + `scripts/bench-hooks.sh`。

## Followup：合并同 matcher 的多 hook

当前每个 matcher 下有多个 hook 文件 → 每个工具调用串行跑 N 次 bash 启动 = N × 56 ms。

| Matcher | hook 数 | cold path 总开销 |
|---|---|---|
| Bash (PreToolUse) | 2 | ~120 ms |
| Edit\|Write (PostToolUse) | 5 | ~280 ms |

合并成单 hook 内部 case 分流可省 (N-1) × 56 ms。trade-off：单文件可读性下降。考虑因素：
- 同 matcher 下逻辑高度独立 → 保持拆分（当前选择）
- 同 matcher 下都做"file_path 后缀分类"路由 → 适合合并

PostToolUse 5 个 hook 都是"按 file_path 后缀路由"模式，合并 ROI 高（省 224 ms/Edit）。延后实施，作为独立 PR。

## 历史事件（教训）

- **2026-05-16** bash-hook-perf — Bash 工具调用慢。复盘三层根因：
  1. 8 个 hook 各跑 2 次 python3 解析 JSON（python3 启动 ~60ms × 2 = 120ms / hook）
  2. PreToolUse hook 重复判断 matcher 已 gate 的 `tool_name` 字段（多 1 次 python3）
  3. `input=$(cat)` 起 cat 子进程（25ms）vs `$(</dev/stdin)` bash 内置（~0ms）

  优化：删冗余 tool_name 检查 + 加 case 预判 + python3→jq + `$(cat)`→`$(</dev/stdin)` + deny-edit-on-main 直读 `.git/HEAD` 替代 `git branch`（省 80ms）。

  Cold path 总开销 **817ms → 503ms（-38%）**，配合 .zshrc CLAUDE_* guard 把 Bash 工具调用整链 wall time 从 ~1.7s 降到 ~0.6s（-65%）。物理下限 ~56ms/hook 见上方分析。
