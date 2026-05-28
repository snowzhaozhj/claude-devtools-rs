---
name: session-insights
description: Analyze Claude Code sessions — find errors, check token usage, search content, or diagnose a specific session. Use when the user asks about session failures, costs, token consumption, or wants to understand what happened in a session.
---

# Session Insights

Provides session analysis workflows using the `cdt` CLI. Choose the appropriate workflow based on what the user needs.

## Workflow Selection

| User intent | Workflow |
|---|---|
| "what went wrong" / "show me errors" / "failed sessions" | Error Analysis |
| "how much did I spend" / "token usage" / "cost" | Token & Cost |
| "find sessions with X" / "search for error" | Search |
| "what happened in session X" / "diagnose" / "session report" | Single Session Diagnosis |

## Error Analysis

Find sessions with errors and identify patterns.

```bash
# List recent sessions for a project
cdt sessions list --project <project-name> --since 7d

# Get errors for a specific session
cdt sessions errors <session-id>

# Error-focused detail view
cdt sessions detail <session-id> --filter errors_only
```

Summarize: which tools failed, common error messages, time clustering, suggested actions.

## Token & Cost

Aggregate token usage and estimated cost.

```bash
# Overall stats for a time period
cdt stats 7d

# Project-specific stats
cdt stats 7d --project <project-name>

# Per-session cost breakdown
cdt sessions cost <session-id>
```

Present: total tokens (input/output), estimated cost, top sessions by usage.

## Search

Full-text search across session content.

```bash
# Search all sessions
cdt search "<query>" --limit 20

# Search within a project
cdt search "<query>" --project <project-name> --limit 20
```

Examples: `cdt search "permission denied"`, `cdt search "rate limit"`, `cdt search "ENOENT"`.

## Single Session Diagnosis

Comprehensive report for one session.

```bash
# Metadata
cdt sessions show <session-id> --format json

# Structured summary
cdt sessions summary <session-id>

# Cost
cdt sessions cost <session-id>

# Errors
cdt sessions errors <session-id>

# Recent chunks
cdt sessions detail <session-id> --tail 20
```

Present: overview (title, duration, status, messages), resource usage, tool activity, errors, outcome.

## Common Notes

- Use `cdt projects list` to discover available project names
- `--since` accepts: 7d, 24h, 30d, today, week
- `--format json` available on most commands for structured output
- Session IDs come from `cdt sessions list` output
