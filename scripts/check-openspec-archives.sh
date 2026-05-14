#!/usr/bin/env bash
set -euo pipefail

if ! command -v openspec >/dev/null 2>&1; then
  echo "openspec CLI not found; cannot check completed active changes." >&2
  exit 2
fi

json=$(openspec list --json)

completed_changes=$(printf '%s' "$json" | python3 -c '
import json
import sys

data = json.load(sys.stdin)
completed = []
for change in data.get("changes", []):
    name = change.get("name")
    status = change.get("status")
    completed_tasks = change.get("completedTasks")
    total_tasks = change.get("totalTasks")
    if name and (status == "complete" or (total_tasks is not None and completed_tasks == total_tasks)):
        completed.append(name)
print("\n".join(completed))
')

if [[ -z "$completed_changes" ]]; then
  exit 0
fi

{
  echo "Blocking because completed OpenSpec changes are still active:"
  printf '  - %s\n' $completed_changes
  echo ""
  echo "Archive them before committing or opening a PR:"
  printf '  /opsx:archive %s\n' $completed_changes
} >&2

exit 2
