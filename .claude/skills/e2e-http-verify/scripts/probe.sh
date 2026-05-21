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
MODE=${1:-list}

require_be() {
  if ! curl -sf -o /dev/null "$BE/api/projects"; then
    echo "❌ cdt-cli not reachable at $BE — 先跑 scripts/start.sh" >&2
    exit 1
  fi
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

  # 重启 cdt-cli 强制 cache miss
  echo "→ 重启 cdt-cli 清内存 cache..."
  pkill -f 'target/(debug|release)/cdt( |$)' 2>/dev/null || true
  sleep 1
  ( cd "${REPO_ROOT:-$(pwd)}" && nohup cargo run -p cdt-cli > /tmp/cdt-cli.log 2>&1 & )
  until curl -sf -o /dev/null "$BE/api/projects" 2>/dev/null; do sleep 1; done
  echo "  ✓ cdt-cli 重启完成"

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
