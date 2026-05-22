#!/usr/bin/env bash
# Boot an isolated SSH test container (cdt-ssh-fixture-test, port 2223) with an
# RW-mounted independent fixture home, so the desktop app can tell SSH from
# Local data on the sidebar at a glance. Coexists with cdt-ssh-test (port 2222).
#
# Usage:
#   bash scripts/repro/setup-ssh-fixture-container.sh up                       # default 3 projects × 1 session × 4 msgs
#   bash scripts/repro/setup-ssh-fixture-container.sh up --scale 50 10 20      # large fixture (50 projects, 10 sessions/project, 20 msgs/session)
#   bash scripts/repro/setup-ssh-fixture-container.sh refresh [--scale N M K]  # rewrite fixture only
#   bash scripts/repro/setup-ssh-fixture-container.sh down                     # stop + remove container + clean fixture home
#   bash scripts/repro/setup-ssh-fixture-container.sh status                   # show container + fixture state
#
# Desktop connection config (after `up`):
#   Host:     localhost
#   Port:     2223 (override via CDT_SSH_FIXTURE_PORT)
#   Username: devuser
#   Auth:     SSH config (uses your ~/.ssh/id_*.pub)

set -euo pipefail

CONTAINER="${CDT_SSH_FIXTURE_CONTAINER:-cdt-ssh-fixture-test}"
HOST_PORT="${CDT_SSH_FIXTURE_PORT:-2223}"
FIXTURE_HOME="${CDT_SSH_FIXTURE_HOME:-$HOME/.cdt-ssh-fixture-home}"
USER_NAME="${CDT_SSH_FIXTURE_USER:-devuser}"
PUID="${CDT_SSH_FIXTURE_PUID:-$(id -u)}"
PGID="${CDT_SSH_FIXTURE_PGID:-$(id -g)}"
IMAGE="${CDT_SSH_FIXTURE_IMAGE:-lscr.io/linuxserver/openssh-server:latest}"

ACTION="${1:-up}"
shift || true

# Parse --scale N M K
SCALE_PROJECTS=3
SCALE_SESSIONS=1
SCALE_MSGS=4
while [ $# -gt 0 ]; do
    case "$1" in
        --scale)
            SCALE_PROJECTS="${2:?--scale requires PROJECTS}"
            SCALE_SESSIONS="${3:?--scale requires SESSIONS}"
            SCALE_MSGS="${4:?--scale requires MSGS}"
            shift 4
            ;;
        *) echo "[fixture] unknown arg: $1" >&2; exit 1 ;;
    esac
done

log()  { printf '[fixture] %s\n' "$*"; }
fail() { printf '[fixture][FAIL] %s\n' "$*" >&2; exit 1; }

container_exists() { docker ps -a --format '{{.Names}}' | grep -qx "$CONTAINER"; }
container_running() { [ "$(docker inspect -f '{{.State.Running}}' "$CONTAINER" 2>/dev/null || echo false)" = "true" ]; }

find_public_key() {
    for k in "$HOME/.ssh/id_ed25519.pub" "$HOME/.ssh/id_rsa.pub" "$HOME/.ssh/id_ecdsa.pub"; do
        [ -f "$k" ] && { cat "$k"; return 0; }
    done
    return 1
}

