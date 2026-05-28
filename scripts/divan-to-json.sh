#!/usr/bin/env bash
# 将 divan bench 的 stdout 转换为 github-action-benchmark 的 customSmallerIsBetter JSON 格式。
#
# 用法：
#   cargo bench -p cdt-parse 2>/dev/null | scripts/divan-to-json.sh cdt-parse > out.json
#   cargo bench --workspace 2>/dev/null | scripts/divan-to-json.sh > out.json
#
# 输入：divan 的表格格式 stdout（含 Unicode 树形字符）
# 输出：JSON 数组，每项 {name, unit, value}，value 取 median 列
#
# 如果传了第一个参数，所有 bench name 前缀加 "<arg>/"；
# 如果没传，用 divan 输出的 bench binary 名作为前缀。
set -euo pipefail

PREFIX="${1:-}"

awk -v prefix="$PREFIX" '
BEGIN {
  print "["
  first = 1
  # 单位到微秒的换算因子
  unit_factor["ns"] = 0.001
  unit_factor["µs"] = 1
  unit_factor["us"] = 1
  unit_factor["ms"] = 1000
  unit_factor["s"]  = 1000000
}

# 跳过空行和表头行
/^[[:space:]]*$/ { next }
/fastest.*slowest.*median.*mean/ { next }
/samples.*iters/ { next }
/Timer precision/ { next }

{
  line = $0
  # 去掉 ANSI 转义码
  gsub(/\033\[[0-9;]*m/, "", line)
}

# 检测 bench binary 名（顶层行：无树形前缀的非空行）
# 格式: "bench_name        fastest │ ..." 或纯 "bench_name"
/^[a-zA-Z_][a-zA-Z0-9_]*/ && !/[├╰│─]/ {
  # 提取第一个 word 作为 binary name
  match(line, /^[a-zA-Z_][a-zA-Z0-9_]*/)
  if (RSTART > 0) {
    current_binary = substr(line, RSTART, RLENGTH)
    # 如果没指定 prefix，用 binary 名
    if (prefix == "") {
      active_prefix = current_binary
    } else {
      active_prefix = prefix
    }
  }
  # 清空层级栈
  delete path_stack
  depth = 0
  next
}

# 解析带数据的行（含 │ 分隔的数值列）
# 典型格式:
#   │  ├─ 100                           66.7 µs       │ 122.6 µs      │ 85.22 µs      │ 84.24 µs
#   ├─ cold_project_scan                1.234 ms      │ 2.345 ms      │ 1.567 ms      │ 1.678 ms
/[├╰│]/ {
  # 计算缩进深度（通过统计 │ 和缩进字符）
  indent_line = line
  # 去掉数据部分（第一组数字开始之后的内容）
  # 找 bench 名称部分
  name_part = ""
  data_part = ""

  # 分割：找第一个数字+单位模式之前的内容为 name_part
  if (match(line, /[0-9]+(\.[0-9]+)? *(ns|µs|us|ms|s) *│/)) {
    name_part = substr(line, 1, RSTART - 1)
    data_part = substr(line, RSTART)
  } else if (match(line, /[0-9]+(\.[0-9]+)? *(ns|µs|us|ms|s)/)) {
    name_part = substr(line, 1, RSTART - 1)
    data_part = substr(line, RSTART)
  } else {
    # 纯分组行（没有数据），记录层级名
    # 去掉树形字符提取名字
    gsub(/[│├╰─ ]+/, " ", line)
    gsub(/^ +| +$/, "", line)
    if (line != "") {
      # 算深度：原始行中 "│" 的数量
      d = 0
      tmp = $0
      gsub(/\033\[[0-9;]*m/, "", tmp)
      while (match(tmp, /│/)) {
        d++
        tmp = substr(tmp, RSTART + 3)  # UTF-8 │ = 3 bytes
      }
      depth = d
      path_stack[depth] = line
      # 清除更深层级
      for (k in path_stack) { if (k > depth) delete path_stack[k] }
    }
    next
  }

  # 从 name_part 提取 bench 名称
  gsub(/[│├╰─ ]+/, " ", name_part)
  gsub(/^ +| +$/, "", name_part)
  bench_name = name_part

  # 从 data_part 提取 median（第三个值）
  # 格式: "66.7 µs       │ 122.6 µs      │ 85.22 µs      │ 84.24 µs"
  # 按 │ 分割
  n_fields = split(data_part, fields, "│")
  if (n_fields >= 3) {
    median_field = fields[3]
    gsub(/^ +| +$/, "", median_field)
    # 提取数值和单位
    if (match(median_field, /([0-9]+(\.[0-9]+)?)/)) {
      median_val = substr(median_field, RSTART, RLENGTH) + 0
      # 提取单位
      unit_str = substr(median_field, RSTART + RLENGTH)
      gsub(/^ +| +$/, "", unit_str)
      if (unit_str == "") unit_str = "µs"

      # 转为微秒统一单位
      if (unit_str in unit_factor) {
        value_us = median_val * unit_factor[unit_str]
      } else {
        value_us = median_val
      }

      # 构建完整 name
      full_name = active_prefix
      for (i = 0; i <= depth; i++) {
        if (i in path_stack) {
          full_name = full_name "/" path_stack[i]
        }
      }
      if (bench_name != "") {
        full_name = full_name "/" bench_name
      }

      # 输出 JSON
      if (!first) printf ",\n"
      printf "  {\"name\": \"%s\", \"unit\": \"µs\", \"value\": %.3f}", full_name, value_us
      first = 0
    }
  }
}

END {
  if (!first) printf "\n"
  print "]"
}
'
