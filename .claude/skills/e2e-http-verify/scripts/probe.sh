#!/usr/bin/env bash
# 后端 HTTP API 探针：列 projects / groups + 测 cache miss → fill 时序。
# 不 hardcode 字段名，全程 JIT 从真返回 keys 里读，避免 schema 漂移后失准。
#
# 用法：
#   bash scripts/probe.sh                 # 列 projects + groups（采样字段）
#   bash scripts/probe.sh --schema        # 输出 /api/projects /api/repository-groups 完整 schema 第一条
#   bash scripts/probe.sh --cache-miss <group_name|group_id>
#                                         # 强制重启 cdt-cli 再 curl 验证 skeleton → fill 时序
#   bash scripts/probe.sh --routes        # grep cdt-api routes.rs 列所有 axum route 与 fallback
#
# 输出可直接喂给 chrome-devtools mcp evaluate_script 用，或贴给 codex。

set -euo pipefail

BE=${BE:-http://127.0.0.1:3456}
PORT_BE=3456
MODE=${1:-list}

# REPO_ROOT 同 start.sh 逻辑：优先 git root，自动适配 worktree。
if [ -z "${REPO_ROOT:-}" ]; then
  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
  REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel 2>/dev/null || true)"
fi

require_be() {
  if ! curl -sf -o /dev/null "$BE/api/projects"; then
    echo "❌ cdt-cli not reachable at $BE — 先跑 scripts/start.sh" >&2
    exit 1
  fi
}

# 校验 :3456 真的被项目自己的 cdt binary 占（不是桌面 Tauri app）。
# codex PR 二审 issue 2：--cache-miss 没这个校验会假阳——桌面 app 占着 3456
# 时 pkill 不动它，cargo run bind 失败，但 curl 仍返 200（桌面 app 响应），
# 脚本误报"重启完成"，cache 也没真清掉。
check_port_owner_is_cdt() {
  local pids proc cmdline
  if ! pids=$(lsof -tiTCP:$PORT_BE -sTCP:LISTEN 2>/dev/null); then
    return 1  # 端口空
  fi
  local pid1
  pid1=$(echo "$pids" | head -1)
  proc=$(ps -p "$pid1" -o comm= 2>/dev/null | head -1)
  cmdline=$(ps -p "$pid1" -o args= 2>/dev/null | head -1)
  case "$proc" in
    *claude-devtools-tauri*|*claude-de*)
      return 2  # 桌面 app
      ;;
  esac
  if echo "$cmdline" | grep -qE 'target/(debug|release)/cdt( |$)'; then
    return 0  # 项目自己的 cdt
  fi
  return 3  # 陌生
}

cmd_list() {
  require_be
  echo "=== /api/projects (head 5) ==="
  curl -s "$BE/api/projects" | python3 -c "
import json,sys
d=json.load(sys.stdin)
print(f'total={len(d)}, keys={list(d[0].keys()) if d else []}')
for p in d[:5]:
    main = p.get('displayName') or p.get('name') or p.get('id','?')
    print(f'  {main}  sessions={p.get(\"sessionCount\",\"?\")}')
"
  echo ""
  echo "=== /api/repository-groups (head 5) ==="
  curl -s "$BE/api/repository-groups" | python3 -c "
import json,sys
d=json.load(sys.stdin)
print(f'total={len(d)}, keys={list(d[0].keys()) if d else []}')
for g in d[:5]:
    main = g.get('name') or (g.get('identity') or {}).get('name') or g.get('id','?')
    wt = g.get('worktrees') or []
    print(f'  {main}  totalSessions={g.get(\"totalSessions\",\"?\")} worktrees={len(wt)}')
    print(f'    id={g.get(\"id\",\"?\")[:80]}')
"
}

cmd_schema() {
  require_be
  for path in /api/projects /api/repository-groups; do
    echo "=== $path[0] ==="
    curl -s "$BE$path" | python3 -c "
import json,sys
d=json.load(sys.stdin)
print(json.dumps(d[0] if d else None, ensure_ascii=False, indent=2))
" | head -60
    echo ""
  done
}

cmd_routes() {
  local routes=crates/cdt-api/src/http/routes.rs
  [ -f "$routes" ] || { echo "❌ not found: $routes (cwd 不是 repo root？)" >&2; exit 1; }
  echo "=== axum routes in $routes ==="
  grep -nE '\.route\(' "$routes"
}