# Generate JSONL fixture via python for speed and correctness with large scale.
# Layout: $FIXTURE_HOME/.claude/projects/-srv-ssh-fixture-NNN/<session-uuid>.jsonl
inject_fixtures() {
    log "Inject fixture: ${SCALE_PROJECTS} projects × ${SCALE_SESSIONS} sessions × ${SCALE_MSGS} msgs"
    log "  fixture home: $FIXTURE_HOME"
    rm -rf "$FIXTURE_HOME/.claude/projects"
    mkdir -p "$FIXTURE_HOME/.claude/projects"

    CONTAINER_NAME="$CONTAINER" \
    PROJECTS="$SCALE_PROJECTS" \
    SESSIONS="$SCALE_SESSIONS" \
    MSGS="$SCALE_MSGS" \
    ROOT="$FIXTURE_HOME/.claude/projects" \
    python3 - <<'PY'
import json
import os
import pathlib
from datetime import datetime, timedelta

projects = int(os.environ["PROJECTS"])
sessions_per = int(os.environ["SESSIONS"])
msgs_per = int(os.environ["MSGS"])
root = pathlib.Path(os.environ["ROOT"])
container = os.environ["CONTAINER_NAME"]

base_ts = datetime(2026, 5, 22, 10, 0, 0)


def iso(t: datetime) -> str:
    return t.strftime("%Y-%m-%dT%H:%M:%S.000Z")


for p_idx in range(projects):
    name = f"{p_idx:03d}"
    cwd = f"/srv/ssh-fixture-{name}"
    proj_id = f"-srv-ssh-fixture-{name}"
    proj_dir = root / proj_id
    proj_dir.mkdir(parents=True, exist_ok=True)

    for s_idx in range(sessions_per):
        # Deterministic session uuid from indices
        session_id = (
            f"{p_idx:08x}-{s_idx:04x}-fffe-aaaa-{s_idx:012x}"
        )
        jsonl = proj_dir / f"{session_id}.jsonl"
        with jsonl.open("w") as f:
            # agent-setting + permission-mode preamble
            f.write(json.dumps({
                "type": "agent-setting",
                "agentSetting": "claude",
                "sessionId": session_id,
            }) + "\n")
            f.write(json.dumps({
                "type": "permission-mode",
                "permissionMode": "auto",
                "sessionId": session_id,
            }) + "\n")

            parent = None
            for m_idx in range(msgs_per):
                ts = iso(base_ts + timedelta(seconds=m_idx))
                msg_uuid = f"msg-{p_idx:03d}-{s_idx:03d}-{m_idx:04d}"

                if m_idx % 2 == 0:
                    entry = {
                        "parentUuid": parent,
                        "isSidechain": False,
                        "type": "user",
                        "message": {
                            "role": "user",
                            "content": (
                                f"[SSH DOCKER FIXTURE {name.upper()} S{s_idx:03d}] "
                                f"User msg #{m_idx}: this exists ONLY on docker {container}. "
                                f"If you see me in the sidebar, SSH data really loaded. "
                                f"project=-srv-ssh-fixture-{name} session={session_id}"
                            ),
                        },
                        "uuid": msg_uuid,
                        "timestamp": ts,
                        "userType": "external",
                        "entrypoint": "cli",
                        "cwd": cwd,
                        "sessionId": session_id,
                        "version": "2.1.140",
                        "gitBranch": "main",
                    }
                else:
                    entry = {
                        "parentUuid": parent,
                        "isSidechain": False,
                        "message": {
                            "id": f"resp_{p_idx}_{s_idx}_{m_idx}",
                            "type": "message",
                            "role": "assistant",
                            "model": "sonnet-fixture",
                            "usage": {"input_tokens": 0, "output_tokens": 0},
                            "content": [{
                                "type": "text",
                                "text": (
                                    f"[SSH DOCKER FIXTURE {name.upper()} S{s_idx:03d} RESPONSE] "
                                    f"Assistant msg #{m_idx}: this session lives in docker {container} only."
                                ),
                            }],
                        },
                        "type": "assistant",
                        "uuid": msg_uuid,
                        "timestamp": ts,
                        "userType": "external",
                        "entrypoint": "cli",
                        "cwd": cwd,
                        "sessionId": session_id,
                        "version": "2.1.140",
                        "gitBranch": "main",
                    }
                f.write(json.dumps(entry) + "\n")
                parent = msg_uuid
PY
    log "  injected $SCALE_PROJECTS projects under $FIXTURE_HOME/.claude/projects/-srv-ssh-fixture-*"
}

