#!/usr/bin/env bash
# 跑全 workspace divan bench 并输出合并的 customSmallerIsBetter JSON。
#
# 用法：
#   scripts/run-divan-bench.sh > bench-results.json
#   scripts/run-divan-bench.sh --crate cdt-parse > parse-only.json
#
# 输出到 stdout 的是合并后的 JSON 数组。编译/bench 本身的 stderr 被丢弃。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ONLY_CRATE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --crate) ONLY_CRATE="$2"; shift 2 ;;
    -h|--help)
      echo "用法: $0 [--crate <crate-name>]"
      echo "跑 divan bench 并输出 customSmallerIsBetter JSON"
      exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 1 ;;
  esac
done

BENCH_CRATES=(cdt-parse cdt-analyze cdt-discover cdt-fs cdt-api)

if [[ -n "$ONLY_CRATE" ]]; then
  BENCH_CRATES=("$ONLY_CRATE")
fi

TMPDIR_BASE="${TMPDIR:-/tmp}/divan-bench-$$"
mkdir -p "$TMPDIR_BASE"
trap 'rm -rf "$TMPDIR_BASE"' EXIT

for crate in "${BENCH_CRATES[@]}"; do
  cargo bench -p "$crate" 2>/dev/null \
    | "$SCRIPT_DIR/divan-to-json.sh" "$crate" \
    > "$TMPDIR_BASE/$crate.json"
done

# 合并所有 crate 的 JSON 数组为一个数组
# 用 jq 如果可用；否则简单文本合并
if command -v jq >/dev/null 2>&1; then
  jq -s 'add' "$TMPDIR_BASE"/*.json
else
  echo "["
  first=1
  for f in "$TMPDIR_BASE"/*.json; do
    # 去掉外层 [ ] 和首尾空行
    content=$(sed '1d;$d' "$f")
    if [[ -n "$content" ]]; then
      if [[ $first -eq 0 ]]; then
        echo ","
      fi
      echo "$content"
      first=0
    fi
  done
  echo "]"
fi
