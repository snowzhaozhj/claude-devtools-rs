---
name: session-insights
description: "Analyze Claude Code sessions — errors, token usage, costs, search, diagnostics, recall."
---

# Session Insights

Load session data progressively — only go deeper when the previous step isn't enough.

## Constraints

1. **Step 2 commands are independent** — call `summary`, `cost`, `errors` in parallel (single tool-call batch)
2. **Step 3 MUST precede Step 4** for unknown sessions — never fetch `--content full` without first browsing structure via `--content omit`
3. **`--grep` auto-expands matched chunks to full** — when grepping common terms, expect large responses; use `--grep-context 0` to limit expansion
4. **If a pipe/jq/python filter returns no output**: next call MUST print the raw JSON keys (`jq 'keys'` or `python3 -c "... print(list(data.keys()))"`) to discover structure — do NOT guess field names repeatedly
5. **Range is `[start, end)` left-inclusive, right-exclusive** — `--range 5:5` returns nothing; use `--range 5:6` for chunk at index 5
6. **Default tail=20**: without `--range`, `--tail`, or `--all`, only the last 20 chunks are returned

## Step 1: Discover

```bash
cdt projects list --format json
cdt --json=sessionId,title,messageCount,isOngoing sessions list --project <name> --since 7d
```

## Step 2: Overview (parallel — no dependencies between these)

```bash
cdt sessions summary <id>    # → phases, tool stats, errors, cost, toolActivity
cdt sessions cost <id>       # → token breakdown by category + model pricing
cdt sessions errors <id>     # → chunkIndex + toolName + errorMessage for each error
```

## Step 3: Structure browse

```bash
cdt sessions detail <id> --format json --content omit --all
# → full chunk structure: ~500B/chunk (vs ~200KB full). Shows chunkIndex, type, toolExecutions summary
# With grep (matched chunks auto-expand to full; others stay omit):
cdt sessions detail <id> --format json --content omit --grep "<keyword>"
```

## Step 4: Precise fetch

```bash
cdt sessions detail <id> --format json --content full --range <start>:<end>
# Range is [start, end) by absolute chunkIndex. Example: --range 10:13 returns chunks 10, 11, 12
# Open-ended: --range 10: returns from chunk 10 to the end
```

## JSON schema reference

### Envelope (sessions detail --format json)

```
{ sessionId, totalChunks, returnedChunks, contentMode, chunks: [ChunkView...] }
```

### ChunkView fields

| Field | Type | When present |
|---|---|---|
| `chunkIndex` | number | Always (absolute 0-based position) |
| `chunkId` | string | Always |
| `type` | `"ai" \| "user" \| "system" \| "compact"` | Always |
| `timestamp` | ISO 8601 | Always |
| `durationMs` | number? | AI chunks |
| `toolExecutions` | ToolExecView[] | AI chunks — **this is where errors live** |
| `responses` | ResponseView[] | AI chunks — model text content only |
| `userContent` | ContentField? | User chunks |
| `systemContent` | ContentField? | System chunks |
| `compactSummary` | string? | Compact (compaction) chunks |
| `grepHit` | boolean? | Only when `--grep` active |

### ToolExecView fields (errors are here, NOT in responses)

| Field | omit mode | full mode |
|---|---|---|
| `toolName` | ✓ | ✓ |
| `toolUseId` | ✓ | ✓ |
| `isError` | ✓ | ✓ |
| `errorMessage` | ✓ (if error) | ✓ (if error) |
| `inputSummary` | ✓ (abbreviated) | — |
| `input` | — | ✓ (full JSON) |
| `output` | — | ✓ (full text) |
| `outputOmitted` | ✓ | ✓ (if upstream-trimmed) |
| `outputChars` | ✓ | ✓ |

### ResponseView fields (model output text, NO tool info)

| Field | omit mode | full mode |
|---|---|---|
| `model` | ✓ | ✓ |
| `content` | — (null) | ✓ |
| `contentOmitted` | ✓ (true) | ✓ (if upstream-trimmed) |
| `contentChars` | ✓ | ✓ |

### ContentField (user/system content)

```
{ text: string|null, omitted: boolean, chars: number }
```

## Common patterns

### Extract errors in one shot

```bash
cdt sessions detail <id> --format json --content full --filter errors_only | \
  python3 -c "
import json, sys
data = json.load(sys.stdin)
for chunk in data['chunks']:
    for te in chunk.get('toolExecutions', []):
        if te.get('isError'):
            print(f\"[Chunk {chunk['chunkIndex']}] {te['toolName']}: {te.get('errorMessage', 'no message')}\")
            print(f\"  Input: {str(te.get('input',''))[:200]}\")
            print()
"
```

### Browse full structure for phase-level understanding

```bash
cdt sessions detail <id> --format json --content omit --all | \
  python3 -c "
import json, sys
data = json.load(sys.stdin)
for c in data['chunks']:
    tools = len(c.get('toolExecutions', []))
    errs = sum(1 for t in c.get('toolExecutions', []) if t.get('isError'))
    err_flag = f' ⚠️{errs}err' if errs else ''
    typ = c['type'][:4]
    print(f\"[{c['chunkIndex']:3d}] {c['timestamp'][:19]} {typ} tools={tools}{err_flag}\")
"
```

### Fetch a single chunk by index

```bash
# Remember: range is [start, end), so use N:N+1 for single chunk
cdt sessions detail <id> --format json --content full --range 5:6
```

## Scenario quick reference

| Scenario | Command sequence |
|---|---|
| Error analysis | `summary` + `errors` (parallel) → `detail --content omit --filter errors_only` → `--content full --range` by chunkIndex |
| Cost | `cost <id>` (or `stats 7d` for aggregate) |
| Search | `search "<query>"` → `detail --content omit --grep "<query>"` |
| Diagnostics | `summary` + `errors` (parallel) → `detail --content omit --tail 20` → precise fetch |
| Recall | `summary` (check toolActivity) → `detail --content omit --grep "<action>"` |

## Flag quick reference

| Flag | Effect |
|---|---|
| `--json=f1,f2` | Implies `--format json` + field projection + compact output; `--json` alone lists available fields |
| `--content omit\|full` | Content granularity for `sessions detail` JSON/JSONL |
| `--grep <kw>` | Chunk content filter; matched chunks auto-expand to full content |
| `--grep-context N` | Number of surrounding chunks to include around grep hits (default 1) |
| `--filter errors_only\|tool_calls` | Chunk type filter |
| `--all` (alias `--full`) | Disable default tail=20; return all chunks |
| `--range M:N` | Window by chunkIndex `[M, N)`; `M:` = from M to end |
| `--tail N` | Last N chunks (mutually exclusive with --range) |
| `--since 7d\|24h\|30d` | Time range for list commands |