cmd_up() {
    command -v docker >/dev/null 2>&1 || fail "docker not installed"
    PUBLIC_KEY=$(find_public_key) || fail "no ~/.ssh/id_*.pub found; run ssh-keygen first"

    inject_fixtures

    if container_exists; then
        if container_running; then
            log "container $CONTAINER already running (port=$HOST_PORT); fixture refreshed"
            return 0
        fi
        log "container exists but not running; starting"
        docker start "$CONTAINER" >/dev/null
    else
        log "creating container $CONTAINER (port=$HOST_PORT, mount=$FIXTURE_HOME/.claude)"
        docker run -d --name "$CONTAINER" \
            -e PUID="$PUID" -e PGID="$PGID" -e USER_NAME="$USER_NAME" \
            -e PASSWORD_ACCESS=false \
            -e PUBLIC_KEY="$PUBLIC_KEY" \
            -p "${HOST_PORT}:2222" \
            -v "${FIXTURE_HOME}/.claude:/config/.claude" \
            "$IMAGE" >/dev/null
    fi

    log "waiting for SSH ready..."
    local ready=0
    for i in $(seq 1 60); do
        if ssh -o BatchMode=yes -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
            -o ConnectTimeout=2 -p "$HOST_PORT" "${USER_NAME}@localhost" 'echo ok' >/dev/null 2>&1; then
            ready=1; break
        fi
        sleep 1
    done
    [ "$ready" = "1" ] || fail "SSH did not become ready in 60s; check: docker logs $CONTAINER"

    log "SSH ready"
    cat <<EOF

==============================================
Desktop connection config:
  Host:     localhost
  Port:     $HOST_PORT
  Username: $USER_NAME
  Auth:     SSH config (public key)

After connecting, the sidebar will show $SCALE_PROJECTS projects:
  -srv-ssh-fixture-000 ... -srv-ssh-fixture-$(printf '%03d' $((SCALE_PROJECTS-1)))
Each project: $SCALE_SESSIONS session(s), $SCALE_MSGS msg(s).
Messages contain [SSH DOCKER FIXTURE ...] markers.

Switching back to Local should make these disappear (your real
~/.claude/projects has no -srv-ssh-fixture-* prefix).

Tear down: bash scripts/repro/setup-ssh-fixture-container.sh down
==============================================
EOF
}

cmd_down() {
    if container_exists; then
        log "stop + rm container $CONTAINER"
        docker stop "$CONTAINER" >/dev/null 2>&1 || true
        docker rm "$CONTAINER" >/dev/null 2>&1 || true
    else
        log "container $CONTAINER does not exist"
    fi
    if [ -d "$FIXTURE_HOME" ]; then
        log "removing fixture home: $FIXTURE_HOME"
        rm -rf "$FIXTURE_HOME"
    fi
    log "done"
}

cmd_refresh() {
    inject_fixtures
    log "fixture rewritten. SSH polling watcher should pick up file changes within ~3s."
}

cmd_status() {
    if container_exists; then
        local running="stopped"
        container_running && running="running"
        log "container $CONTAINER: $running (port=$HOST_PORT, image=$IMAGE)"
    else
        log "container $CONTAINER: not present"
    fi
    if [ -d "$FIXTURE_HOME/.claude/projects" ]; then
        local count
        count=$(find "$FIXTURE_HOME/.claude/projects" -mindepth 1 -maxdepth 1 -type d -name "-srv-ssh-fixture-*" | wc -l | tr -d ' ')
        log "fixture home: $FIXTURE_HOME ($count fixture projects)"
    else
        log "fixture home: missing"
    fi
}

case "$ACTION" in
    up|setup)         cmd_up ;;
    down|clean|rm)    cmd_down ;;
    refresh|reinject) cmd_refresh ;;
    status|ls)        cmd_status ;;
    *) fail "unknown action: $ACTION (supported: up / down / refresh / status)" ;;
esac
