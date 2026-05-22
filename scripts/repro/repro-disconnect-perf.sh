#!/usr/bin/env bash
# Reproduce A2: measure ssh_disconnect IPC + post-disconnect list latency.
#
# Validates the disconnect path itself is fast (165ms in our measurements)
# and switching back to Local does not leave any cleanup residue. This is
# a regression baseline for the disconnect flow — if a future PR makes any
# of these timings significantly worse, run this and compare.
#
# Usage:
#   bash scripts/repro/repro-disconnect-perf.sh
#
# Prereqs same as repro-ssh-dead-channel.sh.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
JOB_DIR="${CLAUDE_JOB_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/cdt-repro.XXXXXX")}"
SSH_HOST="${CDT_SSH_TEST_HOST:-localhost}"
SSH_PORT="${CDT_SSH_TEST_PORT:-2222}"
SSH_USER="${CDT_SSH_TEST_USER:-devuser}"

mkdir -p "$JOB_DIR"
cd "$ROOT_DIR"

now_ms() { python3 -c 'import time; print(int(time.time()*1000))'; }
log() { printf '[repro-A2 %s] %s\n' "$(date +%H:%M:%S)" "$*" | tee -a "$JOB_DIR/repro-A2.log"; }
fail() { log "FAIL: $*"; exit 1; }

cleanup() {
    if [ -n "${SERVER_PID:-}" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

PORT=$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1]);s.close()')
mkdir -p "$JOB_DIR/home-A2/.claude"
[ -d "$HOME/.ssh" ] && [ ! -L "$JOB_DIR/home-A2/.ssh" ] && ln -s "$HOME/.ssh" "$JOB_DIR/home-A2/.ssh"
cat > "$JOB_DIR/home-A2/.claude/claude-devtools-config.json" <<JSON
{"httpServer":{"enabled":true,"port":$PORT},"general":{"claudeRootPath":"$JOB_DIR/home-A2/.claude"}}
JSON

REAL_HOME="$HOME"
log "start cdt-cli HTTP server: port=$PORT"
HOME="$JOB_DIR/home-A2" \
    CARGO_HOME="${CARGO_HOME:-$REAL_HOME/.cargo}" \
    RUSTUP_HOME="${RUSTUP_HOME:-$REAL_HOME/.rustup}" \
    RUST_LOG="cdt_api=debug,cdt_ssh=debug,cdt_watch=info,cdt_cli=info,cdt_discover=info" \
    cargo run -q -p cdt-cli --bin cdt > "$JOB_DIR/cdt-server-A2.log" 2>&1 &
SERVER_PID=$!

for i in $(seq 1 60); do
    if curl -fsS "http://127.0.0.1:$PORT/api/projects" >/dev/null 2>&1; then
        log "server ready"; break
    fi
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
        cat "$JOB_DIR/cdt-server-A2.log"; fail "cdt-cli failed to start"
    fi
    sleep 1
done

log "===== Stage 0: Local baseline (pre-connect) ====="
T0=$(now_ms)
curl -sS "http://127.0.0.1:$PORT/api/repository-groups" > "$JOB_DIR/A2-local-groups-pre.json"
T1=$(now_ms)
log "local list_repository_groups (pre-connect): $((T1-T0))ms (groups=$(jq 'length' "$JOB_DIR/A2-local-groups-pre.json"))"

log "===== Stage 1: ssh_connect ====="
CONNECT_BODY=$(jq -nc --arg h "$SSH_HOST" --argjson p "$SSH_PORT" --arg u "$SSH_USER" \
    '{host:$h,port:$p,username:$u,authMethod:"sshConfig"}')
T0=$(now_ms)
curl -sS -X POST -H 'content-type: application/json' --data "$CONNECT_BODY" \
    "http://127.0.0.1:$PORT/api/ssh/connect" > "$JOB_DIR/A2-connect.json"
T1=$(now_ms)
CONTEXT_ID=$(jq -r '.contextId // empty' "$JOB_DIR/A2-connect.json")
[ -n "$CONTEXT_ID" ] || fail "ssh_connect failed"
log "connected ctx=$CONTEXT_ID elapsed=$((T1-T0))ms"

log "===== Stage 2: list under SSH context ====="
T0=$(now_ms)
curl -sS "http://127.0.0.1:$PORT/api/repository-groups" > "$JOB_DIR/A2-ssh-groups.json"
T1=$(now_ms)
log "ssh list_repository_groups: $((T1-T0))ms (groups=$(jq 'length' "$JOB_DIR/A2-ssh-groups.json"))"

SSH_PROJECT=$(jq -r '[.[].worktrees[]?.id] | unique | .[0] // ""' "$JOB_DIR/A2-ssh-groups.json")
if [ -n "$SSH_PROJECT" ] && [ "$SSH_PROJECT" != "null" ]; then
    SSH_ENC=$(python3 -c "from urllib.parse import quote; print(quote('''$SSH_PROJECT''', safe=''))")
    T0=$(now_ms)
    curl -sS "http://127.0.0.1:$PORT/api/projects/$SSH_ENC/sessions?page=1&pageSize=50" > "$JOB_DIR/A2-ssh-sessions.json"
    T1=$(now_ms)
    log "ssh list_sessions: $((T1-T0))ms"
fi

log "===== Stage 3: ssh_disconnect ====="
DISCONNECT_BODY=$(jq -nc --arg id "$CONTEXT_ID" '{contextId:$id}')
T0=$(now_ms)
curl -sS -X POST -H 'content-type: application/json' --data "$DISCONNECT_BODY" \
    "http://127.0.0.1:$PORT/api/ssh/disconnect" > "$JOB_DIR/A2-disconnect.json"
T1=$(now_ms)
log "ssh_disconnect IPC: $((T1-T0))ms"

ACTIVE_KIND=$(curl -sS "http://127.0.0.1:$PORT/api/contexts/active" | jq -r '.kind // "?"')
log "active.kind=${ACTIVE_KIND} (expected=local)"

log "===== Stage 4: list calls after disconnect ====="
for call in 1 2 3; do
    T0=$(now_ms)
    curl -sS "http://127.0.0.1:$PORT/api/repository-groups" > "$JOB_DIR/A2-post-groups-$call.json"
    T1=$(now_ms)
    log "  list_repository_groups call $call: $((T1-T0))ms"
done

POST_PROJECT=$(jq -r '[.[].worktrees[]?.id] | unique | .[0] // ""' "$JOB_DIR/A2-post-groups-1.json")
if [ -n "$POST_PROJECT" ] && [ "$POST_PROJECT" != "null" ]; then
    POST_ENC=$(python3 -c "from urllib.parse import quote; print(quote('''$POST_PROJECT''', safe=''))")
    for call in 1 2; do
        T0=$(now_ms)
        curl -sS "http://127.0.0.1:$PORT/api/projects/$POST_ENC/sessions?page=1&pageSize=50" > "$JOB_DIR/A2-post-sessions-$call.json"
        T1=$(now_ms)
        log "  list_sessions call $call: $((T1-T0))ms"
    done
fi

log "===== summary ====="
log "log: $JOB_DIR/repro-A2.log"
log "cdt server log: $JOB_DIR/cdt-server-A2.log"
echo "---SSH/disconnect lifecycle excerpt---"
grep -E "ssh_disconnect|ssh_mgr_disconnect|cancel_remote_watcher|abort_scans|context_generation|context_changed" \
    "$JOB_DIR/cdt-server-A2.log" 2>/dev/null | head -40 || true
