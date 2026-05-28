#!/usr/bin/env bash
# 跑全 workspace divan bench 并输出合并的 customSmallerIsBetter JSON。
#
# 用法：
#   scripts/run-divan-bench.sh > bench-results.json
#   scripts/run-divan-bench.sh --crate cdt-parse > parse-only.json
#
# 输出到 stdout 的是合并后的 JSON 数组。编译/bench 本身的 stderr 被丢弃。
# 退出码 1 表示没有产出任何 bench 结果。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ONLY_CRATE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --crate)
      if [[ $# -lt 2 ]]; then
        echo "error: --crate requires a value" >&2
        exit 1
      fi
      ONLY_CRATE="$2"; shift 2 ;;
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
if command -v jq >/dev/null 2>&1; then
  result=$(jq -s 'add // []' "$TMPDIR_BASE"/*.json)
else
  result="["
  first=1
  for f in "$TMPDIR_BASE"/*.json; do
    content=$(sed '1d;$d' "$f")
    if [[ -n "$content" ]]; then
      if [[ $first -eq 0 ]]; then
        result="$result,"
      fi
      result="$result
$content"
      first=0
    fi
  done
  result="$result
]"
fi

echo "$result"

# 校验非空
count=$(echo "$result" | grep -c '"name"' || true)
if [[ "$count" -eq 0 ]]; then
  echo "error: no benchmark results produced" >&2
  exit 1
fi
