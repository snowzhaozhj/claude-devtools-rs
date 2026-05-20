#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SSH_HOST="${CDT_SSH_TEST_HOST:-localhost}"
SSH_PORT="${CDT_SSH_TEST_PORT:-2222}"
SSH_USER="${CDT_SSH_TEST_USER:-devuser}"
REMOTE_PROJECTS="${CDT_SSH_TEST_PROJECTS:-/config/.claude/projects}"
DETAIL_BUDGET_MS="${CDT_SSH_DETAIL_BUDGET_MS:-3000}"
SSE_TIMEOUT_SECONDS="${CDT_SSH_SSE_TIMEOUT_SECONDS:-15}"
SERVER_TIMEOUT_SECONDS="${CDT_SSH_SERVER_TIMEOUT_SECONDS:-120}"
KEEP_TMP="${CDT_SSH_KEEP_TMP:-0}"
REAL_HOME="$HOME"

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/cdt-ssh-e2e.XXXXXX")"
SERVER_PID=""
PORT=""

log() { printf '[ssh-e2e] %s\n' "$*"; }
fail() {
    local kind="$1"
    shift
    printf '[ssh-e2e][%s] %s\n' "$kind" "$*" >&2
    if [ -n "${SERVER_PID}" ] && kill -0 "${SERVER_PID}" 2>/dev/null; then
        printf '[ssh-e2e][%s] server log: %s\n' "$kind" "$TMP_DIR/cdt-server.log" >&2
    fi
    exit 1
}

cleanup() {
    if [ -n "${SERVER_PID}" ] && kill -0 "${SERVER_PID}" 2>/dev/null; then
        kill "${SERVER_PID}" 2>/dev/null || true
        wait "${SERVER_PID}" 2>/dev/null || true
    fi
    if [ "${KEEP_TMP}" != "1" ]; then
        rm -rf "$TMP_DIR"
    else
        log "保留临时目录: $TMP_DIR"
    fi
}
trap cleanup EXIT

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || fail "missing_tool" "缺少命令: $1"
}

now_ms() {
    python3 - <<'PY'
import time
print(int(time.time() * 1000))
PY
}

sessions_array_filter='if type == "array" then . elif has("sessions") then .sessions elif has("items") then .items else [] end'

json_get() {
    jq -r "$1" "$2"
}

curl_json() {
    local label="$1"
    local method="$2"
    local path="$3"
    local body="${4:-}"
    local output="$5"
    local start end status
    start=$(now_ms)
    if [ -n "$body" ]; then
        status=$(curl -sS -o "$output" -w '%{http_code}' -X "$method" -H 'content-type: application/json' --data "$body" "http://127.0.0.1:${PORT}${path}" 2>"$output.stderr") || {
            cat "$output.stderr" >&2 || true
            fail "http_request_failed" "$label 请求失败: $method $path"
        }
    else
        status=$(curl -sS -o "$output" -w '%{http_code}' -X "$method" "http://127.0.0.1:${PORT}${path}" 2>"$output.stderr") || {
            cat "$output.stderr" >&2 || true
            fail "http_request_failed" "$label 请求失败: $method $path"
        }
    fi
    end=$(now_ms)
    printf '%s\t%s\t%s\n' "$label" "$((end - start))" "$status" >> "$TMP_DIR/timings.tsv"
    if [ "$status" -lt 200 ] || [ "$status" -ge 300 ]; then
        cat "$output" >&2 || true
        fail "http_${label}" "$label 返回 HTTP $status"
    fi
}

wait_for_server() {
    local deadline=$((SECONDS + SERVER_TIMEOUT_SECONDS))
    while [ "$SECONDS" -lt "$deadline" ]; do
        if curl -fsS "http://127.0.0.1:${PORT}/api/projects" >/dev/null 2>&1; then
            return 0
        fi
        if [ -n "${SERVER_PID}" ] && ! kill -0 "${SERVER_PID}" 2>/dev/null; then
            cat "$TMP_DIR/cdt-server.log" >&2 || true
            fail "server_start" "cdt-cli 进程提前退出"
        fi
        sleep 0.2
    done
    cat "$TMP_DIR/cdt-server.log" >&2 || true
    fail "server_start" "cdt-cli 在 ${SERVER_TIMEOUT_SECONDS}s 内未就绪，端口 $PORT"
}

