#!/usr/bin/env bash
# Reproduce A1: SFTP "dead channel" scenario (sshd suspended, not closed).
#
# Tests whether polling watcher's permanent failure detection triggers
# self-heal disconnect. Currently FAILS: timeout errors are classified as
# `Transient("timeout")` and never count toward PERMANENT_FAILURE_THRESHOLD,
# so dead_signal never fires and active context stays stuck on SSH.
#
# See `openspec/followups.md::ssh-remote-context::SFTP 失效检测不完整`.
#
# Usage:
#   bash scripts/repro/repro-ssh-dead-channel.sh
#
# Prereqs:
#   - cdt-ssh-test docker container running (port 2222), bind-mounting your
#     local ~/.claude (the original verify-ssh-docker-e2e.sh setup)
#   - cargo build -p cdt-cli (so server starts in seconds, not minutes)
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
JOB_DIR="${CLAUDE_JOB_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/cdt-repro.XXXXXX")}"
SSH_HOST="${CDT_SSH_TEST_HOST:-localhost}"
SSH_PORT="${CDT_SSH_TEST_PORT:-2222}"
SSH_USER="${CDT_SSH_TEST_USER:-devuser}"
DOCKER_NAME="${CDT_SSH_TEST_CONTAINER:-cdt-ssh-test}"

mkdir -p "$JOB_DIR"
cd "$ROOT_DIR"

now_ms() { python3 -c 'import time; print(int(time.time()*1000))'; }
log() { printf '[repro-A1 %s] %s\n' "$(date +%H:%M:%S)" "$*" | tee -a "$JOB_DIR/repro-A1.log"; }
fail() { log "FAIL: $*"; exit 1; }

require_docker_container() {
    docker ps --format '{{.Names}}' | grep -Fqx "$DOCKER_NAME" \
        || fail "docker container '$DOCKER_NAME' not running. start it via docker run lscr.io/linuxserver/openssh-server (see scripts/verify-ssh-docker-e2e.sh for ref)"
}

cleanup() {
    log "cleanup: resume sshd (CONT) + kill cdt server"
    docker exec "$DOCKER_NAME" pkill -CONT sshd 2>/dev/null || true
    if [ -n "${SERVER_PID:-}" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

require_docker_container
PORT=$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1]);s.close()')
mkdir -p "$JOB_DIR/home/.claude"
[ -d "$HOME/.ssh" ] && [ ! -L "$JOB_DIR/home/.ssh" ] && ln -s "$HOME/.ssh" "$JOB_DIR/home/.ssh"
cat > "$JOB_DIR/home/.claude/claude-devtools-config.json" <<JSON
{"httpServer":{"enabled":true,"port":$PORT},"general":{"claudeRootPath":"$JOB_DIR/home/.claude"}}
JSON

REAL_HOME="$HOME"
log "start cdt-cli HTTP server: port=$PORT"
HOME="$JOB_DIR/home" \
    CARGO_HOME="${CARGO_HOME:-$REAL_HOME/.cargo}" \
    RUSTUP_HOME="${RUSTUP_HOME:-$REAL_HOME/.rustup}" \
    RUST_LOG="cdt_api=info,cdt_ssh=debug,cdt_watch=debug,cdt_cli=info" \
    cargo run -q -p cdt-cli --bin cdt > "$JOB_DIR/cdt-server.log" 2>&1 &
SERVER_PID=$!

SERVER_READY=0
for i in $(seq 1 60); do
    if curl -fsS "http://127.0.0.1:$PORT/api/projects" >/dev/null 2>&1; then
        log "server ready"
        SERVER_READY=1
        break
    fi
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
        cat "$JOB_DIR/cdt-server.log"
        fail "cdt-cli server failed to start"
    fi
    sleep 1
done
[ "$SERVER_READY" = "1" ] || { cat "$JOB_DIR/cdt-server.log"; fail "cdt-cli server not ready after 60s"; }

CONNECT_BODY=$(jq -nc --arg h "$SSH_HOST" --argjson p "$SSH_PORT" --arg u "$SSH_USER" \
    '{host:$h,port:$p,username:$u,authMethod:"sshConfig"}')
