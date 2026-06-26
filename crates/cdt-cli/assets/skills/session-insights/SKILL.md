---
name: session-insights
description: "Analyze Claude Code sessions — turn-level drill-down, costs, search, diagnostics."
---

# Session Insights

Three-layer progressive drill-down: session → turn → tool output.

## Rules

1. `cdt session <id>` returns compact turn overview — one call shows all turns with question/answer/tools/metrics
2. `cdt turn <id> <N>` drills into turn N's steps (thinking, tool calls, text)
3. `cdt tool-output <id> <toolUseId>` fetches full untruncated tool output
4. `--page-size` + `--cursor` for pagination (default 20 turns, 50 steps)
5. `--grep` filters turns and adds `matchedIn` attribution (tool:\<name\> > error > thinking > answer > question)
6. `latest` works as session ID — resolves to most recent session
7. `--raw` on `cdt session` falls back to the old composite view (escape hatch)

## Step 1: Discover

```bash
cdt projects list --format json
cdt sessions list --since 7d --json=sessionId,title,messageCount,isOngoing
# With project filter:
cdt sessions list --project <name> --since 7d
# With branch filter:
cdt sessions list --branch feat/auth
# With grouping:
cdt sessions list --since 7d --group-by project
```

## Step 2: Session overview (turn list)

```bash
cdt session <id>                          # all turns with question/answer/tools/metrics
cdt session latest                        # most recent session
cdt session <id> --grep "error"           # only turns matching "error", with matchedIn
cdt session <id> --page-size 5            # first 5 turns
cdt session <id> --page-size 5 --cursor 5 # next 5 turns
```

## Step 3: Turn detail (steps)

```bash
cdt turn <id> 0                           # all steps in turn 0
cdt turn <id> 0 --page-size 10            # first 10 steps
cdt turn <id> 0 --page-size 10 --cursor 10 # next 10 steps
```

## Step 4: Full tool output

```bash
cdt tool-output <id> <toolUseId>          # full untruncated output
```

## JSON Schemas

### `cdt session` response

```json
{
  "sessionId": "abc-123",
  "model": "claude-opus-4-6",
  "totalCost": 12.34,
  "durationMs": 300000,
  "filesModified": ["/src/main.rs"],
  "total": 5,
  "nextCursor": "3",
  "turns": [
    {
      "index": 0,
      "question": "Fix the login bug",
      "answer": "I've fixed the auth...",
      "tools": [{"name": "Edit", "count": 3, "errorCount": 0}],
      "stepCounts": {"tool": 5, "text": 3, "thinking": 2},
      "metrics": {
        "inputTokens": 1000, "outputTokens": 500,
        "cacheReadTokens": 8000, "cacheCreationTokens": 200,
        "cost": 0.05, "durationMs": 30000, "model": "claude-opus-4-6"
      },
      "matchedIn": "tool:Edit"
    }
  ]
}
```

### `cdt turn` response

```json
{
  "sessionId": "abc-123",
  "turnIndex": 0,
  "question": "Fix the login bug",
  "answer": "I've fixed the auth...",
  "stepsTotal": 10,
  "nextCursor": "5",
  "metrics": { "..." },
  "steps": [
    {"type": "thinking", "index": 0, "text": "Let me look at..."},
    {"type": "tool", "index": 1, "toolUseId": "tu_1", "name": "Read",
     "input": {"file_path": "/src/auth.rs"},
     "output": {"kind": "text", "text": "..."},
     "isError": false, "outputTruncated": true, "outputBytes": 15000},
    {"type": "text", "index": 2, "text": "The issue is in..."}
  ]
}
```

Step types: `thinking`, `text`, `tool`, `subagent`, `teammate_spawn`, `workflow`, `interruption`, `user_message`, `slash`, `teammate_message`, `compaction`, `system`.

### `cdt tool-output` response

```json
{
  "sessionId": "abc-123",
  "toolUseId": "tu_1",
  "toolName": "Read",
  "outputBytes": 15000,
  "output": {"kind": "text", "text": "...full content..."}
}
```

## Patterns

**Find errors in a session:**
```bash
cdt session <id> --grep "error"
# Then drill into the matching turn:
cdt turn <id> <turnIndex>
```

**Aggregated stats:**
```bash
cdt stats 7d                              # last 7 days across all projects
cdt stats 30d --project my-app            # single project
cdt stats 7d --group-by model             # grouped by model
```

**Cross-project search:**
```bash
cdt search "deploy error"                 # all projects
cdt search "deploy" --since 7d            # time-scoped
cdt search "keyword" --session <id>       # intra-session search
```

**Export session:**
```bash
cdt export <id>                           # markdown to stdout
cdt export <id> --export-format json -o out.json
cdt export <id> --detail summary          # abbreviated tool outputs
```

## Scenarios

| Goal | Sequence |
|---|---|
| **Daily summary** | `sessions list --since 2026-06-08 --until 2026-06-09 --group-by project --json=projectName,sessionId,title,durationMs,totalCost,filesModified,gitSummary` |
| **Errors** | `session <id>` → find turn with errors in `tools[].errorCount` → `turn <id> <N>` → look for `isError: true` steps |
| **Cost** | `session <id>` → `totalCost` + per-turn `metrics.cost` |
| **Search** | `search "<q>"` → `session <id> --grep "<q>"` → `turn <id> <N>` |
| **Tool output** | `turn <id> <N>` → find `outputTruncated: true` → `tool-output <id> <toolUseId>` |
| **Diagnostics** | `session <id>` → scan `stepCounts` + `tools` per turn → `turn <id> <N>` for detail |
| **Stats** | `stats 7d` or `stats 7d --group-by model` |

## Flags

| Flag | Command | Effect |
|---|---|---|
| `--grep <kw>` | `session` | filter turns; adds `matchedIn` attribution |
| `--page-size N` | `session`, `turn` | items per page (default 20/50, max 100) |
| `--cursor <c>` | `session`, `turn` | pagination cursor from `nextCursor` |
| `--raw` | `session` | fall back to old composite view |
| `--since / --until` | `sessions list`, `search` | time range filter |
| `--branch` | `sessions list` | git branch filter |
| `--group-by` | `sessions list`, `stats` | group results by dimension |
| `--json=f1,f2` | all | field projection + compact output |
