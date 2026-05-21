#!/usr/bin/env bash
# 起 cdt-cli (:3456) + vite (:5173) 跑 e2e 真后端验证。
#
# 关键不变量：
# - 桌面 Tauri app (process name = claude-devtools-tauri) 占 :3456 时不 kill
#   用户进程；陌生进程占用 → pkill 后继续
# - cdt-cli stdout 不 redirect 会阻塞挂起，必须 > LOG_BE
# - ready 检查走 HTTP 200 轮询而非固定 sleep，incremental build 通常 5s 内 OK
# - vite → cdt-cli proxy 同步验过 /api/projects 200 才报 Ready；否则 SSE
#   prelude 缓冲坑没暴露

set -euo pipefail

REPO_ROOT="${REPO_ROOT:-/Users/zhaohejie/RustroverProjects/Project/claude-devtools-rs}"
PORT_BE=3456
PORT_FE=5173
LOG_BE=/tmp/cdt-cli.log
LOG_FE=/tmp/vite-dev.log

check_port() {
  local port=$1
  local pids proc
  if ! pids=$(lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null); then
    return 0
  fi
  proc=$(ps -p "$(echo "$pids" | head -1)" -o comm= 2>/dev/null | head -1)
  case "$proc" in
    *claude-devtools-tauri*|*claude-de*)
      echo "❌ :$port 被桌面 Tauri app ($proc, pid=$pids) 占用" >&2
      echo "   退出桌面 app 后重跑，或临时改 config.http_server.port" >&2
      return 2
      ;;
    *)
      echo "⚠️  :$port 被 $proc (pid=$pids) 占用，pkill 后继续"
      kill -9 $pids 2>/dev/null || true
      sleep 0.5
      ;;
  esac
}

wait_http_200() {
  local url=$1 wait=$2 label=$3
  local i code=000
  for ((i=1; i<=wait; i++)); do
    code=$(curl -s -o /dev/null -w '%{http_code}' "$url" 2>/dev/null || echo 000)
    if [ "$code" = "200" ]; then
      echo "  ✓ $label ready (${i}s)"
      return 0
    fi
    sleep 1
  done
  echo "❌ $label ${wait}s 内未就绪 (last http code: $code)" >&2
  return 1
}

check_port $PORT_BE || exit $?
check_port $PORT_FE

echo "→ 起 cdt-cli on :$PORT_BE (log: $LOG_BE)"
( cd "$REPO_ROOT" && nohup cargo run -p cdt-cli > "$LOG_BE" 2>&1 & )
if ! wait_http_200 "http://127.0.0.1:$PORT_BE/api/projects" 90 cdt-cli; then
  echo "--- $LOG_BE tail ---" >&2
  tail -15 "$LOG_BE" >&2
  exit 1
fi

echo "→ 起 vite on :$PORT_FE (log: $LOG_FE)"
( nohup pnpm --dir "$REPO_ROOT/ui" run dev > "$LOG_FE" 2>&1 & )
if ! wait_http_200 "http://127.0.0.1:$PORT_FE/" 20 vite; then
  echo "--- $LOG_FE tail ---" >&2
  tail -15 "$LOG_FE" >&2
  exit 1
fi

echo "→ 验证 vite proxy → cdt-cli"
code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$PORT_FE/api/projects" || echo 000)
if [ "$code" != "200" ]; then
  echo "❌ proxy 不通 (code=$code) — 检查 ui/vite.config.ts::server.proxy" >&2
  exit 1
fi
echo "  ✓ proxy /api → :$PORT_BE OK"

cat <<EOF

✓ Ready
  浏览器入口: http://127.0.0.1:$PORT_FE/?http=1
  cdt-cli log: $LOG_BE
  vite log:    $LOG_FE
  收尾:        bash $(dirname "$0")/stop.sh
EOF
