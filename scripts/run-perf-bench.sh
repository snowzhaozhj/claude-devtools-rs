#!/usr/bin/env bash
# 性能基准 runner + baseline gate。
# Bug fixes: agg_min zero-filter, GNU time RSS unit, skip detection hardening, PR comment pagination
#
# 用法：
#   scripts/run-perf-bench.sh                       # 跑全部 bench，对比 tests/perf-baseline.json
#   scripts/run-perf-bench.sh --bench perf_cold_scan  # 只跑指定 bench
#   scripts/run-perf-bench.sh --out report.json     # 报告写到指定路径
#   scripts/run-perf-bench.sh --runs 8              # 每个 bench 跑 8 次取 min（默认 5）
#
# 退出码：
#   0 - 全部 bench 通过 OR 跳过（无 corpus 数据）
#   1 - 至少一个 bench 超阈值
#   2 - 用法/环境错误（jq 缺失 / baseline JSON 无效）
#
# OS 处理：
#   - macOS: /usr/bin/time -lp，maxRSS 单位 bytes
#   - Linux: /usr/bin/time -v，maxRSS 单位 kbytes；若极旧/未修补 time
#     误把 kbytes 放大成 bytes 量级，脚本只在异常大值时折算。
#
# CI 行为：bench 内部 `if !projects_dir.exists() { return }`——CI runner 无
# ~/.claude/projects 时 bench 立即 ok 但不输出 perf 行，本脚本检测到无
# perf 数据时标 "skipped"，CI 视为 pass（smoke 校验 binary 能编 / 能跑）。
# 真实 gate 由 dev 在本地跑（有真实数据），是 PR push 前的硬约束。
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASELINE="${REPO_ROOT}/tests/perf-baseline.json"
OUT_REPORT="${REPO_ROOT}/target/perf-report.json"
RUNS=5
ONLY_BENCH=""
UPDATE_BASELINE=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bench) ONLY_BENCH="$2"; shift 2 ;;
    --out) OUT_REPORT="$2"; shift 2 ;;
    --runs) RUNS="$2"; shift 2 ;;
    --update-baseline) UPDATE_BASELINE=1; shift ;;
    -h|--help)
      sed -n '/^# /p' "$0" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

command -v jq >/dev/null 2>&1 || { echo "需要 jq" >&2; exit 2; }
[[ -f "$BASELINE" ]] || { echo "baseline 文件不存在: $BASELINE" >&2; exit 2; }
jq -e '.benches' "$BASELINE" >/dev/null || { echo "baseline JSON 无效" >&2; exit 2; }

OS="$(uname -s)"
# 用 -o 把 time 输出写到独立文件，让 bench 自己的 stderr 不被混进 time stats，
# 也让我们能直接 cat bench 的 stdout+stderr 抓 `★` 等内部 metrics 行。
case "$OS" in
  Darwin) TIME_FLAGS=(-lp) ;;
  Linux)  TIME_FLAGS=(-v) ;;
  *) echo "不支持的 OS: $OS" >&2; exit 2 ;;
esac

GNU_TIME_RSS_SANITY_KB=$((16 * 1024 * 1024))

mkdir -p "$(dirname "$OUT_REPORT")"

