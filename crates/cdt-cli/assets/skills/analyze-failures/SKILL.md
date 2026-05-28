---
name: analyze-failures
description: Analyze recent failed or error-heavy Claude Code sessions to identify patterns and root causes. Use when the user says "what went wrong", "why did it fail", "show me errors", "analyze failures", "debug session", or "failed sessions".
---

# Analyze Failures

Finds recent sessions with errors and surfaces failure patterns.

## Steps

1. List recent sessions and identify those with errors:

```bash
cdt sessions list --since 7d --format json | jq '[.[] | select(.hasErrors == true or .errorCount > 0)] | sort_by(.lastActivityAt) | reverse | .[:10]'
```

2. For each failed session, get error details:

```bash
cdt sessions errors <session-id>
```

3. Summarize:
   - Which tools failed most often
   - Common error messages or patterns
   - Whether failures cluster in time or project
   - Suggested actions (retry, fix config, report bug)

## Output Format

Present a table of failed sessions with columns: session ID (short), project, time, error count, primary error type. Then a "Patterns" section grouping by root cause.
