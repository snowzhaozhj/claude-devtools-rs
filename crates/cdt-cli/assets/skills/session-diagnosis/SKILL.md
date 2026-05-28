---
name: session-diagnosis
description: Generate a diagnostic summary of a specific Claude Code session — token usage, tool calls, errors, duration, and outcome. Use when the user says "diagnose session", "what happened in session", "session summary", "session report", or "explain this session".
---

# Session Diagnosis

Produces a comprehensive diagnostic report for a single session.

## Steps

1. Get session metadata:

```bash
cdt sessions show <session-id> --format json
```

2. Get session summary (structured diagnostic overview):

```bash
cdt sessions summary <session-id>
```

3. Get cost breakdown:

```bash
cdt sessions cost <session-id>
```

4. Check for errors:

```bash
cdt sessions errors <session-id>
```

5. Get chunk-level detail for the last portion:

```bash
cdt sessions detail <session-id> --tail 20
```

## Output Format

Present a structured report:

### Session Overview
- **ID**: (short form)
- **Title**: session title
- **Duration**: start → end (elapsed)
- **Status**: completed / ongoing
- **Messages**: total message count

### Resource Usage
- Input tokens / Output tokens / Total
- Estimated cost

### Tool Activity
- Tools used (from detail --filter tool_calls)
- Failed tool calls (if any)

### Errors
- Error count and types (from sessions errors)
- Critical errors highlighted

### Outcome
- Summary of what was accomplished
- Whether the session ended successfully

## Notes

- If no session ID is provided, use `cdt sessions list --project <name> --since 1d` to help find recent sessions
- Use `cdt projects list` to discover available projects
- The session ID can be found in the output of `sessions list` (sessionId field in JSON format)
