---
name: session-insights
description: "Analyze Claude Code sessions — errors, token usage, costs, search, diagnostics, recall."
---

# Session Insights

Progressive data loading — go deeper only when needed.

## Rules

1. `summary`, `cost`, `errors` are independent — **call in parallel**
2. `--content omit` before `--content full` — browse structure first, then fetch precisely
3. `--range M:N` is **[M, N)** left-inclusive right-exclusive — `5:6` for chunk at index 5; `5:` for open-ended
4. Default **tail=20** without `--range`/`--tail`/`--all`
5. `--grep` auto-expands matched chunks to full content — use `--grep-context 0` to limit
6. Empty pipe output → print `keys()` to discover structure, don't guess field names

## Step 1: Discover

```bash
cdt projects list --format json
cdt --json=sessionId,title,messageCount,isOngoing sessions list --project <name> --since 7d
```

## Step 2: Overview (parallel — no deps)

```bash
cdt sessions summary <id>    # phases, tool stats, cost, toolActivity
cdt sessions cost <id>       # token breakdown + model pricing
cdt sessions errors <id>     # chunkIndex + toolName + errorMessage per error
```

## Step 3: Structure browse

```bash
cdt sessions detail <id> --format json --content omit          # last 20 chunks, ~500B each
cdt sessions detail <id> --format json --content omit --all    # full map (may be ~5MB)
cdt sessions detail <id> --format json --content omit --grep "<kw>"  # hits auto-expand
```

## Step 4: Precise fetch

```bash
cdt sessions detail <id> --format json --content full --range 10:13  # chunks 10, 11, 12
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

## Patterns

**Extract errors (flat, one per line):**
```bash
cdt sessions detail <id> --extract errors --all
# JSON: cdt sessions detail <id> --extract errors --all --format json
```

**Structure overview (one line per chunk):**
```bash
cdt sessions detail <id> --extract overview --all
# JSON: cdt sessions detail <id> --extract overview --all --format json
```

**All tool executions (flat list):**
```bash
cdt sessions detail <id> --extract tools --all
# Only tools from error chunks: --extract tools --filter errors_only --all
```

**Single chunk:** `--range 5:6` (remember: [M, N) so N=M+1 for one chunk)

## Scenarios

| Goal | Sequence |
|---|---|
| Errors | `--extract errors --all` (flat list) → `--content full --range` for detail |
| Overview | `--extract overview --all` (one line per chunk) |
| Cost | `cost <id>` |
| Search | `search "<q>"` → `detail --grep "<q>"` |
| Tools | `--extract tools --all` → filter by chunkIndex |
| Diagnostics | `summary`+`errors` ∥ → `--extract overview --tail 20` → `--range` |

## Flags

| Flag | Effect |
|---|---|
| `--content omit\|full` | structure-only vs full content |
| `--grep <kw>` | filter chunks; matched auto-expand to full |
| `--grep-context N` | context chunks around hits (default 1) |
| `--filter errors_only\|tool_calls` | chunk type filter |
| `--all` | disable default tail=20 |
| `--range M:N` | `[M,N)` by chunkIndex; `M:` open-ended |
| `--tail N` | last N chunks (exclusive with --range) |
| `--extract overview\|errors\|tools` | flat item-level output (conflicts with --content) |
| `--json=f1,f2` | field projection + compact output |
