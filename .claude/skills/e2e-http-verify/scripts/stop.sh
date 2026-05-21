#!/usr/bin/env bash
# 收尾 start.sh 起的 cdt-cli + vite。不动桌面 Tauri app。

set -uo pipefail

PORT_BE=3456
PORT_FE=5173

# binary name 是 cdt（见 crates/cdt-cli/Cargo.toml::[[bin]]）。pattern 用
# 'target/(debug|release)/cdt ' 末尾留空格收紧，避免误匹配 cdt-cli artifact
# string——cdt-cli 是 lib crate 不产可执行，但 pkill -f 按 cmdline 全字符串
# 匹配，留余地更稳。
pkill -f 'target/(debug|release)/cdt( |$)' 2>/dev/null || true
pkill -f 'pnpm.*vite|node.*vite' 2>/dev/null || true
sleep 0.5

for port in $PORT_BE $PORT_FE; do
  if pids=$(lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null); then
    proc=$(ps -p "$(echo "$pids" | head -1)" -o comm= 2>/dev/null | head -1)
    case "$proc" in
      *claude-devtools-tauri*|*claude-de*)
        echo "ℹ️  :$port 仍由桌面 Tauri app ($proc) 占 — 不动"
        ;;
      *)
        echo "⚠️  :$port 仍被 $proc (pid=$pids) 占 — 可能需手动 kill"
        ;;
    esac
  fi
done

echo "✓ stopped"
