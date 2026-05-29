# Proposal: WorkflowCard 6-state rendering

## Problem

When a session contains workflow runs (multi-agent orchestration), the current UI dumps raw JSON or shows nothing. Users need structured visual feedback showing workflow status, phases, agents, and their outcomes.

## Solution

Add `WorkflowCard` component that renders workflow data in 6 distinct states:
1. **Done** — complete phase tree + agent chips with status/tokens/duration
2. **Partial failure** — header "N failed" tag + red chips for failed agents
3. **Running (minimal)** — header spinner + "details available after completion"
4. **Launch error** — rendered as error tool execution (no empty card)
5. **Empty** — header + "No subagents"
6. **Hover** — header/chip hover background

## Scope

- Frontend only (TypeScript types, Svelte component, fixture, tests)
- No backend changes (consumes existing WorkflowItem from IPC)
- Modifies: session-display spec (WorkflowCard rendering), tool-viewer-routing spec (Workflow tool routing)

## Capabilities affected

- `session-display` — adds WorkflowCard rendering scenarios
- `tool-viewer-routing` — adds Workflow tool summary case
