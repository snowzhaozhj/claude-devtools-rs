#!/usr/bin/env bash
# 自动抓 idle CPU 诊断数据：sample + top + 栈分类
# 用法：bash sample-cpu.sh <PID> [duration_seconds]
# 输出：$CLAUDE_JOB_DIR/cdt-perf-<HHMMSS>/{sample.txt,top.txt,thread-types.txt,active-frames.txt}
#
# 给 .claude/skills/perf 的 idle 诊断子模式用——避免每次手敲 sample/top/grep 命令。
# 仅 macOS（用 `sample` 工具）。Linux 用 `perf record / perf report` 替代。

set -euo pipefail

PID="${1:?usage: $0 <PID> [duration_seconds]}"
DUR="${2:-30}"

if ! ps -p "$PID" > /dev/null 2>&1; then
    echo "ERROR: PID $PID not running" >&2
    exit 1
fi

OUT_BASE="${CLAUDE_JOB_DIR:-/tmp}/cdt-perf-$(date +%H%M%S)"
mkdir -p "$OUT_BASE"

echo "== sample $PID for ${DUR}s → $OUT_BASE/ =="
sample "$PID" "$DUR" -file "$OUT_BASE/sample.txt" 2>&1 | tail -3

# top 趋势：5 次采样间隔 2 秒
top -l 5 -s 2 -pid "$PID" -stats pid,cpu,cpu_me,th,csw,pageins > "$OUT_BASE/top.txt" 2>&1

# 线程类型统计（看 tokio worker 数 / blocking pool 大小）
grep -E "^\s+[0-9]+ Thread" "$OUT_BASE/sample.txt" | \
    awk -F': ' '{print $2}' | \
    sort | uniq -c | sort -rn > "$OUT_BASE/thread-types.txt"

# 活跃栈（去 idle wait）— 找真热点
grep -E "^\s+\+ +[0-9]+ " "$OUT_BASE/sample.txt" | \
    grep -vE "kevent|condvar|wait_timeout|psynch_cvwait|mach_msg|nanosleep|RunLoop" | \
    awk '{count[$0]++} END {for(k in count) print count[k], k}' | \
    sort -rn | head -30 > "$OUT_BASE/active-frames.txt"

# 简短栈类型分类（区分 idle wait / 销毁 / 活跃 polling / async runtime）
{
    echo "## 栈类型快速统计"
    awk '
        /Inner::run.*\+ 212/ {idle_wait++}
        /Inner::run.*\+ 1408/ {destroying++}
        /Inner::run.*\+ 400/ {actively_polling++}
        /Context::run.*\+ 3156/ {async_running++}
        /park_internal.*kevent/ {async_idle++}
        /thread::lifecycle.*JoinInner.*join/ {join_count++}
        /__ulock_wait/ {ulock_wait++}
        END {
            print "blocking idle wait (+212):    " idle_wait+0
            print "blocking destroying (+1408):  " destroying+0
            print "blocking actively polling:    " actively_polling+0
            print "async worker running (+3156): " async_running+0
            print "async worker idle (kevent):   " async_idle+0
            print "thread join calls:            " join_count+0
            print "__ulock_wait:                 " ulock_wait+0
        }
    ' "$OUT_BASE/sample.txt"
} > "$OUT_BASE/stack-classification.txt"

echo ""
echo "== 输出文件 =="
ls -la "$OUT_BASE/"
echo ""
echo "## 线程类型 Top 10"
head -10 "$OUT_BASE/thread-types.txt"
echo ""
echo "## 栈分类"
cat "$OUT_BASE/stack-classification.txt"
echo ""
echo "## 活跃栈（去 idle wait）Top 10"
head -10 "$OUT_BASE/active-frames.txt"
