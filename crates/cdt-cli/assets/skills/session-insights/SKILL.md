---
name: session-insights
description: "Analyze Claude Code sessions via the `cdt` CLI. Use this skill whenever the user mentions sessions, errors, costs, token usage, debugging a session, understanding what happened, recalling past actions, or searching session content — even if they don't say 'session-insights' explicitly."
---

# Session Insights

渐进式加载 session 数据，每步只在上一步信息不够时才往下走。

## Step 1: 发现

```bash
cdt projects list --format json
cdt --json=sessionId,title,messageCount,isOngoing sessions list --project <name> --since 7d
```

## Step 2: 概览

```bash
cdt sessions summary <id>
# → phases, tool stats, errors, cost, toolActivity (~2K tokens)
```

## Step 3: 结构浏览

```bash
cdt sessions detail <id> --format json --content omit
# → chunk 结构概览：每个 chunk ~500B（vs 完整 ~200KB）
# 带 grep 时命中 chunk auto-expand 为 full，其余保持 omit：
cdt sessions detail <id> --format json --content omit --grep "<keyword>"
```

## Step 4: 精确拉取

```bash
cdt sessions detail <id> --format json --content full --range <start>:<end>
```

## 场景速查

| 场景 | 命令序列 |
|---|---|
| 错误分析 | `sessions list` → `sessions errors <id>` → `sessions detail <id> --content omit --filter errors_only` → 按 chunkIndex `--content full --range` |
| 费用 | `stats 7d` → `sessions cost <id>` |
| 搜索 | `search "<query>"` → `sessions detail <id> --content omit --grep "<query>"` |
| 诊断 | `sessions summary <id>` → `sessions errors <id>` → `sessions detail <id> --content omit --tail 20` |
| 回忆 | `sessions summary <id>`（看 toolActivity）→ `sessions detail <id> --content omit --grep "<action>"` |

## Flag 速查

| Flag | 作用 |
|---|---|
| `--json=f1,f2` | 隐含 `--format json` + 字段投影 + 紧凑输出；`--json` 无值列出可用字段 |
| `--content omit\|full` | `sessions detail` JSON/JSONL 内容粒度 |
| `--grep <kw>` | chunk 内容过滤，命中 chunk auto-expand 为 full |
| `--filter errors_only\|tool_calls` | chunk 类型过滤 |
| `--all` (alias `--full`) | 禁用默认 tail=20 |
| `--range M:N` / `--tail N` | 窗口选择（互斥） |
| `--since 7d\|24h\|30d` | 时间范围 |
