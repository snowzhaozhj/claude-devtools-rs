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

2. For a more detailed per-session breakdown:

```bash
cdt sessions list --since 7d --format json | jq '.[] | {id: .sessionId, title: .title, input: .inputTokens, output: .outputTokens, total: (.inputTokens + .outputTokens)}' | head -50
```

3. Optionally drill into a specific session's cost:

```bash
cdt sessions cost <session-id>
```

## Output Format

Present:
- Total tokens (input/output split) for the period
- Estimated cost (using $3/MTok input, $15/MTok output for Opus; $0.80/$4 for Sonnet)
- Top 5 most expensive sessions by token count
- Daily trend if data spans multiple days

## Notes

- Token counts come from Claude Code's logged usage data
- Cost estimates are approximate based on public API pricing
- Use `--project <name>` with `cdt stats` to filter by project
