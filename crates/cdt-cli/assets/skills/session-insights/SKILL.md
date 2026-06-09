---
name: session-insights
description: "Analyze Claude Code sessions — errors, token usage, costs, search, diagnostics, recall."
---

# Session Insights

Progressive data loading — go deeper only when needed.

## Rules

1. `cdt session <id>` returns summary + cost + errors in one call — **no need for separate commands**
2. `--content omit` before `--content full` — browse structure first, then fetch precisely
3. `--range M:N` is **[M, N)** left-inclusive right-exclusive — `5:6` for chunk at index 5; `5:` for open-ended
4. Default **tail=20** without `--range`/`--tail`/`--all`
5. `--grep` auto-expands matched chunks to full content — use `--grep-context 0` to limit
6. Empty pipe output → print `keys()` to discover structure, don't guess field names
7. `latest` works as session ID — resolves to most recent session

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

## Step 2: Overview (single call)

```bash
cdt session <id>                          # summary + cost + errors (composite view)
cdt session <id> --include phases,tools   # add phases and tool usage facets
cdt session latest                        # most recent session
```

## Step 3: Structure browse

```bash
cdt session <id> --chunks --content omit                    # last 20 chunks, ~500B each
cdt session <id> --chunks --content omit --all              # full map (may be ~5MB)
cdt session <id> --chunks --content overview                # one-line per chunk summary
cdt session <id> --chunks --content omit --grep "<kw>"      # hits auto-expand
```

## Step 4: Precise fetch

```bash
cdt session <id> --chunks --content full --range 10:13      # chunks 10, 11, 12
```

## JSON Schema

Envelope: `{ sessionId, totalChunks, returnedChunks, contentMode, chunks: [ChunkView] }`

### ChunkView

| Field | Type | Notes |
|---|---|---|
| `chunkIndex` | int | absolute 0-based position |
| `chunkId` | string | unique identifier |
| `type` | `"ai"\|"user"\|"system"\|"compact"` | |
| `timestamp` | ISO 8601 | |
| `durationMs` | int? | AI chunks only |
| `toolExecutions` | ToolExecView[] | AI chunks — **errors live here** |
| `responses` | ResponseView[] | AI chunks — model text only, NO tool info |
| `userContent` | ContentField? | user chunks |
| `systemContent` | ContentField? | system chunks |
| `compactSummary` | string? | compact chunks |
| `grepHit` | bool? | only present when `--grep` active |

### ToolExecView — errors are here, NOT in responses

| Field | omit mode | full mode |
|---|---|---|
| `toolName` | ✓ | ✓ |
| `toolUseId` | ✓ | ✓ |
| `isError` | ✓ | ✓ |
| `errorMessage` | ✓ (when error) | ✓ (when error) |
| `inputSummary` | ✓ (abbreviated) | — |
| `input` | — | ✓ (full JSON) |
| `output` | — | ✓ (string \| object \| null) |
| `outputOmitted` | ✓ | ✓ |
| `outputChars` | ✓ | ✓ |

### ResponseView (model text content, NO tool info)

| Field | omit mode | full mode |
|---|---|---|
| `model` | ✓ | ✓ |
| `content` | absent (key not present) | ✓ (full text) |
| `contentOmitted` | ✓ (true) | ✓ (if upstream-trimmed) |
| `contentChars` | ✓ | ✓ |

### ContentField

`{ text: string|null, omitted: bool, chars: int }`

### Overview mode (--content overview)

Each chunk returns: `{ chunkIndex, kind, timestamp, toolNames: [], errorCount, headline }`

## Patterns

**Extract errors (flat, one per line):**
```bash
cdt session <id> --chunks --extract errors --all
# JSON: cdt session <id> --chunks --extract errors --all --format json
```

**Structure overview (one line per chunk):**
```bash
cdt session <id> --chunks --extract overview --all
# JSON: cdt session <id> --chunks --extract overview --all --format json
```

**All tool executions (flat list):**
```bash
cdt session <id> --chunks --extract tools --all
# Only tools from error chunks: --extract tools --filter errors_only --all
```

**Single chunk:** `--range 5:6` (remember: [M, N) so N=M+1 for one chunk)

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

## Scenarios

| Goal | Sequence |
|---|---|
| **Daily summary** | `sessions list --since 2026-06-08 --until 2026-06-09 --group-by project --json=projectName,sessionId,title,userIntents,durationMs,totalCost,filesTouched,gitSummary` — one command, all data for a daily report. Use `--since/--until YYYY-MM-DD` for calendar days, NOT `--since 1d` (rolling 24h window). |
| Errors | `session <id>` (errors included) → `--chunks --extract errors --all` for detail → `--chunks --content full --range` for full context |
| Overview | `--chunks --extract overview --all` or `--chunks --content overview` |
| Cost | `session <id>` (cost included) |
| Search | `search "<q>"` → `session <id> --chunks --grep "<q>"` |
| Tools | `--chunks --extract tools --all` → filter by chunkIndex |
| Diagnostics | `session <id>` → `--chunks --extract overview --tail 20` → `--chunks --range` |
| Stats | `stats 7d` or `stats 7d --group-by model` |

## Flags

| Flag | Command | Effect |
|---|---|---|
| `--content omit\|overview\|full` | `session --chunks` | structure-only / one-line / full content |
| `--grep <kw>` | `session --chunks` | filter chunks; matched auto-expand to full |
| `--grep-context N` | `session --chunks` | context chunks around hits (default 1) |
| `--filter errors_only\|tool_calls` | `session --chunks` | chunk type filter |
| `--all` | `session --chunks` | disable default tail=20 |
| `--range M:N` | `session --chunks` | `[M,N)` by chunkIndex; `M:` open-ended |
| `--tail N` | `session --chunks` | last N chunks (exclusive with --range) |
| `--extract overview\|errors\|tools` | `session --chunks` | flat item-level output (conflicts with --content) |
| `--include phases,tools,...` | `session` | add facets to composite view |
| `--since / --until` | `sessions list`, `search` | time range filter |
| `--branch` | `sessions list` | git branch filter |
| `--group-by` | `sessions list`, `stats` | group results by dimension |
| `--json=f1,f2` | all | field projection + compact output |
