#!/usr/bin/env bash
# scripts/check-no-hot-event.sh
#
# CI 守门：禁止 cdt_telemetry::event!() 出现在 hot path 文件下。
#
# 详 OpenSpec change `add-telemetry-signal-bus` D9 + spec
# `application-telemetry/spec.md::hot path 性能契约`：Event::push 单次 ~100-200 ns，
# 远高于 Counter (~5 ns) / Histogram (~30-50 ns)；hot path 误用会撞穿 < 0.2%
# wall time 增量预算。
#
# 命中即 PR fail。需要在 hot 文件加事件信号 → 改用 counter / histogram，
# 或迁出 hot path（确认事件真的低频）。

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

# Hot path 文件清单（绝对路径相对仓库根；保持 sorted alphabetical）
HOT_PATH_FILES=(
  "crates/cdt-analyze/src/"
  "crates/cdt-api/src/ipc/local.rs"
  "crates/cdt-api/src/ipc/session_metadata.rs"
  "crates/cdt-discover/src/"
  "crates/cdt-parse/src/"
)

# 模式匹配：cdt_telemetry::event! 调用 / use 后裸 event!
# 允许行：1) 注释里的字面量讨论 2) 匹配 ".event!" 或 "_event!" 等不是 macro 调用
EVENT_RE='(^|[^A-Za-z0-9_.])(cdt_telemetry::)?event!\('

violations=0
for path in "${HOT_PATH_FILES[@]}"; do
  if [[ ! -e "$path" ]]; then
    continue
  fi
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    # 排除注释中的提示（行首到 // 之间不含 event!() 才算违例）
    file_path="${line%%:*}"
    rest="${line#*:}"
    line_num="${rest%%:*}"
    code="${rest#*:}"
    # 去掉 // 后的内容再检
    code_no_comment="${code%%//*}"
    if [[ "$code_no_comment" =~ $EVENT_RE ]]; then
      echo "ERROR: hot-path file $file_path:$line_num contains event!() call:"
      echo "  $code"
      violations=$((violations + 1))
    fi
  done < <(grep -RHnE "$EVENT_RE" "$path" 2>/dev/null || true)
done

if [[ $violations -gt 0 ]]; then
  echo
  echo "❌ Found $violations event!() call(s) in hot-path files."
  echo "   See OpenSpec change add-telemetry-signal-bus D9."
  exit 1
fi

echo "✓ No event!() in hot-path files."
exit 0
