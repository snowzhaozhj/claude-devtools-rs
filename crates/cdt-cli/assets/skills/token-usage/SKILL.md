---
name: token-usage
description: Report token consumption and cost across recent Claude Code sessions. Use when the user says "how much did I spend", "token usage", "cost report", "billing", "usage stats", or "how many tokens".
---

# Token Usage

Aggregates token usage and estimated cost across sessions.

## Steps

1. Get overall stats for the time period:

```bash
cdt stats 7d
```

2. For project-specific stats:

```bash
cdt stats 7d --project <project-name>
```

3. For a specific session's cost breakdown:

```bash
cdt sessions cost <session-id>
```

4. To find the most active sessions to drill into:

```bash
cdt sessions list --project <project-name> --since 7d --format json
```

Then check cost for sessions with high message counts.

## Output Format

Present:
- Total tokens (input/output split) for the period
- Estimated cost breakdown
- Top 5 most expensive sessions by token count
- Daily trend if data spans multiple days

## Notes

- Use `cdt projects list` to find available project names
- `cdt stats` without `--project` aggregates across all projects
- `cdt sessions cost <id>` gives per-session token details
- Cost estimates use public API pricing (may differ from actual billing)
