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

# REPO_ROOT 优先用 git root（自动适配 worktree——历史上写死主 repo 路径会让
# worktree 里跑 start.sh 启动**主 repo** 的代码而非 PR 分支，e2e 验证错版本，
# codex PR 二审 issue 1）。env 变量优先，其次脚本所在 git root。
if [ -z "${REPO_ROOT:-}" ]; then
  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
  REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel 2>/dev/null || true)"
fi
if [ -z "${REPO_ROOT:-}" ] || [ ! -f "$REPO_ROOT/Cargo.toml" ]; then
  echo "❌ 无法确定 REPO_ROOT（试过 git rev-parse），用 REPO_ROOT=<path> 显式指定" >&2
  exit 1
fi

PORT_BE=3456
PORT_FE=5173
LOG_BE=/tmp/cdt-cli.log
LOG_FE=/tmp/vite-dev.log

# 端口归属判定：桌面 app → 拒绝；项目自己的 cdt/vite → kill；陌生 → 提示用户。
# 拒绝自动 kill 陌生进程是 codex PR 二审 issue 3——`:5173` 上的任何 node 都被
# kill -9 安全边界过宽（开发者可能有别的 vite/dev server 占着）。
check_port() {
  local port=$1
  local pids proc cmdline
  if ! pids=$(lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null); then
    return 0
  fi
  local pid1
  pid1=$(echo "$pids" | head -1)
  proc=$(ps -p "$pid1" -o comm= 2>/dev/null | head -1)
  cmdline=$(ps -p "$pid1" -o args= 2>/dev/null | head -1)
  case "$proc" in
    *claude-devtools-tauri*|*claude-de*)
      echo "❌ :$port 被桌面 Tauri app ($proc, pid=$pids) 占用" >&2
      echo "   退出桌面 app 后重跑，或临时改 config.http_server.port" >&2
      return 2
      ;;
  esac
  # 项目自己的 cdt binary 或 vite/pnpm dev server—— 可安全清理
  if echo "$cmdline" | grep -qE 'target/(debug|release)/cdt( |$)|pnpm.*vite|node.*vite'; then
    echo "⚠️  :$port 被项目自己的进程 ($proc, pid=$pids) 占用，pkill 后继续"
    kill -9 $pids 2>/dev/null || true
    sleep 0.5
    return 0
  fi
  # 陌生进程：拒绝自动 kill，让用户决定
  echo "❌ :$port 被陌生进程 $proc (pid=$pids) 占用" >&2
  echo "   cmdline: $cmdline" >&2
  echo "   不属于项目自动清理范围；请人工 kill 或换端口" >&2
  return 2
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
( cd "$REPO_ROOT" && nohup cargo run -p cdt-cli -- serve > "$LOG_BE" 2>&1 & )
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