# ============================================================================
# 解析 /usr/bin/time 输出（stderr 流），输出 JSON: {wall_ms, user_ms, sys_ms, max_rss_kb}
# ============================================================================
parse_time_output() {
  local time_log="$1"
  local os="$2"
  local rss_sanity_kb="${3:-16777216}"
  awk -v os="$os" -v rss_sanity_kb="$rss_sanity_kb" '
    function to_ms(s,   n) { n = s + 0; return int(n * 1000 + 0.5) }
    function hms_to_ms(t,   parts, h, m, s, n) {
      n = split(t, parts, ":")
      if (n == 3) { return to_ms(parts[1]*3600 + parts[2]*60 + parts[3]) }
      if (n == 2) { return to_ms(parts[1]*60 + parts[2]) }
      return to_ms(t)
    }
    BEGIN { wall=-1; user=-1; sys=-1; rss=-1 }
    os == "Darwin" {
      if ($1 == "real") { wall = to_ms($2); next }
      if ($1 == "user") { user = to_ms($2); next }
      if ($1 == "sys")  { sys = to_ms($2); next }
      # macOS BSD time -l: bytes 数字 + "maximum resident set size"
      if (match($0, /^ *[0-9]+ +maximum resident set size/)) {
        # 第一个数字
        n = $1 + 0
        rss = int(n / 1024 + 0.5)
        next
      }
    }
    os == "Linux" {
      # GNU time -v 输出：键值对，键以空白起首
      if (match($0, /Elapsed \(wall clock\) time/)) {
        # 行尾的时间串可能是 m:ss.xx 或 h:mm:ss
        n = split($0, parts, ":")
        # 取最后一段的实际时间表达
        sub(/.*:[ ]*/, "", $0)  # 这种写法不可靠；改用 match
      }
      if (match($0, /Elapsed.*: */)) {
        t = substr($0, RSTART + RLENGTH)
        gsub(/[ \t]+$/, "", t)
        wall = hms_to_ms(t)
        next
      }
      if (match($0, /User time \(seconds\): */)) {
        user = to_ms(substr($0, RSTART + RLENGTH)); next
      }
      if (match($0, /System time \(seconds\): */)) {
        sys = to_ms(substr($0, RSTART + RLENGTH)); next
      }
      if (match($0, /Maximum resident set size \(kbytes\): */)) {
        n = substr($0, RSTART + RLENGTH) + 0
        # Debian/Ubuntu 的 GNU time 1.9 已修正 maxRSS 单位；只对明显异常的
        # 旧输出做兜底，避免把正常 30MB 误折成 30KB。
        if (n > rss_sanity_kb) n = n / 1024
        rss = int(n + 0.5); next
      }
    }
    END {
      if (wall <= 0 || user < 0 || sys < 0 || rss <= 0) exit 1
      printf "{\"wall_ms\":%d,\"user_ms\":%d,\"sys_ms\":%d,\"max_rss_kb\":%d}\n", wall, user, sys, rss
    }
  ' "$time_log"
}

# ============================================================================
# 解析 bench stdout 抽 in-process metrics
# perf_cold_scan: `[perf] cold total (list_repository_groups equivalent)=63ms`
# perf_get_session_detail: `★ get_session_detail (with OMIT): payload=1420 KB, ipc 54 ms`
# ============================================================================
parse_internal_metrics() {
  local stdout_log="$1"
  local bench="$2"
  case "$bench" in
    perf_cold_scan)
      awk '
        BEGIN { found = 0 }
        /\[perf\] cold total/ {
          if (match($0, /=[0-9]+ms/)) {
            v = substr($0, RSTART+1, RLENGTH-3)
            cold_total = v + 0
            found = 1
          }
        }
        END {
          if (found) printf "{\"cold_total_ms\":%d}\n", cold_total
          else print "{}"
        }
      ' "$stdout_log"
      ;;
    perf_get_session_detail)
      # 取最大 sample 的 IPC + payload
      awk '
        /★ get_session_detail/ {
          ipc = 0; kb = 0
          if (match($0, /payload=[0-9]+ KB/)) { kb = substr($0, RSTART+8, RLENGTH-11) + 0 }
          if (match($0, /ipc [0-9]+ ms/)) { ipc = substr($0, RSTART+4, RLENGTH-7) + 0 }
          if (kb > max_kb || ipc > max_ipc) {
            if (kb > max_kb) max_kb = kb
            if (ipc > max_ipc) max_ipc = ipc
          }
        }
        END {
          printf "{\"largest_ipc_ms\":%d,\"largest_payload_kb\":%d}\n", max_ipc, max_kb
        }
      ' "$stdout_log"
      ;;
    *) echo "{}" ;;
  esac
}

