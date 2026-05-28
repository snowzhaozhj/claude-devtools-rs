---
name: analyze-failures
description: Analyze recent failed or error-heavy Claude Code sessions to identify patterns and root causes. Use when the user says "what went wrong", "why did it fail", "show me errors", "analyze failures", "debug session", or "failed sessions".
---

# Analyze Failures

Finds recent sessions with errors and surfaces failure patterns.

## Steps

1. List recent sessions for the current project:

```bash
cdt sessions list --project <project-name> --since 7d --format json
```

2. For each session, check for errors:

```bash
cdt sessions errors <session-id>
```

3. For detailed error context within a session:

```bash
cdt sessions detail <session-id> --filter errors_only
```

4. Summarize:
   - Which tools failed most often
   - Common error messages or patterns
   - Whether failures cluster in time or by session
   - Suggested actions (retry, fix config, report bug)

## Output Format

Present a table of sessions with errors: session ID (short), title, time, error summary. Then a "Patterns" section grouping by root cause.

## Notes

- Use `cdt projects list` to find available project names
- The `--since` flag accepts: 7d, 24h, 30d, today, week
- Session IDs from `sessions list` can be passed directly to `sessions errors`
