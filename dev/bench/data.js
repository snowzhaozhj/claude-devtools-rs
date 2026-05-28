window.BENCHMARK_DATA = {
  "lastUpdate": 1779947078725,
  "repoUrl": "https://github.com/snowzhaozhj/claude-devtools-rs",
  "entries": {
    "Divan Benchmarks": [
      {
        "commit": {
          "author": {
            "email": "81480356+snowzhaozhj@users.noreply.github.com",
            "name": "snowzhaozhj",
            "username": "snowzhaozhj"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0468e42f12b18f0ad9247890a147018443a22bbd",
          "message": "feat(ci): add divan bench trend tracking via github-action-benchmark (#373)\n\n* feat(ci): add divan bench trend tracking via github-action-benchmark (#360)\n\n接入 github-action-benchmark 对 divan 算法级 bench 做持续追踪：\n- scripts/divan-to-json.sh: 解析 divan stdout → customSmallerIsBetter JSON\n- scripts/run-divan-bench.sh: 跑全 workspace bench 合并输出\n- .github/workflows/bench-trend.yml: push to main 存 gh-pages 历史 +\n  PR 对比评论（alert-threshold 130%，不 fail 只 comment）\n\n图表页面：合并后首次 push 自动创建 gh-pages 分支 + 部署到\nhttps://snowzhaozhj.github.io/claude-devtools-rs/dev/bench/\n\nCloses #360\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* fix(ci): mkdir target before divan bench output redirect\n\nCI fresh checkout 没有 target/ 目录，shell redirect 在脚本执行前就\n尝试打开文件导致 No such file or directory。\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* fix: address codex review findings + gh-pages bootstrap\n\nCodex 二审修复：\n- divan-to-json.sh: 加 json_escape() 防止 bench name 含引号破坏 JSON\n- divan-to-json.sh: 用 indent-level 算法替代 pipe-count，修复\n  validate_encoded_path 等末尾分组路径丢失问题\n- run-divan-bench.sh: 空结果时 exit 1 而非静默通过\n- run-divan-bench.sh: --crate 缺参数时明确报错\n- bench-trend.yml: fork PR 跳过 comment（GITHUB_TOKEN 只读）\n- bench-trend.yml: pin actions 到完整 SHA\n- bench-trend.yml: gh-pages 不存在时优雅跳过 Compare step\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* chore(ci): bump actions to latest versions\n\n- actions/checkout: v4.2.2 → v6.0.2\n- Swatinem/rust-cache: v2.7.8 → v2.9.1\n- actions/upload-artifact: v4.6.2 → v7.0.1\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.7 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-28T12:37:13+08:00",
          "tree_id": "20ba2763ea22df4a568c9776195221aec93a5da4",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/0468e42f12b18f0ad9247890a147018443a22bbd"
        },
        "date": 1779944049318,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 113.6,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1130,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 6959,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 1.492,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.261,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 47.15,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 34.52,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 302.4,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1330,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 2669,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2792,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 40990,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 63.22,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 62.11,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 629.9,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6315,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 192,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1927,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 51.95,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 523.9,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5276,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 130.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1299,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 7.341,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 73.17,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 9392,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 954,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9161,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 949,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 49.61,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 521.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 92.07,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 922.6,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9246,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 196,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1273,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12170,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "81480356+snowzhaozhj@users.noreply.github.com",
            "name": "snowzhaozhj",
            "username": "snowzhaozhj"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "512a3ce20de0a4249115f59f10c52c4f33f6ac49",
          "message": "refactor(rules): expand codex role + fix worktree hook (#374)\n\n* refactor(rules): rewrite codex-usage — expand role, drop version lock, compress\n\n- Remove hardcoded \"GPT-5.4\" — model governed by ~/.codex/config.toml\n- Expand from 5 to 8 trigger points (add: adversarial verification,\n  refactor impact, perf root cause, error path completeness)\n- Merge edge-case + concurrency into generalized \"adversarial verification\"\n  covering 6 sub-domains (concurrency/state-machine/cache/error-recovery/\n  config-combo/async-lifecycle)\n- Change PR review from \"always run\" to \"high-risk trigger hit\" to reduce\n  cost on low-risk PRs\n- Add \"devil's advocate\" 2-question protocol to design review\n- Add core principles section (minimal context prompts, require repro path,\n  continue-don't-restart)\n- Cut 78→61 lines; move verbose examples/choreography to templates\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* fix(hooks): use git toplevel instead of CLAUDE_PROJECT_DIR for push check\n\nCLAUDE_PROJECT_DIR always points to the main repo root, but when pushing\nfrom a worktree the openspec check should inspect the worktree's state.\nUse `git rev-parse --show-toplevel` which correctly resolves to the\ncurrent worktree root.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* refactor(rules): default-call philosophy + explore trigger + observable signals\n\n- Switch from \"conditional-call\" to \"default-call + trivial exemption\"\n  to reduce missed triggers (Claude skipping codex when it shouldn't)\n- Add #9 explore divergence trigger: ≥2 viable options affecting data\n  model/IPC/persistence/async/perf → adversarial perspective\n- Replace subjective triggers with observable signals:\n  \"stuck 30min\" → \"3 attempts failed or 30min no progress\"\n  \"scenario incomplete\" → \"no test name mapping exists\"\n- Add explicit trivial exemption criteria (all must hold to skip)\n- Require \"Codex skipped: <reason>\" trace when exempted\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* docs(templates): add codex prompt templates for new trigger points\n\n- codex-prompt-adversarial.md: #4 adversarial verification (concurrency/\n  state-machine/cache/error-recovery/config-combo/async-lifecycle)\n- codex-prompt-explore-divergence.md: #9 explore direction fork\n- codex-prompt-refactor-perf-error.md: #6 refactor impact, #7 perf\n  regression root cause, #8 error boundary completeness\n\nThese complement the existing pr-review, design-review, and progressive-\ndiagnosis templates, ensuring all 9 trigger points have corresponding\nprompt skeletons.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* docs(templates): add \"blind spot direction\" question to explore prompt\n\nAdd question 3: \"有没有我完全没考虑到的方向 D？\" — leverages\nheterogeneous reasoning to surface search-space blind spots that Claude\nmay miss due to anchoring on early options. Explicitly scoped: must be\nfundamentally different from listed options, allowed to say \"无\".\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.7 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-28T13:41:23+08:00",
          "tree_id": "e60b79725eb2759c3386fd642af862142d349f42",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/512a3ce20de0a4249115f59f10c52c4f33f6ac49"
        },
        "date": 1779947078321,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 113.1,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1112,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 5516,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.861,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.341,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 45.92,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 34.73,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 294,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1242,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3170,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2866,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 40520,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 62.82,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 62.53,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 632,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6349,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 195.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1958,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 52.44,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 527.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5290,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 128.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1291,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 7.34,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 73.17,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 8881,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 921.8,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8845,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 926.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 49.74,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 520.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 93.57,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 936.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9374,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 182.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1268,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12210,
            "unit": "µs"
          }
        ]
      }
    ]
  }
}