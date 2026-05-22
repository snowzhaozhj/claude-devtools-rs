#!/usr/bin/env bash
# `just dev` 启动前的 5173 端口预检 + 一键清理。
#
# 背景：`ui/vite.config.ts` 顶部注释解释了 `host: '127.0.0.1' + strictPort: true`
# 是故意的——vite 默认 fallback 到 `[::1]:5174` 时 macOS WKWebView 解析
# `localhost` 优先 IPv4，会让 Tauri webview 拿到外部进程的 IPv4:5173 响应（或
# connection refused）→ 白屏且无可见错误。所以"端口冲突自动 fallback"是反
# 模式，正确做法是立即报错暴露问题。本脚本只是把 vite 那一行红字升级为"哪
# 个进程占着 + 怎么处理"的明确提示。
#
# 子命令：
#   check  端口空闲 exit 0；被占 exit 1 + 列占用进程 + 提示如何清理
#   kill   杀掉所有占用 5173 的进程；空闲也 exit 0（幂等）
#
# 退出码：
#   0 端口空闲 / kill 成功
#   1 端口被占 (check) / lsof 不可用且需要它 (kill)
#
# Windows / 无 lsof 环境：check 静默 exit 0（降级回原 `cargo tauri dev` 行为）；
# kill 报错 exit 1（无法定位进程）。

set -euo pipefail

PORT=5173

cmd="${1:-check}"

if ! command -v lsof >/dev/null 2>&1; then
  case "$cmd" in
    check) exit 0 ;;
    kill)  echo "lsof 不可用——请手动定位 :$PORT 占用进程" >&2; exit 1 ;;
    *)     echo "用法：$0 {check|kill}" >&2; exit 1 ;;
  esac
fi

# 选出"真能阻塞 vite bind 127.0.0.1:$PORT"的 IPv4 LISTEN：
#   - 127.0.0.1:PORT       直接冲突
#   - 0.0.0.0:PORT / *:PORT  IPv4 wildcard 也阻塞 127.0.0.1 bind
#   排除 IPv6 ::1（vite host=127.0.0.1 不冲突）+ 排除 outbound 远端连接（-sTCP:LISTEN）+
#   排除非 5173 IPv4 LISTEN（理论上 -i4TCP 已限定，awk 二次确认 hostname:port 字面）
# 用 `-F pn` 输出机器可解析格式（p<pid> / n<host:port>），awk 配对解析。
pids=$(
  lsof -nP -a -i4TCP:"$PORT" -sTCP:LISTEN -F pn 2>/dev/null |
    awk -v port="$PORT" '
      /^p/ { pid = substr($0, 2) }
      /^n/ {
        name = substr($0, 2)
        if (name ~ ("(^|[[:space:]])(127\\.0\\.0\\.1|0\\.0\\.0\\.0|\\*):" port "([[:space:]]|$)")) {
          print pid
        }
      }
    ' |
    sort -u || true
)

case "$cmd" in
  check)
    if [ -z "$pids" ]; then
      exit 0
    fi
    echo "❌ :$PORT 已被占用："
    # `head` 提前关 pipe 在占用进程多于 5 行时 SIGPIPE → pipefail 下整管道
    # 非零 → set -e 中止后续 echo / exit 1 行；尾巴加 `|| true` 兜底。
    lsof -nP -a -i4TCP:"$PORT" -sTCP:LISTEN 2>/dev/null | head -5 || true
    echo ""
    echo "→ 跑 \`just dev-kill-port\` 一键清理（通常是上次 dev 没退干净）"
    echo "→ 或手动 \`kill -9 $pids\`"
    exit 1
    ;;
  kill)
    if [ -z "$pids" ]; then
      echo "✓ :$PORT 未被占用"
      exit 0
    fi
    echo "kill -9 $pids"
    # shellcheck disable=SC2086
    kill -9 $pids 2>/dev/null || true
    echo "✓ 已清理"
    exit 0
    ;;
  *)
    echo "用法：$0 {check|kill}" >&2
    exit 1
    ;;
esac