choose_port() {
    python3 - <<'PY'
import socket
s = socket.socket()
s.bind(('127.0.0.1', 0))
print(s.getsockname()[1])
s.close()
PY
}

ssh_base=(ssh -F /dev/null -o BatchMode=yes -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=5 -p "$SSH_PORT" "${SSH_USER}@${SSH_HOST}")
sftp_base=(sftp -F /dev/null -o BatchMode=yes -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=5 -P "$SSH_PORT" "${SSH_USER}@${SSH_HOST}")

require_cmd cargo
require_cmd curl
require_cmd jq
require_cmd ssh
require_cmd sftp
require_cmd python3
if ! command -v timeout >/dev/null 2>&1 && ! command -v gtimeout >/dev/null 2>&1; then
    log "未找到 timeout/gtimeout，将使用 python fallback 控制 SSE 超时"
fi

log "Docker preflight: SSH ${SSH_USER}@${SSH_HOST}:${SSH_PORT}"
if ! "${ssh_base[@]}" 'printf ok' >"$TMP_DIR/ssh.out" 2>"$TMP_DIR/ssh.err"; then
    cat "$TMP_DIR/ssh.err" >&2 || true
    fail "ssh_connect_failed" "无法通过 public key/ssh_config 连接 cdt-ssh-test"
fi

if ! printf 'ls %s\n' "$REMOTE_PROJECTS" | "${sftp_base[@]}" >"$TMP_DIR/sftp.out" 2>"$TMP_DIR/sftp.err"; then
    cat "$TMP_DIR/sftp.err" >&2 || true
    fail "sftp_failed" "SFTP 无法列出 $REMOTE_PROJECTS"
fi

if ! "${ssh_base[@]}" "root='$REMOTE_PROJECTS'; \
if [ ! -d \"\$root\" ]; then echo jsonl_count=0; echo max_jsonl_bytes=0; echo project_dir_count=0; exit 0; fi; \
find \"\$root\" -mindepth 2 -maxdepth 2 -name '*.jsonl' -type f > /tmp/cdt-ssh-jsonl-files.\$\$; \
count=0; max=0; \
while IFS= read -r f; do count=\$((count + 1)); size=\$(wc -c < \"\$f\" | tr -d ' '); [ \"\$size\" -gt \"\$max\" ] && max=\$size; done < /tmp/cdt-ssh-jsonl-files.\$\$; \
rm -f /tmp/cdt-ssh-jsonl-files.\$\$; \
projects=\$(find \"\$root\" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' '); \
echo jsonl_count=\$count; echo max_jsonl_bytes=\$max; echo project_dir_count=\$projects" >"$TMP_DIR/remote-stats.txt" 2>"$TMP_DIR/remote-stats.err"; then
    cat "$TMP_DIR/remote-stats.err" >&2 || true
    fail "ssh_remote_stats_failed" "无法统计远端 JSONL 背景数据"
fi
cat "$TMP_DIR/remote-stats.txt"
remote_jsonl_count=$(grep '^jsonl_count=' "$TMP_DIR/remote-stats.txt" | cut -d= -f2)
if [ "${remote_jsonl_count:-0}" -lt 1 ]; then
    fail "remote_fixture_empty" "$REMOTE_PROJECTS 下未发现任何 */*.jsonl，无法做真实 HTTP 验收"
fi

PORT="${CDT_SSH_E2E_PORT:-$(choose_port)}"
mkdir -p "$TMP_DIR/home/.claude"
if [ -d "$REAL_HOME/.ssh" ]; then
    ln -s "$REAL_HOME/.ssh" "$TMP_DIR/home/.ssh"
fi
cat > "$TMP_DIR/home/.claude/claude-devtools-config.json" <<JSON
{
  "httpServer": { "enabled": true, "port": $PORT },
  "general": { "claudeRootPath": "$TMP_DIR/home/.claude" }
}
JSON