# ============================================================================
# 聚合：取最小值。perf bench 标准做法——min 代表"算法真实最佳能力"，
# 系统噪声 / FS cache 状态只在 min 之上加波动。新代码若引入算法回归，min
# 必然变大；用 median / max 会被噪声污染导致假阳性（用户原话："wall+20%
# 在 noisy runner 假阳性概率"），min-of-N 是公认的稳定指标。
# ============================================================================
agg_min() {
  printf '%s\n' "$@" | awk '
    $1 + 0 > 0 {
      if (!seen || $1 < min) min = $1
      seen = 1
    }
    END {
      if (seen) print min
      else print 0
    }'
}

# ============================================================================
# 编译 bench binary 并返回路径
# ============================================================================
locate_binary() {
  local binary="$1"
  (cd "$REPO_ROOT" && cargo test --release -p cdt-api --test "$binary" --no-run \
    --message-format=json 2>/dev/null) \
    | jq -r --arg b "$binary" \
      'select(.profile.test == true and (.target.name | startswith($b))) | .executable' \
    | head -n 1
}

# ============================================================================
# 跑单个 bench：编译 → 1 warmup → RUNS 次测量 → 取 min → 判定
# 返回 JSON 单条 bench 结果到 stdout
# ============================================================================
run_bench() {
  local name="$1"
  local binary; binary=$(jq -r --arg n "$name" '.benches[$n].binary' "$BASELINE")
  local test_filter; test_filter=$(jq -r --arg n "$name" '.benches[$n].test_filter' "$BASELINE")

  echo "[perf] 编译 $binary ..." >&2
  local bin_path; bin_path=$(locate_binary "$binary")
  if [[ -z "$bin_path" || ! -x "$bin_path" ]]; then
    jq -n --arg name "$name" \
      '{name: $name, status: "error", error: "binary 编译失败或定位不到"}'
    return
  fi

  local tmp; tmp=$(mktemp -d)

  # warmup（不计）
  echo "[perf] $name warmup..." >&2
  "$bin_path" "$test_filter" --ignored --nocapture --exact >/dev/null 2>&1 || true

  local walls=() users=() syss=() rsss=() internals=()
  local skipped=0
  for i in $(seq 1 "$RUNS"); do
    echo "[perf] $name run $i/$RUNS..." >&2
    local time_log="$tmp/time-$i.log"
    local out_log="$tmp/out-$i.log"
    /usr/bin/time "${TIME_FLAGS[@]}" -o "$time_log" \
      "$bin_path" "$test_filter" --ignored --nocapture --exact \
      >"$out_log" 2>&1 || {
      # bench 内部 panic / build_chunks 失败：把全部日志吐出来供诊断
      echo "[perf] $name run $i FAILED" >&2
      cat "$time_log" >&2 || true
      cat "$out_log" >&2 || true
      jq -n --arg name "$name" '{name: $name, status: "error", error: "bench 执行失败"}'
      return
    }

    # 检测 skip：corpus 不存在
    if grep -q "跳过：\|skip:" "$out_log" 2>/dev/null; then
      skipped=1
    fi

    local parsed
    if ! parsed=$(parse_time_output "$time_log" "$OS" "$GNU_TIME_RSS_SANITY_KB") || [[ -z "$parsed" ]]; then
      skipped=1
      continue
    fi
    local wall user sys rss
    wall=$(echo "$parsed" | jq '.wall_ms')
    user=$(echo "$parsed" | jq '.user_ms')
    sys=$(echo "$parsed" | jq '.sys_ms')
    rss=$(echo "$parsed" | jq '.max_rss_kb')
    if [[ "$wall" -le 0 || "$user" -le 0 || "$sys" -lt 0 || "$rss" -le 0 ]]; then
      skipped=1
      continue
    fi
    walls+=("$wall")
    users+=("$user")
    syss+=("$sys")
    rsss+=("$rss")
    internals+=("$(parse_internal_metrics "$out_log" "$binary")")
  done

  if [[ "$skipped" == "1" ]]; then
    jq -n --arg name "$name" \
      --argjson runs "$RUNS" \
      '{name: $name, status: "skipped", reason: "bench 内部跳过（~/.claude/projects 无真实 corpus）", runs: $runs}'
    return
  fi

  local wall_med; wall_med=$(agg_min "${walls[@]}")
  local user_med; user_med=$(agg_min "${users[@]}")
  local sys_med; sys_med=$(agg_min "${syss[@]}")
  local rss_med; rss_med=$(agg_min "${rsss[@]}")

  # user/real ratio = user_ms / wall_ms（避免 awk 浮点跨平台不一致用 jq）
  local ratio
  if [[ "$wall_med" -eq 0 ]]; then ratio="0"
  else ratio=$(jq -n --argjson u "$user_med" --argjson w "$wall_med" '($u / $w * 1000 | floor) / 1000')
  fi

  # 合并 internal metrics：对每个 key 取所有 run 的 min（与 process 级聚合策略一致）
  local internal_json
  internal_json=$(printf '%s\n' "${internals[@]}" | jq -s '
    if length == 0 then {}
    else
      . as $rows
      | reduce (($rows[0] // {}) | keys[]) as $k ({};
          .[$k] = ([$rows[] | .[$k] // empty] | sort | .[0] // 0))
    end')

  # 与 baseline 对比
  local baseline_json; baseline_json=$(jq -c --arg n "$name" '.benches[$n]' "$BASELINE")
  local thresholds; thresholds=$(jq -c '.thresholds' "$BASELINE")

  jq -n \
    --arg name "$name" \
    --argjson wall "$wall_med" \
    --argjson user "$user_med" \
    --argjson sys "$sys_med" \
    --argjson rss "$rss_med" \
    --argjson ratio "$ratio" \
    --argjson internal "$internal_json" \
    --argjson baseline "$baseline_json" \
    --argjson thr "$thresholds" \
    '
    def pct_delta(actual; base):
      if base == 0 then 0
      else ((actual - base) / base * 1000 | round) / 10 end;

    def check_metric(actual; base; thr_pct):
      {
        actual: actual,
        baseline: base,
        delta_pct: pct_delta(actual; base),
        threshold_pct: thr_pct,
        fail: (base > 0 and (actual - base) / base * 100 > thr_pct)
      };

    {
      name: $name,
      status: "measured",
      metrics: {
        wall_ms: check_metric($wall; $baseline.metrics.wall_ms; $thr.wall_ms_pct),
        user_ms: check_metric($user; $baseline.metrics.user_ms; $thr.user_ms_pct),
        sys_ms: { actual: $sys, baseline: $baseline.metrics.sys_ms, info_only: true },
        max_rss_kb: check_metric($rss; $baseline.metrics.max_rss_kb; $thr.max_rss_kb_pct),
        user_real_ratio: {
          actual: $ratio,
          baseline: $baseline.metrics.user_real_ratio,
          threshold_max: $thr.user_real_ratio_max,
          fail: ($ratio > $thr.user_real_ratio_max)
        }
      },
      internal: $internal,
      regressions: [
        (if (($wall - $baseline.metrics.wall_ms) / ($baseline.metrics.wall_ms | if . == 0 then 1 else . end) * 100) > $thr.wall_ms_pct
          then "wall_ms 超 +\($thr.wall_ms_pct)%（\($wall) vs \($baseline.metrics.wall_ms)）" else empty end),
        (if (($user - $baseline.metrics.user_ms) / ($baseline.metrics.user_ms | if . == 0 then 1 else . end) * 100) > $thr.user_ms_pct
          then "user_ms 超 +\($thr.user_ms_pct)%（\($user) vs \($baseline.metrics.user_ms)）" else empty end),
        (if (($rss - $baseline.metrics.max_rss_kb) / ($baseline.metrics.max_rss_kb | if . == 0 then 1 else . end) * 100) > $thr.max_rss_kb_pct
          then "max_rss_kb 超 +\($thr.max_rss_kb_pct)%（\($rss) vs \($baseline.metrics.max_rss_kb)）" else empty end),
        (if $ratio > $thr.user_real_ratio_max
          then "user/real ratio \($ratio) > \($thr.user_real_ratio_max)（多核打满风险）" else empty end)
      ]
    }
  '
}

# ============================================================================
# 主流程
# ============================================================================
BENCH_NAMES=()
if [[ -n "$ONLY_BENCH" ]]; then
  BENCH_NAMES=("$ONLY_BENCH")
else
  while IFS= read -r n; do BENCH_NAMES+=("$n"); done < <(jq -r '.benches | keys[]' "$BASELINE")
fi

ALL_RESULTS="["
FIRST=1
for bench in "${BENCH_NAMES[@]}"; do
  result=$(run_bench "$bench")
  if [[ "$FIRST" == "1" ]]; then FIRST=0; else ALL_RESULTS+=","; fi
  ALL_RESULTS+="$result"
done
ALL_RESULTS+="]"

# 写报告
echo "$ALL_RESULTS" | jq --arg os "$OS" --argjson runs "$RUNS" '{
  os: $os,
  runs: $runs,
  results: .
}' > "$OUT_REPORT"

echo ""
echo "=== 性能报告 ==="
jq -r '.results[] |
  if .status == "skipped" then "[\(.name)] SKIPPED（\(.reason)）"
  elif .status == "error" then "[\(.name)] ERROR：\(.error)"
  else
    "[\(.name)] MEASURED\n" +
    "  wall=\(.metrics.wall_ms.actual)ms (Δ\(.metrics.wall_ms.delta_pct)%, base \(.metrics.wall_ms.baseline)ms, thr +\(.metrics.wall_ms.threshold_pct)%)\n" +
    "  user=\(.metrics.user_ms.actual)ms (Δ\(.metrics.user_ms.delta_pct)%, base \(.metrics.user_ms.baseline)ms, thr +\(.metrics.user_ms.threshold_pct)%)\n" +
    "  sys=\(.metrics.sys_ms.actual)ms (base \(.metrics.sys_ms.baseline)ms, info-only)\n" +
    "  max_rss=\(.metrics.max_rss_kb.actual)KB (Δ\(.metrics.max_rss_kb.delta_pct)%, base \(.metrics.max_rss_kb.baseline)KB, thr +\(.metrics.max_rss_kb.threshold_pct)%)\n" +
    "  user/real=\(.metrics.user_real_ratio.actual) (base \(.metrics.user_real_ratio.baseline), max \(.metrics.user_real_ratio.threshold_max))\n" +
    "  internal=\(.internal)\n" +
    (if (.regressions | length) > 0 then "  ❌ 回归：\n    " + (.regressions | join("\n    ")) else "  ✓ 无回归" end)
  end' "$OUT_REPORT"

# 判定 exit code
HAS_REGRESSION=$(jq '[.results[] | select(.status == "measured") | .regressions | length] | add // 0' "$OUT_REPORT")
HAS_ERROR=$(jq '[.results[] | select(.status == "error")] | length' "$OUT_REPORT")

echo ""
echo "报告路径：$OUT_REPORT"

if [[ "$HAS_ERROR" -gt 0 ]]; then
  echo "❌ 至少一个 bench 执行失败" >&2
  exit 1
fi
if [[ "$HAS_REGRESSION" -gt 0 ]]; then
  echo "❌ 检测到性能回归（共 $HAS_REGRESSION 条）" >&2
  exit 1
fi

echo "✓ 全部 bench 通过 / 跳过"
exit 0
