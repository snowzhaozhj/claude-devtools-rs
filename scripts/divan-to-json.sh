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
function json_escape(s) {
  gsub(/\\/, "\\\\", s)
  gsub(/"/, "\\\"", s)
  gsub(/\t/, "\\t", s)
  gsub(/\r/, "\\r", s)
  gsub(/\n/, "\\n", s)
  return s
}

function get_indent_level(s,    clean, pos) {
  clean = s
  gsub(/\033\[[0-9;]*m/, "", clean)
  # 找到第一个字母或数字的位置（byte offset）
  # 跳过 UTF-8 树形字符(│├╰─) 和空格
  pos = 0
  while (match(clean, /^[^a-zA-Z0-9]/)) {
    pos += RLENGTH
    clean = substr(clean, RLENGTH + 1)
  }
  return pos
}

BEGIN {
  print "["
  first = 1
  unit_factor["ns"] = 0.001
  unit_factor["µs"] = 1
  unit_factor["us"] = 1
  unit_factor["ms"] = 1000
  unit_factor["s"]  = 1000000
  group_count = 0
}

/^[[:space:]]*$/ { next }
/fastest.*slowest.*median.*mean/ { next }
/samples.*iters/ { next }
/Timer precision/ { next }

{
  line = $0
  gsub(/\033\[[0-9;]*m/, "", line)
}

# bench binary 名（顶层行）
/^[a-zA-Z_][a-zA-Z0-9_]*/ && !/[├╰│─]/ {
  match(line, /^[a-zA-Z_][a-zA-Z0-9_]*/)
  if (RSTART > 0) {
    current_binary = substr(line, RSTART, RLENGTH)
    if (prefix == "") {
      active_prefix = current_binary
    } else {
      active_prefix = prefix
    }
  }
  group_count = 0
  next
}

/[├╰│]/ {
  name_part = ""
  data_part = ""

  if (match(line, /[0-9]+(\.[0-9]+)? *(ns|µs|us|ms|s) *│/)) {
    name_part = substr(line, 1, RSTART - 1)
    data_part = substr(line, RSTART)
  } else if (match(line, /[0-9]+(\.[0-9]+)? *(ns|µs|us|ms|s)/)) {
    name_part = substr(line, 1, RSTART - 1)
    data_part = substr(line, RSTART)
  } else {
    # 纯分组行：记录分组名和缩进层级
    indent = get_indent_level($0)
    gsub(/[│├╰─ ]+/, " ", line)
    gsub(/^ +| +$/, "", line)
    if (line != "") {
      # 清除同级或更深的旧分组
      new_count = 0
      for (i = 0; i < group_count; i++) {
        if (group_indent[i] < indent) {
          new_count++
        } else {
          break
        }
      }
      group_count = new_count
      group_names[group_count] = line
      group_indent[group_count] = indent
      group_count++
    }
    next
  }

  # 数据行
  data_indent = get_indent_level($0)

  gsub(/[│├╰─ ]+/, " ", name_part)
  gsub(/^ +| +$/, "", name_part)
  bench_name = name_part

  n_fields = split(data_part, fields, "│")
  if (n_fields >= 3) {
    median_field = fields[3]
    gsub(/^ +| +$/, "", median_field)
    if (match(median_field, /([0-9]+(\.[0-9]+)?)/)) {
      median_val = substr(median_field, RSTART, RLENGTH) + 0
      unit_str = substr(median_field, RSTART + RLENGTH)
      gsub(/^ +| +$/, "", unit_str)
      if (unit_str == "") unit_str = "µs"

      if (unit_str in unit_factor) {
        value_us = median_val * unit_factor[unit_str]
      } else {
        value_us = median_val
      }

      # 构建完整 name：prefix + 比当前浅的所有分组 + bench_name
      full_name = active_prefix
      for (i = 0; i < group_count; i++) {
        if (group_indent[i] < data_indent) {
          full_name = full_name "/" group_names[i]
        } else {
          break
        }
      }
      if (bench_name != "") {
        full_name = full_name "/" bench_name
      }

      if (!first) printf ",\n"
      printf "  {\"name\": \"%s\", \"unit\": \"µs\", \"value\": %.3f}", json_escape(full_name), value_us
      first = 0
    }
  }
}

END {
  if (!first) printf "\n"
  print "]"
}
'