log "ssh_connect"
T0=$(now_ms)
curl -sS -X POST -H 'content-type: application/json' --data "$CONNECT_BODY" \
    "http://127.0.0.1:$PORT/api/ssh/connect" > "$JOB_DIR/connect.json"
T1=$(now_ms)
CONTEXT_ID=$(jq -r '.contextId // empty' "$JOB_DIR/connect.json")
[ -n "$CONTEXT_ID" ] || fail "ssh_connect failed: $(cat "$JOB_DIR/connect.json")"
log "connected ctx=$CONTEXT_ID elapsed=$((T1-T0))ms"

log "baseline: list_repository_groups"
T0=$(now_ms)
curl -sS "http://127.0.0.1:$PORT/api/repository-groups" > "$JOB_DIR/groups.json"
T1=$(now_ms)
log "list_repository_groups baseline: $((T1-T0))ms"

PROJECT_ID=$(jq -r '[.[].worktrees[]?.id] | unique | .[0]' "$JOB_DIR/groups.json")
[ -n "$PROJECT_ID" ] && [ "$PROJECT_ID" != "null" ] || fail "no project_id: $(cat "$JOB_DIR/groups.json")"
ENCODED=$(PROJECT_ID="$PROJECT_ID" python3 -c "import os, urllib.parse; print(urllib.parse.quote(os.environ['PROJECT_ID'], safe=''))")

T0=$(now_ms)
curl -sS "http://127.0.0.1:$PORT/api/projects/$ENCODED/sessions?page=1&pageSize=50" > "$JOB_DIR/sessions-baseline.json"
T1=$(now_ms)
log "list_sessions baseline: $((T1-T0))ms"

log "===== suspend sshd to simulate dead SFTP channel ====="
docker exec "$DOCKER_NAME" pkill -STOP sshd
DEAD_T0=$(now_ms)
log "sshd suspended at T+0 (epoch_ms=$DEAD_T0)"

log "list_sessions immediately after STOP (timeout 30s)"
T0=$(now_ms)
HTTP_CODE=$(curl -sS -o "$JOB_DIR/sessions-during-dead.json" -w '%{http_code}' --max-time 30 \
    "http://127.0.0.1:$PORT/api/projects/$ENCODED/sessions?page=1&pageSize=50" 2>&1 || echo "TIMEOUT")
T1=$(now_ms)
log "list_sessions during-dead: elapsed=$((T1-T0))ms http=$HTTP_CODE"

log "===== monitor active context ====="
for i in $(seq 1 60); do
    NOW=$(now_ms)
    KIND=$(curl -sS --max-time 2 "http://127.0.0.1:$PORT/api/contexts/active" 2>/dev/null | jq -r '.kind // "?"')
    log "T+$((NOW-DEAD_T0))ms active.kind=$KIND"
    if [ "$KIND" = "local" ]; then
        log "active switched to local at T+$((NOW-DEAD_T0))ms"
        break
    fi
    sleep 0.5
done

log "===== resume sshd (CONT) before measuring post-recovery ====="
docker exec "$DOCKER_NAME" pkill -CONT sshd
sleep 1  # let sshd schedule + accept queued packets
log "post-recovery: list_sessions (should be fast IF self-heal triggered & re-connected)"
T0=$(now_ms)
curl -sS --max-time 10 "http://127.0.0.1:$PORT/api/projects/$ENCODED/sessions?page=1&pageSize=50" > "$JOB_DIR/sessions-after-recover.json" 2>&1 || true
T1=$(now_ms)
log "list_sessions post-recovery: $((T1-T0))ms"

log "===== summary ====="
log "log: $JOB_DIR/repro-A1.log"
log "cdt server log: $JOB_DIR/cdt-server.log"
echo "---SSH/lifecycle log excerpt---"
grep -E "ssh_disconnect|polling reported|dead_signal|skip stale|consecutive_permanent|SFTP channel appears dead|polling scan failed" \
    "$JOB_DIR/cdt-server.log" 2>/dev/null | head -50 || true