log "启动 cdt-cli HTTP server: port=$PORT HOME=$TMP_DIR/home"
(
    cd "$ROOT_DIR"
    HOME="$TMP_DIR/home" \
        CARGO_HOME="${CARGO_HOME:-$REAL_HOME/.cargo}" \
        RUSTUP_HOME="${RUSTUP_HOME:-$REAL_HOME/.rustup}" \
        RUST_LOG="${RUST_LOG:-cdt_api=info,cdt_ssh=info,cdt_cli=info}" \
        cargo run -q -p cdt-cli --bin cdt
) >"$TMP_DIR/cdt-server.log" 2>&1 &
SERVER_PID=$!
wait_for_server

CONNECT_BODY=$(jq -nc \
    --arg host "$SSH_HOST" \
    --argjson port "$SSH_PORT" \
    --arg username "$SSH_USER" \
    '{host:$host, port:$port, username:$username, authMethod:"sshConfig"}')

curl_json "ssh_connect" POST "/api/ssh/connect" "$CONNECT_BODY" "$TMP_DIR/connect.json"
CONTEXT_ID=$(json_get '.contextId // empty' "$TMP_DIR/connect.json")
[ -n "$CONTEXT_ID" ] || fail "ssh_connect_failed" "ssh_connect 响应缺 contextId: $(cat "$TMP_DIR/connect.json")"
log "SSH context: $CONTEXT_ID"

curl_json "active_context" GET "/api/contexts/active" "" "$TMP_DIR/active.json"
jq -e --arg id "$CONTEXT_ID" '.id == $id and .kind == "ssh" and .isActive == true' "$TMP_DIR/active.json" >/dev/null || \
    fail "active_context_failed" "active context 不是 SSH: $(cat "$TMP_DIR/active.json")"

curl_json "repository_groups" GET "/api/repository-groups" "" "$TMP_DIR/groups.json"
group_count=$(jq 'length' "$TMP_DIR/groups.json")
[ "$group_count" -gt 0 ] || fail "aggregation_failed" "repository-groups 为空"
if jq -e 'any(.[]; (.worktrees | length) > 1)' "$TMP_DIR/groups.json" >/dev/null; then
    log "worktree 聚合断言通过：存在 worktrees.length > 1 的 group"
else
    log "远端 fixture 未暴露多 worktree group；跳过 worktrees.length > 1 强断言"
fi
project_ids_json=$(jq '[.[] | .worktrees[]?.id] | unique' "$TMP_DIR/groups.json")
project_count=$(printf '%s' "$project_ids_json" | jq 'length')
[ "$project_count" -ge 1 ] || fail "aggregation_failed" "repository-groups 没有 worktree project id: $(cat "$TMP_DIR/groups.json")"

first_project=$(printf '%s' "$project_ids_json" | jq -r '.[0]')
second_project=$(printf '%s' "$project_ids_json" | jq -r '.[1] // empty')
encoded_first=$(python3 - <<PY
from urllib.parse import quote
print(quote('''$first_project''', safe=''))
PY
)
curl_json "sessions_first" GET "/api/projects/${encoded_first}/sessions?page=1&pageSize=50" "" "$TMP_DIR/sessions-first.json"
first_sessions=$(jq "${sessions_array_filter} | [.[]?.sessionId] | unique" "$TMP_DIR/sessions-first.json")
first_session_count=$(printf '%s' "$first_sessions" | jq 'length')
[ "$first_session_count" -gt 0 ] || fail "project_sessions_failed" "project $first_project 没有 session"

if [ -n "$second_project" ]; then
    encoded_second=$(python3 - <<PY
from urllib.parse import quote
print(quote('''$second_project''', safe=''))
PY
)
    curl_json "sessions_second" GET "/api/projects/${encoded_second}/sessions?page=1&pageSize=50" "" "$TMP_DIR/sessions-second.json"
    second_sessions=$(jq "${sessions_array_filter} | [.[]?.sessionId] | unique" "$TMP_DIR/sessions-second.json")
    if [ "$(printf '%s' "$second_sessions" | jq 'length')" -gt 0 ] && [ "$first_sessions" != "$second_sessions" ]; then
        log "project 切换断言通过：session id 集合变化"
    else
        log "远端 fixture 不足两个非空且不同 session 集合的 project；跳过切换差异强断言"
    fi
else
    log "远端 fixture 只有一个 project；跳过切换差异强断言"
fi

