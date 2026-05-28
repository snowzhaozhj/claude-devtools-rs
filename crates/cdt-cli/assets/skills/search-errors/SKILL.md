---
name: search-errors
description: Search across all sessions for specific error messages, tool failures, or keywords. Use when the user says "find sessions with", "search for error", "grep sessions", "which session had", or "find where".
---

# Search Errors

Full-text search across session content to locate specific errors or patterns.

## Steps

1. Search across all sessions:

```bash
cdt search "<query>" --limit 20
```

2. For each relevant hit, get context:

```bash
cdt sessions detail <session-id> --filter errors_only
```

3. If the user wants tool-specific failures:

```bash
cdt sessions detail <session-id> --filter tool_calls | grep -i "error\|fail\|denied"
```

## Usage Examples

- `cdt search "permission denied"` — find permission issues
- `cdt search "rate limit"` — find rate limiting events
- `cdt search "ENOENT"` — find missing file errors
- `cdt search "hook failed"` — find hook failures

## Output Format

Present matches grouped by session, with:
- Session ID, project, timestamp
- Matched text snippet with surrounding context
- Suggestion for resolution if pattern is recognizable
