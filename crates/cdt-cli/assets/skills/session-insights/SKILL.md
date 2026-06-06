---
name: session-insights
description: "Analyze Claude Code sessions — errors, token usage, costs, search, diagnostics, recall."
---

# Session Insights

Load session data progressively — only go deeper when the previous step isn't enough.

## Step 1: Discover

```bash
cdt projects list --format json
cdt --json=sessionId,title,messageCount,isOngoing sessions list --project <name> --since 7d
```

## Step 2: Overview

```bash
cdt sessions summary <id>
# → phases, tool stats, errors, cost, toolActivity (~2K tokens)
```

## Step 3: Structure browse

```bash
cdt sessions detail <id> --format json --content omit
# → chunk structure overview: ~500B/chunk (vs ~200KB full)
# With grep, matched chunks auto-expand to full; others stay omit:
cdt sessions detail <id> --format json --content omit --grep "<keyword>"
```

## Step 4: Precise fetch

```bash
cdt sessions detail <id> --format json --content full --range <start>:<end>
```

## Scenario quick reference

| Scenario | Command sequence |
|---|---|
| Error analysis | `sessions list` → `sessions errors <id>` → `sessions detail <id> --content omit --filter errors_only` → `--content full --range` by chunkIndex |
| Cost | `stats 7d` → `sessions cost <id>` |
| Search | `search "<query>"` → `sessions detail <id> --content omit --grep "<query>"` |
| Diagnostics | `sessions summary <id>` → `sessions errors <id>` → `sessions detail <id> --content omit --tail 20` |
| Recall | `sessions summary <id>` (check toolActivity) → `sessions detail <id> --content omit --grep "<action>"` |

## Flag quick reference

| Flag | Effect |
|---|---|
| `--json=f1,f2` | Implies `--format json` + field projection + compact output; `--json` alone lists available fields |
| `--content omit\|full` | Content granularity for `sessions detail` JSON/JSONL |
| `--grep <kw>` | Chunk content filter; matched chunks auto-expand to full |
| `--filter errors_only\|tool_calls` | Chunk type filter |
| `--all` (alias `--full`) | Disable default tail=20 |
| `--range M:N` / `--tail N` | Window selection (mutually exclusive) |
| `--since 7d\|24h\|30d` | Time range |