cmd_cache_miss() {
  local q="${2:-}"
  [ -n "$q" ] || { echo "usage: probe.sh --cache-miss <group_name|group_id>" >&2; exit 2; }
  require_be

  # 端口归属前置检查（codex PR 二审 issue 2）
  check_port_owner_is_cdt
  local owner_status=$?
  case $owner_status in
    0) ;;  # cdt 占 OK
    1) echo "❌ :$PORT_BE 没人 LISTEN — 先 start.sh" >&2; exit 1 ;;
    2) echo "❌ :$PORT_BE 被桌面 Tauri app 占 — 退出桌面 app 后重试" >&2; exit 2 ;;
    3) echo "❌ :$PORT_BE 被陌生进程占 — 不能安全 pkill，请人工处理" >&2; exit 2 ;;
  esac

  [ -n "${REPO_ROOT:-}" ] && [ -f "$REPO_ROOT/Cargo.toml" ] || {
    echo "❌ 无法定位 REPO_ROOT — 用 REPO_ROOT=<path> 显式指定" >&2; exit 1; }

  # 找 group id
  local group_id
  group_id=$(curl -s "$BE/api/repository-groups" | python3 -c "
import json,sys
q='$q'
d=json.load(sys.stdin)
for g in d:
    if q == g.get('name') or q == g.get('id') or q in (g.get('name') or ''):
        print(g['id']); break
")
  [ -n "$group_id" ] || { echo "❌ group 未匹配: $q" >&2; exit 1; }
  echo "→ matched group: $group_id"

  # 记录旧 cdt pid 用于"重启后真换了新进程"验证
  local old_pid
  old_pid=$(lsof -tiTCP:$PORT_BE -sTCP:LISTEN 2>/dev/null | head -1)

  echo "→ 重启 cdt-cli (REPO_ROOT=$REPO_ROOT) 清内存 cache..."
  pkill -f 'target/(debug|release)/cdt( |$)' 2>/dev/null || true
  sleep 1

  ( cd "$REPO_ROOT" && nohup cargo run -p cdt-cli > /tmp/cdt-cli.log 2>&1 & )

  # 等真的是**新** cdt 进程占 :3456，不只是 :3456 可达（防桌面 app 漏检）
  local i new_pid=""
  for i in {1..60}; do
    sleep 1
    new_pid=$(lsof -tiTCP:$PORT_BE -sTCP:LISTEN 2>/dev/null | head -1)
    [ -n "$new_pid" ] && [ "$new_pid" != "$old_pid" ] && {
      # double-check 是真 cdt 不是其它捷足者
      check_port_owner_is_cdt && break || true
    }
    new_pid=""
  done
  [ -n "$new_pid" ] || {
    echo "❌ cdt-cli 60s 内未真正重启占 :$PORT_BE (旧 pid=$old_pid)" >&2
    echo "--- /tmp/cdt-cli.log tail ---" >&2; tail -15 /tmp/cdt-cli.log >&2
    exit 1
  }
  echo "  ✓ cdt-cli 重启完成 (old=$old_pid new=$new_pid)"

  local enc
  enc=$(python3 -c "import urllib.parse;print(urllib.parse.quote('$group_id', safe=''))")

  echo ""
  echo "=== T0 立即 curl（应大量 skeleton title=null）==="
  curl -s "$BE/api/repository-groups/$enc/sessions?pageSize=50" | python3 -c "
import json,sys
d=json.load(sys.stdin); s=d.get('sessions',[])
print(f'total={len(s)} titled={sum(1 for x in s if x.get(\"title\"))} skeleton={sum(1 for x in s if not x.get(\"title\"))}')
"
  echo ""
  echo "=== T+6s 再 curl（scan 应跑完，title fill）==="
  sleep 6
  curl -s "$BE/api/repository-groups/$enc/sessions?pageSize=50" | python3 -c "
import json,sys
d=json.load(sys.stdin); s=d.get('sessions',[])
print(f'total={len(s)} titled={sum(1 for x in s if x.get(\"title\"))} skeleton={sum(1 for x in s if not x.get(\"title\"))}')
"
}

case "$MODE" in
  list|"")          cmd_list ;;
  --schema)         cmd_schema ;;
  --routes)         cmd_routes ;;
  --cache-miss)     cmd_cache_miss "$@" ;;
  -h|--help)        sed -n '1,15p' "$0" ;;
  *) echo "unknown mode: $MODE (-h for help)" >&2; exit 2 ;;
esac
