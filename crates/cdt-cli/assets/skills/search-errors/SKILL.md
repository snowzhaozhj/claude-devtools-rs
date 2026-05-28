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

2. To narrow by project:

```bash
cdt search "<query>" --project <project-name> --limit 20
```

3. For each relevant hit, get error context:

```bash
cdt sessions errors <session-id>
```

4. For detailed tool-call level inspection:

```bash
cdt sessions detail <session-id> --filter tool_calls
```

## Usage Examples

- `cdt search "permission denied"` — find permission issues
- `cdt search "rate limit"` — find rate limiting events
- `cdt search "ENOENT"` — find missing file errors
- `cdt search "hook failed"` — find hook failures
- `cdt search "panic" --project my-project` — find panics in a specific project

## Output Format

Present matches grouped by session, with:
- Session ID, project, timestamp
- Matched text snippet with surrounding context
- Suggestion for resolution if pattern is recognizable

## Notes

- Use `cdt projects list` to find available project names
- Search covers all message content within sessions
- Combine with `cdt sessions detail <id> --filter errors_only` for error-focused view