SESSION_ID=$(printf '%s' "$first_sessions" | jq -r '.[0]')
SSE_OUT="$TMP_DIR/sse.out"
SSE_RAW="$TMP_DIR/sse.raw"
log "等待 session_metadata_update SSE: project=$first_project session=$SESSION_ID timeout=${SSE_TIMEOUT_SECONDS}s"
if command -v timeout >/dev/null 2>&1; then
    timeout_cmd=(timeout "$SSE_TIMEOUT_SECONDS")
elif command -v gtimeout >/dev/null 2>&1; then
    timeout_cmd=(gtimeout "$SSE_TIMEOUT_SECONDS")
else
    timeout_cmd=(python3 -c 'import signal, subprocess, sys; p=subprocess.Popen(sys.argv[2:]); signal.signal(signal.SIGALRM, lambda *_: (p.kill(), sys.exit(124))); signal.alarm(int(sys.argv[1])); sys.exit(p.wait())' "$SSE_TIMEOUT_SECONDS")
fi
("${timeout_cmd[@]}" curl -N -sS "http://127.0.0.1:${PORT}/api/events" >"$SSE_RAW" 2>"$TMP_DIR/sse.err") &
SSE_PID=$!
sleep 0.5
curl_json "sessions_for_sse" GET "/api/projects/${encoded_first}/sessions?page=1&pageSize=50" "" "$TMP_DIR/sessions-sse-trigger.json"
set +e
wait "$SSE_PID"
sse_status=$?
set -e
grep 'session_metadata_update' "$SSE_RAW" > "$SSE_OUT" || true
if [ -s "$SSE_OUT" ]; then
    if grep -Eq 'message_count|messageCount' "$SSE_OUT"; then
        log "metadata SSE 断言通过"
    else
        cat "$SSE_OUT" >&2 || true
        fail "metadata_sse_failed" "metadata SSE 缺 messageCount/message_count 字段"
    fi
else
    file_change_count=$(grep -c 'file_change' "$SSE_RAW" 2>/dev/null || true)
    cat "$TMP_DIR/sse.err" >&2 || true
    fail "metadata_sse_timeout" "${SSE_TIMEOUT_SECONDS}s 内未收到 session_metadata_update；同期 file_change=$file_change_count"
fi
if [ "$sse_status" != "0" ] && [ "$sse_status" != "124" ]; then
    fail "metadata_sse_failed" "SSE curl 异常退出: $sse_status"
fi

encoded_session=$(python3 - <<PY
from urllib.parse import quote
print(quote('''$SESSION_ID''', safe=''))
PY
)
start=$(now_ms)
curl_json "session_detail" GET "/api/sessions/${encoded_session}" "" "$TMP_DIR/detail.json"
end=$(now_ms)
detail_ms=$((end - start))
jq -e --arg sid "$SESSION_ID" '.sessionId == $sid and (.chunks != null)' "$TMP_DIR/detail.json" >/dev/null || \
    fail "detail_failed" "detail 响应不含目标 session/chunks: $(jq '{sessionId, projectId, chunksType:(.chunks|type)}' "$TMP_DIR/detail.json")"
if [ "$detail_ms" -gt "$DETAIL_BUDGET_MS" ]; then
    fail "detail_timeout" "GET /api/sessions/$SESSION_ID 耗时 ${detail_ms}ms，超过预算 ${DETAIL_BUDGET_MS}ms；疑似全局扫描或远端大文件过慢"
fi
log "session detail 断言通过: ${detail_ms}ms <= ${DETAIL_BUDGET_MS}ms"

DISCONNECT_BODY=$(jq -nc --arg id "$CONTEXT_ID" '{contextId:$id}')
curl_json "ssh_disconnect" POST "/api/ssh/disconnect" "$DISCONNECT_BODY" "$TMP_DIR/disconnect.json"
curl_json "local_projects_after_disconnect" GET "/api/projects" "" "$TMP_DIR/local-projects.json"
local_kind=$(curl -sS "http://127.0.0.1:${PORT}/api/contexts/active" | jq -r '.kind // empty')
[ "$local_kind" = "local" ] || fail "disconnect_failed" "disconnect 后 active context 未恢复 local"
log "disconnect 后 local /api/projects 正常"

log "耗时摘要(label ms status):"
cat "$TMP_DIR/timings.tsv"
log "全部通过"
