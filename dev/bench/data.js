window.BENCHMARK_DATA = {
  "lastUpdate": 1780300438436,
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
          "id": "b80fadcefc15891575276502284c995cb9f22d08",
          "message": "feat(mcp): add MCP stdio server with rmcp SDK (#375)\n\n* feat(mcp): add MCP stdio server with rmcp SDK (#366)\n\nImplements #366: MCP Server for claude-devtools session intelligence.\n\nArchitecture:\n- 8 read-only tools (list_projects, list_sessions, get_session_summary,\n  get_session_detail, get_session_errors, search_sessions,\n  get_session_cost, get_stats) via rmcp #[tool_router]\n- stdio transport (JSON-RPC over stdin/stdout)\n- Secret redaction layer (API keys, tokens, passwords → [REDACTED])\n- Structured truncation by chunk boundary (max_tokens param)\n- TokenEstimator trait for extensible token budget control\n\nCode changes:\n- Move summary/cost/stats pure algorithms from cdt-cli to cdt-query\n- New cdt-query::token module (TokenEstimator trait + CharRatioEstimator)\n- New cdt-cli::mcp module (CdtMcpServer + redact + truncate)\n- cdt-cli gains lib.rs target for integration test access\n- Integration tests via tokio duplex transport\n\nNew dependencies: rmcp 1.7, schemars 1.0\n\nPart of #362, closes #366.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* style: cargo fmt\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* fix(mcp): address codex review — parameter validation, search resolve, truncation edge case\n\n- get_stats: return explicit error instead of misleading success text\n- get_session_detail: invalid filter/range params now return McpError::invalid_params\n  with helpful message instead of silently falling back to no-filter\n- search_sessions: project param now goes through resolve_project() to handle names\n- truncate: handle single-chunk-exceeds-budget edge case (include it + mark truncated)\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* chore(opsx): archive mcp-server-stdio\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.7 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-28T14:32:13+08:00",
          "tree_id": "b03f1e30d70f40b94095cc307f7a3da1047bb7ba",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/b80fadcefc15891575276502284c995cb9f22d08"
        },
        "date": 1779950116128,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 113.8,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1097,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 5009,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.841,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.311,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 47.59,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 34.48,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 298.5,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1244,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3233,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2927,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 41020,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 61.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 62.17,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 629.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6325,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 194,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1947,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 51.86,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 524.3,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5304,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 127.9,
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
            "value": 9182,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 978,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8698,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 914,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 48.84,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 515.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 94.14,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 947.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9498,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 188.7,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1277,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12090,
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
          "id": "66ef47c24f94b28a85ac41aefc3ddc1d5344cac4",
          "message": "feat(cli): implement `cdt setup skills` with session-aware skill templates (#376)\n\n* feat(cli): implement `cdt setup skills` with 4 session-aware skill templates (#367)\n\nAdd skill template installation to the `cdt` CLI. Templates are compiled\ninto the binary via `include_str!` and written to `.claude/skills/` in the\nuser's current project directory.\n\nSkills included:\n- analyze-failures: identify error patterns across recent sessions\n- token-usage: aggregate token consumption and cost estimates\n- search-errors: full-text search for errors across sessions\n- session-diagnosis: comprehensive diagnostic report for a single session\n\nAlso adds \"Claude Code 集成\" section to README documenting both MCP and\nSkills setup.\n\nCloses #367\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* fix(cli): address codex review — return error on install failure, fix skill templates\n\n- cmd_setup_skills now returns Result<()> and bails if any writes fail\n- Skill templates updated to use real CLI interface (--project flag,\n  correct subcommands, no references to nonexistent JSON fields)\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* refactor(cli): consolidate 4 skills into 1 unified session-insights skill\n\nPer skill-creator best practices: fewer skills = less always-on context\ncost, avoids trigger overlap, and matches real workflows that combine\nmultiple analysis steps. One skill with workflow routing handles all\nsession analysis needs.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.7 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-28T16:26:13+08:00",
          "tree_id": "9c1e0b81097e6ac9ea808056fabd045abcbf35dd",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/66ef47c24f94b28a85ac41aefc3ddc1d5344cac4"
        },
        "date": 1779956966034,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 115.8,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1123,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4759,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 1.682,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 9.908,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 41.74,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.61,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 296.9,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1211,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3344,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3269,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 40360,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 67.04,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 59.56,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 611.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6225,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 192.3,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1926,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 53.94,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 546.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5456,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 116.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1168,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.501,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 64.76,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 9345,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 1017,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9567,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 1018,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 45.92,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 486,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 91.53,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 920.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9221,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 204,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1347,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12740,
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
          "id": "0799d48b40c8db9e281a896cc62b5debfd656f3c",
          "message": "feat(cli): add standalone binary distribution (#378)\n\n* feat(cli): add standalone binary distribution\n\n- Add `build-cli` job to release.yml (4 targets, parallel with Tauri build)\n- Add `install.sh` one-liner installer (auto-detect OS/arch, download from Release)\n- Add cargo-binstall metadata to cdt-cli/Cargo.toml\n- Update README with CLI installation section\n- Add CLI binary assets to publish verification checklist\n- Update release notes template with CLI download info\n\nCloses: related to #362\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* fix(cli): address codex review findings in install.sh and README\n\n- Fix CRITICAL: Windows path uses cdt.exe, skip chmod on Windows\n- Fix HIGH: remove cargo-binstall recommendation (crate not on crates.io)\n- Fix LOW: check unzip (not tar) on Windows, tar on Unix\n- Fix LOW: handle GitHub API rate-limit gracefully in version detection\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.7 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-28T17:22:35+08:00",
          "tree_id": "2e139a0935df88fd51be19ccd1d0a76ea567afcd",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/0799d48b40c8db9e281a896cc62b5debfd656f3c"
        },
        "date": 1779960342487,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 113.7,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1133,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4904,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 1.502,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.111,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 47.21,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.78,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 295.2,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1233,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 2901,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3335,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 41790,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 63.03,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 62.53,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 631.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6360,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 194.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1957,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 51.87,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 525.3,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5275,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 128,
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
            "value": 8665,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 977.6,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9139,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 931,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 51.78,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 538.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 93.85,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 946.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9486,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 190.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1295,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12230,
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
          "id": "8f457d3c76f5fb2788243aabfb6bfe932ae2676b",
          "message": "chore(release): 0.5.13 (#379)\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>",
          "timestamp": "2026-05-28T17:34:26+08:00",
          "tree_id": "0f80161b6a7f53dd5c0b90f411f10d5a99630e2d",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/8f457d3c76f5fb2788243aabfb6bfe932ae2676b"
        },
        "date": 1779961233209,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 117.4,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1132,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 5378,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.842,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.851,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 40.78,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 36.21,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 288.4,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1232,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 2855,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2962,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 39320,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 70.19,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 59.9,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 619.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6299,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 191.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1933,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 53.93,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 547.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5477,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 116.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1172,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.812,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 67.84,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 9364,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 1009,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9482,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 1013,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 47.7,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 503.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 91.83,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 919.6,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9267,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 206.9,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1342,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12730,
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
          "id": "4f77cf074b27855eba892dbd5294cd1a7ef0a839",
          "message": "fix(ui): pin copy button to right side of bash command block (#380)\n\nAdd flex: 1 and min-width: 0 to .bash-cmd so the command text fills\navailable space, pushing CopyButton to the right edge consistently\ninstead of floating inline after the command text.\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.7 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-28T19:00:28+08:00",
          "tree_id": "c0f33c4c5bd405805e7768012b12f69584a344b1",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/4f77cf074b27855eba892dbd5294cd1a7ef0a839"
        },
        "date": 1779966224374,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 116.5,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1133,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4769,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.842,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 9.186,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 43.93,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.53,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 296.3,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1220,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3289,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2988,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 38790,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 69.81,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 59.85,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 618,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6273,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 191.9,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1922,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 54.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 547.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5490,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 117.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1187,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.812,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 67.84,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 9394,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 1167,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 10180,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 1149,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 47.79,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 507.6,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 94.18,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 953.2,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9580,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 231.9,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1349,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12880,
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
          "id": "2e8a6d37186abe8b7ba6cd79850a771823963309",
          "message": "fix(cli): suppress perf tracing noise in release builds (#382)\n\nIn release mode, default tracing filter is \"info,cdt_api::perf=warn\"\nso timing probes (INFO) are hidden from users while failure diagnostics\n(WARN/ERROR) remain visible. Debug builds keep full \"info\" for dev use.\nUsers can always override via RUST_LOG env var.\n\nApplies to both cdt-cli and Tauri desktop app.\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.7 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-28T19:56:18+08:00",
          "tree_id": "9c9e103cce025d5ad4098fcea90f3d879f1db130",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/2e8a6d37186abe8b7ba6cd79850a771823963309"
        },
        "date": 1779969579811,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 112.9,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1114,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4757,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.831,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.101,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 46.18,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 34.54,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 296.8,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1240,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3262,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3302,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 40460,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 61.07,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 63.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 638.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6392,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 195,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1944,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 52.12,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 525.9,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5404,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 128.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1297,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 7.69,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 76.66,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 8637,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 869.3,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8758,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 985.9,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 48.97,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 513,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 93.49,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 948.2,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9505,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 184.9,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1293,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12410,
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
          "id": "08a5e93c1755d4792dec054ab25144ce159e8f7e",
          "message": "chore(release): 0.5.14 (#383)\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>",
          "timestamp": "2026-05-28T20:19:43+08:00",
          "tree_id": "fce1645323498c5f44d8b119fa5ee4ed3faf831d",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/08a5e93c1755d4792dec054ab25144ce159e8f7e"
        },
        "date": 1779970982702,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 113.9,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1113,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 5376,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 1.512,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.231,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 46.41,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 34.64,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 299.8,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1283,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3271,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3236,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 41640,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 59.87,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 62.59,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 636.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6437,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 222.9,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 2235,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 65.54,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 660.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 6605,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 127.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1285,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 7.69,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 76.66,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 8589,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 936.8,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8667,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 928.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 49.02,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 518,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 95.01,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 955.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9590,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 199.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1331,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12540,
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
          "id": "138f8e6bf0b05d1593103df9155c41d69055fe78",
          "message": "fix(cli): display full session ID in table output (#386)\n\n* fix(cli): display full session ID in table output\n\nSession IDs (UUID format, 36 chars) were truncated to 10 chars\nvia `.chars().take(10)` without ellipsis, making them unusable\nfor copy-paste into other commands like `sessions show <id>`.\n\nNow displays the full UUID in both `sessions list` and `search`\ntable output.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* fix(cli): correct separator line widths\n\nAlign separator dashes with actual column widths:\n- sessions list: 106 → 108\n- search: 118 → 119\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.7 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-29T10:42:10+08:00",
          "tree_id": "2412547f6871cfe83648809376423032bdd06928",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/138f8e6bf0b05d1593103df9155c41d69055fe78"
        },
        "date": 1780022830150,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 115,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1109,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4644,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.872,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 10.02,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 42.19,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 32.69,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 286.7,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1223,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 5490,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 5572,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 40770,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 102.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 60.51,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 608.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6130,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 193.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1954,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 54.97,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 557.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5580,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 119.9,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1214,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.501,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 64.76,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 9371,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 972.2,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9782,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 1129,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 46.26,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 493,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 93.74,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 937.6,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9400,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 198.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1320,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12700,
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
          "id": "76523b68e019741c4cd698591219733eae532dc3",
          "message": "fix: resolve 3 bugs found by bug-hunt audit (#385)\n\n1. CLI UTF-8 panic: `parse_duration_to_ms` used `split_at(len-1)` which\n   panics on multi-byte chars (e.g. \"7天\"). Fixed both occurrences\n   (main.rs + mcp/mod.rs) to use `char_indices().next_back()`.\n\n2. ContextPanel $derived misuse: `$derived(() => {...})` stored the\n   function instead of computing the result (should be `$derived.by`).\n   Template was calling it as `rankedGrouped()` which bypassed caching.\n\n3. CommandPalette group id mismatch: was passing group id to\n   `listSessions` (expects worktree project id), causing empty results\n   for multi-worktree groups. Changed to `listGroupSessions` which\n   correctly handles group-level queries.\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.7 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-29T10:52:36+08:00",
          "tree_id": "1419eb66c5e539e009c45be268bc43185f2e0149",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/76523b68e019741c4cd698591219733eae532dc3"
        },
        "date": 1780023349231,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 115.4,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1125,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4836,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.872,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 9.967,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 40.97,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 32.95,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 290.9,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1208,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3198,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3179,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 42510,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 66.13,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 59.12,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 605.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6041,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 193.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1946,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 55.01,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 557.9,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5564,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 117.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1184,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.501,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 64.76,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 10680,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 1145,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9648,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 1141,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 47.95,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 505,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 94.25,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 942.6,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9568,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 196.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1336,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12720,
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
          "id": "dcadbd8e9c3bf09a4e53ccb73417adca1cb6e004",
          "message": "feat(ui): add scroll arrows to worktree chip cluster (#384)\n\n* feat(ui): add scroll arrows and wheel support to worktree chip cluster\n\nPure mouse users couldn't scroll the horizontal worktree chip list.\nAdd left/right arrow buttons (conditionally visible on overflow) and\nwheel-to-horizontal-scroll mapping. Arrows follow DESIGN.md ghost\nbutton conventions (28x28 hit area, hover background, muted chevron).\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* fix(ui): address codex review findings for chip scroll arrows\n\n- W1: wheel handler only preventDefault when scroll direction has room\n- W2: scrollIntoView only fires on actual value change (untrack prevValue)\n- W3: $effect on options.length → tick → updateOverflow for dynamic chips\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* fix(ui): guard ResizeObserver for jsdom test environment\n\njsdom doesn't provide ResizeObserver; check typeof before constructing.\nOverflow detection still works via scroll event listener fallback.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.7 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-29T11:06:02+08:00",
          "tree_id": "293c9ef2fb8dff1aa95f133c1a61114f885254e0",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/dcadbd8e9c3bf09a4e53ccb73417adca1cb6e004"
        },
        "date": 1780024153139,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 114.6,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1125,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4786,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 1.536,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.312,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 47.24,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.6,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 292.6,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1240,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3146,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3251,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 40600,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 59.97,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 63.35,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 640.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6427,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 221.9,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 2227,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 66.05,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 663.9,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 6642,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 128.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1293,
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
            "value": 8562,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 971.8,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8706,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 974.6,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 48.44,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 510.7,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 93.18,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 940.6,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9404,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 185.9,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1307,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12420,
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
          "id": "ea585945530d74dc038c3ba3599aa09c9f3f72be",
          "message": "feat(context): per-turn context badge + visible context (#388)\n\n* feat(context): add TurnContextStats to SessionDetail IPC\n\nBackend implementation for per-turn context badge feature:\n- Add TurnContextStats struct to cdt-core (new_count, new_tokens,\n  new/cumulative tokens_by_category)\n- Add turn_context_stats field to SessionDetail (sparse map, only\n  turns with new_count > 0)\n- Project stats_map data that was previously computed and discarded\n  in inject_context_annotations\n- IPC contract tests for serialization shape + backward compat\n- OpenSpec change artifacts (proposal, design, spec delta, tasks)\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* feat(ui): add ContextBadge + token popover click + visible context\n\nFrontend implementation for per-turn context badge:\n- New ContextBadge.svelte: click pill \"Context +N\" with popover\n  showing category breakdown sorted by tokens desc\n- Token popover converted from hover to click trigger\n- Added \"Visible Context ≈N%\" section in token popover\n- Mutual exclusion: only one popover open at a time (per chunk)\n- Outside click / Esc dismiss for all popovers\n- contextExtractor.ts: TurnContextStats type, shouldShowBadge(),\n  buildInjectionsByTurnMap(), getCategoryBreakdown()\n- api.ts: SessionDetail.turnContextStats field\n- Fixed circular dependency: contextExtractor no longer imports\n  from api.ts\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* chore(opsx): tick business tasks for per-turn-context-badge\n\nAll implementation tasks complete. N.1-N.4 release steps pending.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* fix(ui): visible context fallback for turns without turnContextStats\n\nCodex CR #2: turnContextStats is sparse (only turns with new\ninjections), but Visible Context percentage should show for all\nturns. Fallback to session-level contextInjections token sum when\nper-turn cumulative data is unavailable.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* fix(ui): include turnContextStats in hasAiContent gate\n\nCodex CR #1: AI chunks with usage=None but non-zero context\ninjections (e.g. initial CLAUDE.md injection on first turn) were\nhidden by hasAiContent check. Add chunkTurnStats.newCount > 0 as\nan additional condition to ensure the header (and badge) renders.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* chore(opsx): archive per-turn-context-badge\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* fix(ui): widen context badge popover to prevent line wrapping\n\nSet min-width: 300px + width: max-content + white-space: nowrap on\ncategory labels and token values to prevent ugly line breaks in the\npopover content.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>\n\n* style(ui): impeccable polish for ContextBadge popover\n\nVisual critique fixes (impeccable critique + designer review):\n- Natural-language labels (badge \"Context\", category names, footer)\n  use sans; only numeric values (+N, token counts) stay mono. Fixes\n  \"reads like a log\" feel — category names are prose, not machine data.\n- focus-visible ring uses --color-accent-blue token (was hardcoded\n  rgba) + bumped alpha to 35% for keyboard a11y visibility.\n- Removed dead box-shadow accent-blue 0% ring (rendered nothing).\n\nAutomated detector: 0 findings. Designer review: 0 P0.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.7 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-29T12:25:05+08:00",
          "tree_id": "41a81b8a7371bb6a3d6e2867d95aacfad502580f",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/ea585945530d74dc038c3ba3599aa09c9f3f72be"
        },
        "date": 1780028899089,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 116.6,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1124,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4807,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.872,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 10.03,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 41.1,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.45,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 296.2,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1246,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3322,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3333,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 39130,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 67.64,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 61.41,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 623,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6129,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 192.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1938,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 55.65,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 563.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5621,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 117.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1185,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.501,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 64.76,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 9214,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 995.3,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9554,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 994.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 46.64,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 491.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 92.21,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 933.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9391,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 197.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1381,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12870,
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
          "id": "430b0deb133f3ca7239a86686cfa24d7f255c274",
          "message": "feat(tooling): add bug-hunt workflow (deterministic lens fanout + schema) (#396)\n\n* feat(tooling): add bug-hunt workflow for deterministic lens fanout + schema-enforced findings\n\nIntroduces .claude/workflows/bug-hunt.js — a Workflow tool script that\norchestrates bug hunting with:\n- Parallel fan-out of 6 lenses + domain reviewers (risk-level gated)\n- Batch 4-gate verification per lens (not per-candidate, controls cost)\n- Deterministic double-axis matrix classification in pure JS\n- Schema-enforced structured output (file:line + evidence + trigger + test gap)\n\nAlso updates SKILL.md with a hybrid wrapper (Step 1.5) that routes to\nthe workflow for scopes >= 10 files and falls back to direct scanning\nfor smaller scopes.\n\nValidated on cdt-config crate: 6 agents, found 5 critical UTF-8 byte-slice\npanics + 2 major silent-failure bugs, 0 false positives in findings tier.\n\nCloses #395\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(tooling): address codex review findings on bug-hunt workflow\n\nFixes 7 issues found by codex adversarial review:\n\n- Use computed confidence from gatesPassed (deterministic invariant),\n  not agent self-reported confidence that could bypass the matrix\n- Add windows-compat-reviewer to domain routing (was in comment only)\n- Guard rust/windows reviewers to only run for Rust scopes\n- Wrap all scope/candidate data in [UNTRUSTED DATA] blocks to mitigate\n  prompt injection via malicious scope args\n- Detect scan/gate all-null failures instead of falsely reporting \"clean\"\n- Unify discarded return type (always discardedCount: number)\n- Validate args at entry (scope required, enum sanitization)\n- Add Workflow to SKILL.md allowed-tools\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-29T14:21:00+08:00",
          "tree_id": "344538fa794652c14df60ebfba9291095296aa00",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/430b0deb133f3ca7239a86686cfa24d7f255c274"
        },
        "date": 1780035858773,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 115,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1150,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 5539,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 1.537,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.251,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 48.15,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 34.31,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 301.4,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1307,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3255,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3330,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 41330,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 62.16,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 63.24,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 639.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6417,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 218.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 2203,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 64.81,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 657.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 6571,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 128.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1289,
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
            "value": 8863,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 1011,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9260,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 934.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 49.79,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 522.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 91.87,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 927.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9301,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 198.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1309,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12450,
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
          "id": "bc5e09b8eacbb83b92a69aeb383ea1e7b705ed9f",
          "message": "feat(tool-linking): extract Workflow runId to ToolExecution (#404)\n\n* feat(tool-linking): extract Workflow runId to ToolExecution\n\nAdd `workflow_run_id: Option<String>` field to `ToolExecution` struct,\nextracted from `toolUseResult.runId` during pair phase (before output\ntrim). This is the foundational key for associating Workflow tool calls\nwith their `workflows/wf_<runId>.json` manifest files.\n\nOnly extracted when `tool_name == \"Workflow\"` — zero overhead for all\nother tool types. Field uses `skip_serializing_if = \"Option::is_none\"`\nto ensure no payload impact on non-Workflow sessions.\n\nCloses #398\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test(tool-linking): add pair-level tests for workflow_run_id extraction\n\nAddress codex review finding: IPC contract tests only verify\nserialization shape but don't exercise the actual extraction logic\nin pair_tool_executions. Add 3 unit tests:\n- workflow_run_id_extracted_from_tool_use_result (positive)\n- workflow_run_id_none_when_run_id_missing (graceful degradation)\n- workflow_run_id_none_for_non_workflow_tool (isolation)\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* chore(opsx): archive workflow-run-id-extraction\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-29T15:00:16+08:00",
          "tree_id": "5a41d855f16394a2684d14621140b53421ec510f",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/bc5e09b8eacbb83b92a69aeb383ea1e7b705ed9f"
        },
        "date": 1780038210061,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 115.4,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1127,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 5425,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.831,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 9.777,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 41.99,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 32.7,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 289.7,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1231,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3292,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3075,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 41140,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 65.62,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 60.41,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 609,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6041,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 190.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1923,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 55.17,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 560.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5592,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 117.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1184,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.501,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 64.76,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 10680,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 1018,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9449,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 1015,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 51.05,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 538.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 93.19,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 934,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9441,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 231.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1411,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12770,
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
          "id": "fccdbaaf62a545b9a084a372b75a48782b80a554",
          "message": "fix(ui): remove empty whitespace row above tool input/output blocks (#405)\n\n* fix(ui): remove empty whitespace row above tool input/output blocks\n\nWhen OutputBlock had no label (used by DefaultToolViewer for INPUT/OUTPUT),\nit rendered a dedicated header row containing only the CopyButton, creating\na large blank area that looked like a bug.\n\nFix: when no label is present, position the CopyButton as an absolute\noverlay in the top-right corner of the code block (visible on hover),\neliminating the empty row entirely.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* test(fixture): add Read/Edit/Write tool executions to multi-project-rich\n\nCover all tool viewer paths (DefaultToolViewer, ReadToolViewer,\nEditToolViewer, WriteToolViewer, BashToolViewer) so that CopyButton\nplacement can be visually verified for each type.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-29T15:42:42+08:00",
          "tree_id": "96bd7a91812d16cd2568b0fed84e09c2a491683d",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/fccdbaaf62a545b9a084a372b75a48782b80a554"
        },
        "date": 1780040765496,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 113.2,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1149,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 5043,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.821,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.176,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 45.26,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.8,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 297.5,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1261,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3053,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2996,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 41040,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 61.41,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 67.44,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 671.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6475,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 222.9,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 2232,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 65.66,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 679.3,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 6607,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 130.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1315,
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
            "value": 8690,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 880.9,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9172,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 936.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 48.36,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 510.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 92.45,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 931.2,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9337,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 186.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1294,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12390,
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
          "id": "e6e03a12665237cf1154dde2e290113db0a0d8af",
          "message": "feat(workflow): WorkflowCard rendering — backend manifest + frontend 6-state (#397) (#406)\n\n* feat(ui): WorkflowCard 6-state rendering with mock fixture (#400)\n\n- Add WorkflowPhase / WorkflowAgent / WorkflowItem types to api.ts\n- Add AIChunk.workflows? optional field for backend compatibility\n- Extend displayItemBuilder with WorkflowDisplayItem union + pool sorting\n- Add \"Workflow\" case to getToolSummary in toolHelpers\n- Create WorkflowCard.svelte with 6-state rendering:\n  - done: phase tree + green agent chips\n  - partial_failure: \"N failed\" header + red chips\n  - running: spinner + \"details available after completion\"\n  - launch error: via BaseItem error path (no empty card)\n  - empty: \"No subagents\" text\n  - hover: header/chip bg transition\n- Create workflow-rich fixture (4 variants + launch error)\n- Add vitest tests for displayItemBuilder + toolHelpers workflow logic\n- Add openspec delta for session-display + tool-viewer-routing specs\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix(ui): align WorkflowCard CSS with Visual Contract\n\n- Header padding/gap: use --bubble-header-padding-l1 / --bubble-header-gap tokens\n- Done status: add 14x14 checkmark SVG icon\n- Failure tag: 10px/500/uppercase + red border (per Visual Contract)\n- Spinner: 10x10 (was 8x8)\n- Chip: add border-color 0.12s transition\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix(ui): address codex review findings for WorkflowCard\n\n- Add default branch to statusLabel switch (BUG 1)\n- Fix agent chip each-key: use phase-index composite to avoid collision (ISSUE 2)\n- Fix test name to match assertion: \"returns empty string when input is null\" (ISSUE 3)\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* feat(workflow): manifest parsing + WorkflowItem + cache (#399)\n\nAdd backend infrastructure for Workflow tool specialized rendering:\n\n- cdt-core: WorkflowItem/WorkflowPhase/WorkflowAgent/WorkflowStatus types\n  with serde camelCase + skip_serializing_if for zero-payload guarantee\n- cdt-api: WorkflowManifestCache (FileSignature-based, stat-only after\n  first read) + parse_manifest + resolve_workflow_items (async, SSH-compat)\n- Integration: get_session_detail Step 5.5 conditionally resolves workflow\n  items only when Workflow tool_use chunks exist (zero-cost when absent)\n- Failure detection: combination of logs \"failed\" + tokens=0 + toolCalls=0\n  (manifest state field is unreliable — always \"done\")\n- Graceful degradation: missing/corrupt manifest → WorkflowItem::pending()\n- IPC contract tests: 4 new tests locking camelCase field names and\n  skip_serializing_if behavior\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix(ui): align WorkflowCard with backend data contract\n\nCRITICAL fixes from codex review:\n\n1. Data flow: workflows now read from SessionDetail.workflowItems (top-level)\n   and matched via toolExecution.workflowRunId, not from AIChunk.workflows\n2. Field rename: WorkflowAgent.status → WorkflowAgent.state\n3. Enum alignment: agent state uses \"pending\"/\"running\"/\"completed\"/\"failed\"\n   (matching backend), not \"done\"/\"queued\"/\"cached\"\n4. WorkflowItem.status union: added \"failed\" variant\n\nStructural changes:\n- api.ts: add SessionDetail.workflowItems?, ToolExecution.workflowRunId?,\n  remove AIChunk.workflows?\n- displayItemBuilder.ts: remove workflow-from-chunk pool logic\n- ExecutionTrace.svelte: accept workflowItems prop, match tool→WorkflowCard\n- SessionDetail.svelte: derive workflowMap, match tool→WorkflowCard inline\n- WorkflowCard.svelte: agent.status → agent.state, enum value updates\n- workflow-rich fixture: restructured to match real backend shape\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* chore(opsx): archive workflow-card-frontend\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-29T16:57:45+08:00",
          "tree_id": "2feeb479de47140c9f14290bf4ed1cc886dc15dd",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/e6e03a12665237cf1154dde2e290113db0a0d8af"
        },
        "date": 1780045272308,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 130,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1147,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 5249,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 1.467,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.091,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 46.62,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.57,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 302.3,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1258,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3009,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3254,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 42500,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 59.49,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 63,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 636.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6375,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 217.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 2193,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 66.12,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 661,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 6633,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 127.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1288,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 7.34,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 73.18,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 8635,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 886.4,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8874,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 943.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 49.58,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 523.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 93.87,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 928,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9305,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 189.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1329,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12660,
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
          "id": "1add75749958e8ca93c169d30c6de126620d507e",
          "message": "fix(workflow): crash on expanding WorkflowCard + summary not counting workflows (#408)\n\n* fix(workflow): crash on expanding WorkflowCard + summary not counting workflows\n\nWorkflowCard crashed with `TypeError: undefined is not an object\n(evaluating '$$props.workflow.phases.length')` because backend\n`skip_serializing_if = \"Vec::is_empty\"` omitted `phases`/`agents` from\nJSON when empty, but frontend type declared them as non-optional arrays.\n\nAlso fixed tool summary bar not counting workflow calls separately —\nbuildSummary now accepts optional workflowRunIds to distinguish them.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix(workflow): deduplicate workflow counting in buildSummary\n\nMultiple tool calls sharing the same workflowRunId were each incrementing\nthe workflow counter, producing misleading summaries like \"4 workflows\"\nwhen only 1 workflow existed. Now deduplicates by tracking seen runIds.\n\nAlso preserved the `case \"workflow\"` branch for future WorkflowDisplayItem\nsupport by adding the runId to the same dedup set.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* refactor(workflow): revert backend change, fix frontend type instead\n\nBackend `skip_serializing_if = \"Vec::is_empty\"` is a valid payload\noptimization — empty phases/agents should not waste bandwidth. The real\nfix is frontend: mark `phases` and `agents` as optional in the TypeScript\ntype (`phases?: WorkflowPhase[]`), and the component already uses `?? []`.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-29T18:06:48+08:00",
          "tree_id": "a26259deb79b94f5fb20a6c6ebcdeb61d836b308",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/1add75749958e8ca93c169d30c6de126620d507e"
        },
        "date": 1780049408485,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 112.9,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1120,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 5383,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.841,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.102,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 47.51,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 34.02,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 294,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1243,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3063,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2932,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 40800,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 59.83,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 63.08,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 639.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6403,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 222.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 2210,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 65.23,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 656.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 6572,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 128.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1293,
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
            "value": 8671,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 909.8,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8659,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 920.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 48.22,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 514,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 91.77,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 928.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9265,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 186,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1303,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12250,
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
          "id": "6b4bb46d953dbf4d3a0a4165990bdb575c7a0a1d",
          "message": "feat(perf): parallelize subagent scan + merge double file reads (#409)\n\n* feat(perf): parallelize subagent scan + merge double file reads\n\nOptimize `parse_subagent_candidate` to use a single structured parse\n(via `parse_file_via_fs`) instead of two full file reads (generic Value\nscan + structured parse). Additionally, parallelize subagent processing\nwithin the same project directory using Semaphore(4).\n\nResults on a 31-subagent session:\n- scan_subagents_ms: 921ms → ~90ms (90% reduction)\n- total get_session_detail: 975ms → ~100ms\n- user/real ratio: 0.19 (well within 0.66 budget)\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* chore(opsx): archive perf-subagent-scan-parallel\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-29T18:47:36+08:00",
          "tree_id": "c28167023024bdf6a9abfd8c4c2ac6f00624183e",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/6b4bb46d953dbf4d3a0a4165990bdb575c7a0a1d"
        },
        "date": 1780051834797,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 88.22,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 867.9,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 3766,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.651,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 6.269,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 35.58,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 26.06,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 229.5,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 986.3,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 2572,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2221,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 31240,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 48.18,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 48.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 491.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 4930,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 169.9,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1709,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 50.73,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 509.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5085,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 101.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1020,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 5.708,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 56.86,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 6956,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 741.2,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 6830,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 722.9,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 39.17,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 403.7,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 72.18,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 725.6,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 7237,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 156,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1032,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 9674,
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
          "id": "6515cf7df9851e32210670f64d8e02e2f425be4f",
          "message": "fix(workflow): WorkflowCard renders blank on expand (#410)\n\n* fix(workflow): WorkflowCard renders blank — wrong session_dir path + unrecognized \"done\" state\n\nTwo bugs caused WorkflowCard to always show \"0 phases · 0 agents · PENDING\":\n\n1. session_dir was computed as `jsonl_path.parent()` (= project_dir), but\n   manifests live at `<project_dir>/<session_id>/workflows/<run_id>.json`.\n   Fixed: use `located.project_dir.join(session_id)`.\n\n2. Claude Code workflow manifests report agent state as \"done\", not\n   \"completed\". The parser only matched \"completed\"/\"running\", so all\n   agents fell through to Pending.\n   Fixed: accept \"done\" as Completed.\n\nAlso reads the top-level `status` field from manifests as a fallback for\noverall workflow status derivation.\n\nVerified with real session b01806f0 (wf_a04767d2-4f1): 2 phases, 8 agents,\nall correctly showing Completed with proper token/duration data.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix(workflow): tighten top-level status fallback per codex review\n\nOnly use manifest's top-level \"completed\" status as fallback when no\nagents are actively in \"running\" state, preventing premature completion\nmarking.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-29T18:52:09+08:00",
          "tree_id": "b1690344374001a0721e1facdda738c56793f111",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/6515cf7df9851e32210670f64d8e02e2f425be4f"
        },
        "date": 1780052127992,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 116.7,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1144,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 5030,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 1.071,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 10.87,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 42.48,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 32.8,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 310.9,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1219,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3238,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3184,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 38720,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 62.74,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 59.65,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 610.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6171,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 197.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1961,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 55.34,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 562.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5630,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 118.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1196,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.502,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 74.1,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 9579,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 1025,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9780,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 984.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 46.92,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 497.2,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 92.91,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 932.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9435,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 218.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1354,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 13100,
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
          "id": "bedec62db6d23337c54ae11f83eb46421251aeb3",
          "message": "feat(workflow): 运行态降级渲染（manifest 缺失用 journal + scriptPath 合成 Running 态） (#412)\n\n* feat(workflow): 运行态降级渲染（manifest 缺失时用 journal + scriptPath 合成 Running 态）\n\nissue #397 PR 6。Workflow manifest 完成后才写，运行中不存在——此前 UI 空白。\n本 PR 在 manifest 缺失时诚实降级出 Running 态。\n\nTier 0（零依赖）：\n- ToolExecution 加 workflow_script_path（toolUseResult.scriptPath，回退 input.scriptPath）\n- resolve_running_state：读 journal.jsonl 按 agentId 数 started/result 合成匿名 agents\n  （有 result→Completed 仅 started→Running），独立于 manifest 失败启发式\n- name 从 scriptPath basename 精确 strip_suffix 剥 runId 后缀；journal 按 FileSignature 缓存\n- 前端 WorkflowCard 运行态：N agents (M done) 计数 + \"Agent N\" 匿名 chip；修复空 body gap\n\nTier 1（引 json5）：\n- workflow_script::parse_script_meta 隔离 lexer 切 meta 块 → json5 取 name+phases\n  失败静默降回 Tier 0；按 script FileSignature 缓存\n- 前端 Tier 1 phases 作静态 pill 列表（合成 agent 无真实 phaseIndex，不分组）\n\n走 openspec change workflow-running-degradation（tool-execution-linking + ipc-data-api\n+ session-display 三 capability spec delta）。perf 实测无回归。\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(workflow): 降级路径区分 NotFound 与读取错误（替代二审修复）\n\ncodex 额度耗尽，改用本仓 reviewer subagent 做异构替代二审，修复 3 个本 PR\n新增降级路径的健壮性缺陷 + 加固 1 处实证假设：\n\n- resolve_single：manifest stat 失败原先 `let Ok else` 把所有错误当「manifest\n  缺失」进运行态合成——非 NotFound（权限/IO/SSH 抖动）时 manifest 可能真实存在\n  却读不到，会合成虚假 Running 卡片。改按 FsError::NotFound 分流：仅 NotFound\n  降级运行态，其余 warn + pending。对齐 design「manifest 缺失」语义。\n- read_journal_agents：journal stat/read 失败同样区分 NotFound；非 NotFound 的\n  read 失败原先静默吞掉（无日志）导致 Running 被误降 Pending，现加 warn 留痕。\n- read_script_meta：Tier1 script read 失败原先 `Err(_) => None` 丢弃 error，加\n  debug 日志区分 read 异常 vs json5 解析降级（预期 graceful）。\n- 缓存 stale 注释护栏：注明不返 stale 计数依赖 journal append-only size 单调增。\n- 加固 extract_journal_agent_id 顶层优先测试（result 为 JSON 对象内嵌未转义 key）。\n\n新增 3 测试（含 FaultyFs 注错 mock 覆盖非 NotFound 降级分流）；cdt-api 全量\n300+ 测试通过，clippy 0 warning。\n\npre-existing 未在本 PR 修：parse_manifest 的 failed_by_heuristic 对 state=running\n的 agent 也套用失败启发式（main 已存在，触发罕见——manifest 完成才写），另记\nIssue 跟踪。\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* perf(workflow): WorkflowCard agentLabel 消除 O(n²) indexOf（自查二审）\n\nagentLabel 原先在 {#each agents} 内对每个 chip 调 agents.indexOf(agent)——\nN agents → O(n²) 渲染。design 明确运行态可能「极端 fan-out 上千 agent」，违反\nperf.md「O(N²) N>100」反模式。改为 index 由调用点传入（运行态全局序号 / 完成态\nphase 内序号，label 恒非空不触发 fallback）。\n\nsvelte-check 0 error；WorkflowCard vitest 8 例不回归。\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* chore(opsx): archive workflow-running-degradation\n\n3 capability spec delta（ipc-data-api / session-display / tool-execution-linking）\n已 sync 回主 spec。tasks 4.5 视觉验收 deferred（sandbox classifier outage 挡 e2e\n截图，已用 8 个 DOM 级 vitest 断言兜底），随 change 冻结。\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-30T00:39:16+08:00",
          "tree_id": "88eb67010bc0f7a30a3057be9afc010674aa4374",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/bedec62db6d23337c54ae11f83eb46421251aeb3"
        },
        "date": 1780072960726,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 114.4,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1132,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4865,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 1.552,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.341,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 46.6,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 34.03,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 296.8,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1231,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3241,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3163,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 39600,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 62.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 63.69,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 645,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6463,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 221.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 2199,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 67.15,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 661,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 6619,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 128.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1295,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 7.34,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 73.18,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 8579,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 912.9,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8974,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 902,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 48.61,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 510.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 92.75,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 944.6,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9493,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 183.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1284,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12440,
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
          "id": "d8a48c1445a3dcd952f7949836da1a7213765fc4",
          "message": "chore: Launch 准备 — README i18n + CHANGELOG + 截图 + 发布文案 (#414)\n\n* chore: Launch 准备 — README i18n + CHANGELOG + 截图 + 发布文案\n\n- README.md 改为英文主（面向 HN/Reddit Launch 受众）\n- 新增 README.zh-CN.md 中文版，顶部互相链接切换\n- 新增 \"Why\" 段 + Problem/Solution 对比表突出核心价值\n- 新增 CHANGELOG.md（追溯 v0.5.0 → v0.5.14，Keep a Changelog 格式）\n- 新增 docs/assets/ 三张高质量深色主题截图（Playwright 自动化）\n- 新增 docs/launch-materials.md（HN/Reddit 发帖草稿 + demo GIF 规格）\n- gh repo edit 设置 description / homepage / 7 topics\n\nCloses #392 (README/截图/分发素材部分; GIF 录屏 + 体验修复 + 社交发帖需手动)\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* chore: remove launch-materials.md from repo\n\n发布文案草稿不适合入库，改为私下维护。\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-30T09:41:20+08:00",
          "tree_id": "90eef22ad572f5247cb7180a41ea19c278000e15",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/d8a48c1445a3dcd952f7949836da1a7213765fc4"
        },
        "date": 1780105486051,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 206.6,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1112,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4892,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.872,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 9.126,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 41.18,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 32.92,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 294,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1183,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3112,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3100,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 40710,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 69.75,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 60.11,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 619.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6143,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 192.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1928,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 55.29,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 557,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5590,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 116.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1177,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.501,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 64.76,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 10530,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 1012,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 10720,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 1164,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 46.92,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 495.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 93.13,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 934.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9395,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 204,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1421,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 13360,
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
          "id": "d1e3a1729309055d753a53bd8f0c8dd6ea075888",
          "message": "fix(command-palette): make search and project list group-aware (#415)\n\n* fix(command-palette): make search and project list group-aware (#387)\n\nCommandPalette was passing selectedGroupId to search_sessions (which\nexpects a single worktree project_id) and using listProjects() for its\nproject list (which returns worktree-level entries). Multi-worktree\nusers got empty search results and semantic mismatch on project select.\n\nChanges:\n- Backend: add search_group_sessions IPC command + HTTP route that\n  traverses all worktrees in a repository group (global mtime order)\n- Backend: add SessionSearcher::search_across_projects method\n- Frontend: CommandPalette now uses loadProjectData().projects (group-\n  summarized) and searchGroupSessions() instead of listProjects/\n  searchSessions\n- Contract tests updated (52→53 commands)\n\nCloses #387\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* chore(opsx): archive command-palette-group-aware\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(command-palette): address review findings\n\n- Critical: eliminate race condition by deriving SearchConfig from\n  fs.kind() instead of a separate active_fs_and_policy() await\n- Important: set is_partial=true when all worktree dirs fail to list\n- Important: fix assertion message (52→53)\n- Important: add 4 unit tests for search_across_projects covering\n  mtime ordering, missing dir skip, all-fail partial, single-wt degenerate\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix: add backticks for clippy doc-markdown lint\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-30T10:45:30+08:00",
          "tree_id": "60f7489bd19fa17ac00f1bac214af60d5eb13075",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/d1e3a1729309055d753a53bd8f0c8dd6ea075888"
        },
        "date": 1780109346118,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 115.4,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1117,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4899,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 1.682,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 9.196,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 44.54,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.91,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 299.1,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1196,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 4468,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 4501,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 40290,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 90.63,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 59.11,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 609.3,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6069,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 196.9,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1966,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 54.04,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 548,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5455,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 118.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1199,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.501,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 64.76,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 9454,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 957.6,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9295,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 1007,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 50.57,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 536.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 94.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 937.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9489,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 226.7,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1334,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12910,
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
          "id": "a84ab3e5fade4f3fa6fb510064be6c5e5b3333c4",
          "message": "feat(workflow): WorkflowAgent session_id + get_workflow_agent_trace IPC (#397) (#423)\n\n* feat(workflow): WorkflowAgent session_id + get_workflow_agent_trace IPC\n\n为 workflow 子代理下钻提供数据基础（Epic #397 PR 4）：\n- WorkflowAgent 新增 session_id 字段，manifest/journal 路径均填充\n- 新增 get_workflow_agent_trace IPC + HTTP route 懒加载子代理对话\n- 前端类型同步 + tauriMock + transport 映射\n- ipc_contract / http_contract / manifest 单测覆盖\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* fix: address codex review — path traversal guard + TOCTOU elimination\n\n- Add is_safe_path_component() validation for user-supplied path params\n- Remove redundant exists() checks, handle NotFound at parse time\n- Add comment explaining is_sidechain=false rationale\n- Fix spec scenario example (was showing double agent- prefix)\n- Rename session_id param to parent_session_id for clarity\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* chore(opsx): archive workflow-subagent-pool-scan\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* fix: critical file-not-found bug + security hardening\n\n- Replace dead-code string match (\"No such file\" never matches) with\n  pre-parse fs.exists() check — fixes spec violation where missing\n  JSONL returned 500 instead of empty Vec\n- Add tracing::debug/error for diagnostic trail\n- Harden is_safe_path_component: reject null bytes and bare \".\"\n- Sanitize read_dir error to not leak filesystem paths\n- Check target file exists (not just session_dir) to prevent\n  first-match directory collision\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 <noreply@anthropic.com>",
          "timestamp": "2026-05-30T15:50:45+08:00",
          "tree_id": "13938bb2d6624fa73b859efd64bc83d67004f2b7",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/a84ab3e5fade4f3fa6fb510064be6c5e5b3333c4"
        },
        "date": 1780127645528,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 114.7,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1116,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4694,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.872,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 9.166,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 46.62,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 32.24,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 284.9,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1206,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 2910,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2792,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 37700,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 67.11,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 58.68,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 600.9,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 5968,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 204.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 2040,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 55.08,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 557.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5578,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 118.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1188,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.501,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 64.76,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 9475,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 953,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9525,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 1037,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 46.85,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 495.9,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 92.22,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 924.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9284,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 207.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1402,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 13160,
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
          "id": "6ff3c2bc36616cc3e757861a115f47d92a89a442",
          "message": "feat(workflow): agent chip drilldown — click to view full conversation (#397) (#425)\n\n* feat(workflow): agent chip drilldown — click to view full conversation (#397)\n\nWorkflowCard agent chips now support click-to-expand:\n- Click a chip with sessionId to lazy-load its full conversation trace\n- Uses getWorkflowAgentTrace IPC (from PR #423) for data fetching\n- Renders via ExecutionTrace (same component as SubagentCard)\n- Visual feedback: hover highlight, active border, expand chevron\n- Loading/empty states handled gracefully\n\nThis is Epic #397 PR 5 — the final piece of workflow visualization.\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* fix: scope isLoadingAgentTrace to current agent + derive display items\n\nAddresses codex review:\n- isLoadingAgentTrace only cleared if still viewing same agent (rapid\n  switch no longer shows premature \"No trace data\")\n- buildDisplayItemsFromChunks cached via $derived to avoid redundant\n  recomputation on unrelated state changes\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 <noreply@anthropic.com>",
          "timestamp": "2026-05-30T16:16:45+08:00",
          "tree_id": "6003fea5e080e8ce10daa01c5c167fe9b66c64b4",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/6ff3c2bc36616cc3e757861a115f47d92a89a442"
        },
        "date": 1780129212076,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 113.5,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1107,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 5733,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.862,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.272,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 45.88,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.92,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 295.5,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1243,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3177,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2937,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 40200,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 61.82,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 64.48,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 664.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6353,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 195.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1977,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 51.66,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 524.3,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5273,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 128.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1284,
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
            "value": 8675,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 920.1,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8642,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 865.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 49.56,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 523.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 93.18,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 933.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9460,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 201.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1321,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12710,
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
          "id": "156b2f80ab10903948529c1c93470fef94c9586f",
          "message": "fix(workflow): coalesce file events + cache subagent scan (idle CPU 32% → <3%) (#424)\n\n* fix(workflow): coalesce workflow file events + cache subagent scan to fix 32% idle CPU\n\n问题：一个 workflow run 有 102 个 agent 文件，per-path debounce 导致 100ms 后\nflush 出 103 个独立 FileChangeEvent 全指向同一 session。每个 event 触发\nget_session_detail 全量管线含 scan_subagent_candidates_cross_project（276 次\nread_dir）。\n\n修复：\n1. watcher 层 path coalescing：notify 回调中将同一 <run_id>/ 下的所有文件\n   合并为 journal.jsonl 单一 debounce key（103 events → 1）\n2. subagent scan 5s TTL cache：workflow 运行期间 subagents/ 目录结构不变\n   （新文件在更深层 workflows/<run_id>/），cache hit 跳过跨目录全量 scan\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* refactor: address PR review — named SubagentScanCache + eviction + path.parent() opt\n\n- 用命名 struct SubagentScanCache 替代裸 HashMap（对齐 ParsedMessageCache 模式）\n- insert 时 retain 过期条目 + cap=32 防内存泄漏\n- coalesce 路径用 path.parent()?.join() 减少一次分配（code-simplifier）\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* style: rustfmt\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* revert: remove SubagentScanCache — coalesce alone is sufficient\n\nP0-1 coalesce 把 103 events → 1 已消灭事件风暴。之后每 250ms 最多 1 次\nget_session_detail，scan 的 276 次 read_dir（大部分 NotFound 几十微秒）\n总共 5-10ms，完全可接受。\n\ncache 引入的 TTL 选择、内存管理、脏数据风险复杂度不值得。\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-30T18:58:08+08:00",
          "tree_id": "e7989b5470c12a3af939c19114a5e8a2048c2412",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/156b2f80ab10903948529c1c93470fef94c9586f"
        },
        "date": 1780138893113,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 112.9,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1100,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 6533,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.841,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.271,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 46.6,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.78,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 293.6,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1269,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 2813,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2967,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 40000,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 60.34,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 62.73,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 629.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6322,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 192.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1938,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 51.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 522.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5268,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 128,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1288,
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
            "value": 8662,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 983.5,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8661,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 912.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 48.93,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 519.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 92.43,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 936.7,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9372,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 187,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1291,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12440,
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
          "id": "26f21a94bc727e784742a500b8f1c445c5bf5196",
          "message": "chore(release): 0.6.0 (#426)\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 <noreply@anthropic.com>",
          "timestamp": "2026-05-30T22:40:10+08:00",
          "tree_id": "e431184cd8812601b9d52d0106962d667cb5acee",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/26f21a94bc727e784742a500b8f1c445c5bf5196"
        },
        "date": 1780152208185,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 115.9,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1114,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4689,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.872,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 10.06,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 41.26,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.38,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 299.3,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1197,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3318,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3297,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 37560,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 72.01,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 60.48,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 620.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6163,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 192.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1943,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 55.45,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 561.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5617,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 118.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1196,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.501,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 64.76,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 9240,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 998.5,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9491,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 1139,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 48.13,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 501.2,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 90.77,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 903.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9166,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 224.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1388,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12520,
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
          "id": "2b53107dc7b23f1c82934669e1b018b39be8ab64",
          "message": "fix(cli): self-update 不指定版本走 releases/latest 重定向探测（绕开 API rate limit） (#427)\n\n* fix(cli): self-update probes latest tag via releases/latest redirect\n\n`cdt self-update` without `--version` always failed with \"GitHub API rate\nlimit exceeded\" because fetch_latest_tag hit api.github.com, whose\nunauthenticated quota (60 req/hr per egress IP) is trivially exhausted on\nshared NAT IPs. Passing `--version` worked because it skips the API and\nbuilds the asset download URL directly.\n\nProbe the latest tag via the 302 redirect of\ngithub.com/<repo>/releases/latest (Location: .../releases/tag/vX.Y.Z),\nwhich does not consume the API quota — verified working on a machine\nalready at 0/60. Fall back to the REST API (5000/hr with a token) when the\nredirect probe fails or the repo has no release (404).\n\nbuild_client now takes an explicit redirect policy: none for the tag probe\n(to read the 302 Location), default for asset download (asset URLs 302 to\nobjects.githubusercontent.com and must be followed).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(cli): GET instead of HEAD for tag probe + strip query/fragment\n\nAddress codex review findings:\n- HEAD can be rewritten to GET and auto-followed by enterprise transparent\n  proxies, dropping the 302+Location and silently degrading back to the API\n  path. GET (body unconsumed) is less likely to be tampered with.\n- Strip query string / fragment from the redirect Location before parsing\n  the tag, so a future `?utm=...` suffix can't leak into the version string.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-30T23:51:47+08:00",
          "tree_id": "2660387b2ed9459632b9c249546606c02ce68c65",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/2b53107dc7b23f1c82934669e1b018b39be8ab64"
        },
        "date": 1780156511492,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 116.9,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1128,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 5085,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 1.677,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 10.01,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 42.64,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 32.76,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 287.9,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1199,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3084,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3263,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 38310,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 67.98,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 59.02,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 610.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6237,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 195.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1979,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 56.04,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 558,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5622,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 115,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1158,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.501,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 64.76,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 9479,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 1138,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9451,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 1014,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 50.53,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 534.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 93.15,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 940.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9461,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 203.9,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1347,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 13270,
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
          "id": "cfc2d9ec9a0fd46148c36d688070570b93520785",
          "message": "fix(workflow): restrict failed_by_heuristic to completed agents (#413) (#433)\n\nRunning/pending agents with tokens=0 and toolCalls=0 are normal (just started),\nnot failed. Only apply the \"no output = failed\" heuristic to completed/done agents.\n\nCloses #413\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-31T09:09:37+08:00",
          "tree_id": "1adbb2964ebbc7006f2c00df59be555bb0276038",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/cfc2d9ec9a0fd46148c36d688070570b93520785"
        },
        "date": 1780189995423,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 118,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1152,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4720,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.871,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 9.327,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 42.32,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.46,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 294,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1218,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3221,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3202,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 37670,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 68.21,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 60.35,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 613.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6071,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 194,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1942,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 55.18,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 559.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5584,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 115.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1164,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.501,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 64.76,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 9261,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 945.1,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9525,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 1032,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 47.24,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 502.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 93,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 931,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9325,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 207,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1349,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12680,
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
          "id": "3541d5dba6f04966730003d8a23fa5fc5332ca9c",
          "message": "fix(discover): fallback group matching when git status changes #369 (#432)\n\n* fix(discover): fallback group matching when git status changes (#369)\n\nWhen a project directory transitions from non-git to git repo (e.g. git init),\nthe group_id changes from encoded-path to git-common-dir form. The frontend may\nstill hold the stale encoded-path ID causing a 404 and empty session list.\n\nAdd fallback matching: when exact group_id lookup fails, check if it matches\nany worktree.id within a group (worktree.id == project.id, which is the old\nencoded-path form).\n\nCloses #369\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* refactor: use swap_remove instead of into_iter().nth() in fallback\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix: add tracing::debug on fallback + priority test\n\nAddress PR review findings:\n- Log when worktree-id fallback triggers (project pattern for fallback paths)\n- Add test verifying exact match takes priority over worktree fallback\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-31T09:25:24+08:00",
          "tree_id": "a78f8e55df10c29f39df4f4cc912d20e7cc8987d",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/3541d5dba6f04966730003d8a23fa5fc5332ca9c"
        },
        "date": 1780190929663,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 115.5,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1120,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4881,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 1.717,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 10.05,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 42.16,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.61,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 304.8,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1264,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3245,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3357,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 38160,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 68.86,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 59.44,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 617.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6059,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 194.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1960,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 55.46,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 561.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5597,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 114.9,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1156,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.501,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 64.76,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 10550,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 998.5,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9295,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 1152,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 46.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 502.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 91.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 912,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9180,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 204.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1462,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12960,
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
          "id": "ac1d2391a6bd5aee8c2fa5ded24b84e7e5cb809a",
          "message": "fix(config): use char-boundary-safe truncation to prevent CJK panic (#431)\n\nThe preview truncation in error_trigger_checker.rs (`&str[..200]`) and\ntruncate_message in detected_error.rs (`&str[..500]`) panic when the\nbyte index lands inside a multi-byte UTF-8 character (e.g. CJK text).\n\nFix: use `is_char_boundary()` to find the nearest valid boundary before\nslicing. This is O(1) (at most 3 byte walk-back) with zero perf impact.\n\nToken estimation logic (len()/4) is intentionally left unchanged — it\nhappens to give reasonable results for the dominant use case and changing\nit has broader behavioral implications that need separate analysis.\n\nCloses #393\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-31T09:43:44+08:00",
          "tree_id": "e79568305049b6d4e6161b8c4bc1cda37bed0e8a",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/ac1d2391a6bd5aee8c2fa5ded24b84e7e5cb809a"
        },
        "date": 1780192023908,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 114.5,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1112,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4762,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.861,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.461,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 46.45,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 34.43,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 295.8,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1240,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 2776,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2895,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 42370,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 59.34,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 62.87,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 636.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6386,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 219,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 2208,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 65.35,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 661.3,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 6595,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 132.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1331,
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
            "value": 8518,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 932.5,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8682,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 898.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 49.39,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 520,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 91,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 922.7,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9228,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 193.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1294,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12370,
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
          "id": "0793de0746bc5557545aad279149f90f7e995819",
          "message": "fix(perf): restore Unchanged short-circuit for ongoing sessions (#429)\n\n* fix(perf): restore Unchanged short-circuit for ongoing sessions\n\n前端对 ongoing session 强制 fpToSend=null 击穿了后端 locate_session_file\n的 fingerprint 短路，使 workflow 运行态每个 coalesced file-change event\n都跑完整 get_session_detail pipeline（~60-150ms/次），导致主进程 CPU 20%+。\n\n改法：把 is_stale（mtime 距今 ≥5min）编入 IPC fingerprint（v1→v2），\n使 stale 翻转由 fingerprint 变化自然触发重算；前端 always 传 fingerprint，\nongoing session 在\"父 jsonl 未变、仅 subagent/journal 写\"这类最高频 flush\n直接 Unchanged 短路，零 UX 变化。\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix: address PR review — saturating_sub + named constant + tests\n\n- `now_ms - ms` → `now_ms.saturating_sub(ms)` 防 debug panic\n- 硬编码 300_000 → 引用 `STALE_SESSION_THRESHOLD` 保持单源\n- 注释修正：承认 stale 翻转依赖外部 file-change 触发（既有行为）\n- 新增 2 个 unit test 覆盖 stale-bit fingerprint 翻转\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* style: cargo fmt\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-31T09:55:59+08:00",
          "tree_id": "c3f2e3261cda5171c1c35931dd8936d756f5c99d",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/0793de0746bc5557545aad279149f90f7e995819"
        },
        "date": 1780192775488,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 115.6,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1126,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4918,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.911,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 10.1,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 43.13,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.9,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 298.5,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1227,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 2857,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3071,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 37380,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 67.08,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 63.09,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 644.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6354,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 197.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1988,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 55.21,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 559.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5585,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 115.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1181,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.501,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 64.76,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 10520,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 1015,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9650,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 1135,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 47.79,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 506.6,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 92.63,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 928.6,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9346,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 226.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1334,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12710,
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
          "id": "fc1ac1eae75794e7f841d296799cc3c7d255fcfc",
          "message": "feat(session-display): render queued user messages inline in AI turn (#430)\n\n* feat(session-display): render queued user messages inline in AI turn\n\nClaude Code 2.1.x records mid-turn user input as `type:\"attachment\"` +\n`attachment.type:\"queued_command\"`. Previously these were silently dropped\nby the parser, causing user interjections to disappear from session replay.\n\nChanges:\n- cdt-parse: recognize queued_command attachments as user messages\n- cdt-core: add SemanticStep::UserMessage variant + ParsedMessage.is_queued_input\n- cdt-analyze: inline embed queued messages into AIChunk semantic_steps\n  at their precise timeline position (no turn break)\n- Frontend: render as BaseItem disclosure (MESSAGE_SQUARE icon, \"User\" label)\n  identical to Output rows\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* test: add IPC contract + displayItemBuilder coverage for UserMessage step\n\nCodex review found tasks.md 5.1 claimed done but IPC contract test was\nmissing. Also adds all missing SemanticStep variants (SubagentSpawn,\nInterruption) to the enum tag test, and a frontend unit test for the\nuser_message display item.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* chore(opsx): archive render-queued-user-message\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* feat(session-display): use user silhouette icon for queued User step\n\nReplace MESSAGE_SQUARE (same as Output rows, causing visual confusion)\nwith a dedicated USER_ICON (lucide user silhouette). The human figure is\nthe only \"alive\" shape in a stream of abstract tool icons, making user\ninterjections instantly recognizable during scan.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-31T10:06:58+08:00",
          "tree_id": "95ab0c3deaab35327315bee0987c349c40245915",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/fc1ac1eae75794e7f841d296799cc3c7d255fcfc"
        },
        "date": 1780193422502,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 117.8,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1137,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 5069,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.861,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 9.837,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 41.79,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.5,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 288.3,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1209,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 2887,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3000,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 38350,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 69.24,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 60.97,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 621.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 5997,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 196.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1972,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 58.63,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 594.3,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5915,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 118.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1189,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.501,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 64.76,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 10110,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 1013,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 10020,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 1085,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 47.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 506,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 96.76,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 973.2,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9753,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 207.2,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1529,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 14490,
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
          "id": "c1e506decc84d5a37f402b160cf2d1f98e375222",
          "message": "feat(jobs): Background Jobs Panel Phase 1 & 2 (#422)\n\n* feat(jobs): Background Jobs Panel Phase 1 & 2 (#421, #420)\n\n完整实现后台任务面板——对齐 `claude agents` 原生 GUI 等价物。\n\nPhase 1 (#421):\n- cdt-core: BackgroundJob/JobState/JobSummary/JobsResponse 类型 + 分组/badge 逻辑\n- cdt-watch: FileWatcher 扩展 jobs_dir + route_event 严格过滤 state.json\n- cdt-api: list_jobs IPC + HTTP route + broadcast bridge\n- src-tauri: command wrapper + invoke_handler\n- UI: JobsView + JobRow + TitleBar badge + session 跳转\n- 降级: jobs/ 不存在时零 UI 暴露\n\nPhase 2 (#420):\n- 实时推送: jobs-update event → 前端 listen → 自动刷新\n- Badge 实时更新: 后端计算 badge 色 + 数字，前端直接消费\n- Command Palette: \"Open Jobs\" / \"Background Jobs\" 注册\n- Stop 操作: stop_job IPC → `claude stop <daemonShort>`\n\n测试覆盖:\n- vitest 45 pass (badge/分组/state→color/projectId提取/stop)\n- Playwright 7 pass (tab/分组/展开/PR chip/badge/降级/空态)\n- IPC contract 143 pass (含 list_jobs camelCase 验证)\n- HTTP contract 3 pass (含 /api/jobs route)\n\nCloses #421, Closes #420\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix(ci): add list_jobs_from_dir to fs-direct-calls allowlist\n\njobs/ 目录永远 Local-only，不参与 SSH context。\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix(http): remove duplicate /api/jobs route causing router panic\n\naxum panics on duplicate route registration. Removed the redundant\nroute/handler pair added during merge (kept the backend-engineer's\nversion at the canonical telemetry section position).\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix(jobs): trait dispatch + real data parsing + visual redesign\n\n- Move list_jobs/stop_job from `impl LocalDataApi` to `impl DataApi for LocalDataApi`\n  so HTTP route via `dyn DataApi` trait object hits the real implementation\n- Align BackgroundJob struct with real state.json format: children is\n  Option<Vec<JobChild>> (nullable), inFlight is Option<JobInFlight> struct\n- Replace CPU icon with square-terminal (more intuitive for bg tasks)\n- Redesign JobsView/JobRow CSS: proper spacing rhythm, expand area with\n  border instead of raised bg, chevron hover state, label typography aligned\n  to DESIGN.md spec (11px/600/0.04em)\n- Add __cdtReady signal in App.svelte onMount for stable Playwright timing\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* fix(jobs): address review findings — error handling + type safety\n\n- stop_job: safe char-based truncation instead of byte slice (panic risk)\n- stop_job: distinguish NotFound (binary missing) from other IO errors\n- list_jobs_from_dir: log warnings on parse failure and read_dir errors\n- Frontend: read jobsDirExists from backend response, not hardcode true\n- Frontend: add jobsDirExists to ListJobsResult TypeScript type\n- Frontend: stopJob shows inline error on failure instead of fire-and-forget\n- Frontend: subscribe catch logs warning instead of empty swallow\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* fix(jobs): done/failed/stopped jobs go to Completed even with PR children\n\nAlign grouping logic with `claude agents` CLI behavior: terminal states\nalways land in Completed group. \"Ready for review\" only applies when the\njob is still active (working/idle) but has produced a PR.\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* fix(jobs): redesign interaction model + remove green badge\n\n- Row no longer clickable (no accidental navigation)\n- Explicit \"打开 session →\" link per row instead of whole-row click\n- PR chip is a real <a> link with hover underline\n- Stop button only on working rows, visible inline\n- Remove green badge (only red=failed, amber=blocked interrupt user)\n- Completed group uses opacity 0.65 to visually recede\n- No italic text, no chevron, no expand — all info visible in 2 lines\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* fix(jobs): done/idle/stopped dot color → muted (not green)\n\nCompleted jobs should visually recede, not draw attention with\ngreen dots. Only working (blue) and blocked (amber) use saturated color.\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* feat(jobs): bridge jobs-update event to HTTP SSE for real-time updates\n\nWithout this, the HTTP server mode (browser ?http=1) never receives\njob state changes — users see stale data until manual refresh.\n\n- Add PushEvent::JobsUpdate variant\n- Add spawn_jobs_bridge in bridge.rs\n- Wire jobs_rx into spawn_event_bridge (7th param)\n- Frontend: use subscribeEvent(\"jobs_update\") instead of Tauri-only listen\n- TauriTransport: add \"jobs-update\" listener for desktop parity\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* style(jobs): polish action links + spacing + opacity per DESIGN.md\n\n- Action links: blue color for session link, danger for Stop, hover underline\n- Terminal rows: opacity 0.55 (more faded), hover 0.9\n- Group spacing: 20px between groups\n- Transitions: 150ms ease-out aligned to design system\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* fix(jobs): use --font-sans + remove ghostly opacity on completed rows\n\n- Add font-family: var(--font-sans) to .jobs-view container\n- Replace opacity-based fading with muted text color for completed jobs\n  (opacity made the whole row look disabled/broken)\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* fix(jobs): name is session link, Stop hidden until hover, always show group label\n\n- Job name is the session link (click → open session, hover → underline)\n- Remove dedicated actions row (saves vertical space)\n- Stop: grey text, only visible on row hover (not red, not prominent)\n- Always show group label even with single group (user needs state context)\n- Completed jobs: name uses muted color, no opacity hack\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* fix(jobs): tempo-based classification + needs field + Working/Idle split\n\n- tempo=active → unconditionally Working (aligns with CLI status=busy)\n- tempo=blocked → Blocked/NeedsInput\n- tempo=idle → respects state field (Completed for done/failed/stopped)\n- Working state no longer routes to ReadyForReview (only Idle+PR does)\n- Add `needs` field to BackgroundJob/JobSummary for blocked action prompts\n- Frontend: show needs text in amber when job is blocked\n- Update mock data, tests, contract test for new field\n\nVerified against `claude agents --json` and daemon control.sock protocol.\nResearch confirmed tempo values: active|idle|blocked (daemon real-time signal).\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* fix(jobs): handle null needs field + guard blocked override on terminal states\n\nReview findings:\n- HIGH: daemon writes `\"needs\": null` for non-blocked jobs → serde fails →\n  job silently disappears. Fix: deserialize_nullable_string (null → \"\").\n- IMPORTANT: tempo=blocked could clobber terminal states (done/failed/stopped)\n  during race. Fix: guard with !matches!(terminal states).\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* fix(jobs): done+PR → ReadyForReview instead of Completed\n\nCLI checks PR children BEFORE routing success to Completed.\nA done job with a PR means \"work finished, PR awaits user review\".\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* fix(jobs): align with CLI — done/idle unconditionally Completed (plan B)\n\nWithout GitHub API we cannot determine PR checks/review status,\nso ReadyForReview is unreachable for now. All non-working, non-blocked\njobs go to Completed — matching what `claude agents` shows.\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* style: cargo fmt\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* feat(jobs): add delete job + visual hierarchy for completed jobs\n\n- Backend: `delete_job` (calls `claude rm`) + `delete_completed_jobs` (bulk)\n- HTTP routes: DELETE /api/jobs/{id} + DELETE /api/jobs/completed\n- Frontend: optimistic removal + two-step inline confirm (click → \"确认?\" → execute)\n- Visual: completed+PR keeps normal opacity; completed without PR fades (0.55);\n  failed never fades\n- JobsView: \"Clear\" button in Completed group header (bulk delete with confirm)\n- Design decisions D8/D9/D10 documented in openspec change\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* fix(jobs): address PR review findings\n\n- Add \"idle\" to isTerminal check (idle jobs in Completed group now get\n  delete button and faded visual)\n- Replace empty catch blocks with console.error logging\n- Add onDestroy timer cleanup in JobRow + JobsView\n- Add try/finally to stopJob for consistent refresh on error\n- Add tracing::warn! for per-job delete failures in bulk operation\n- Fix trait doc to match implementation (deletes all terminal, not just no-PR)\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* fix(ui): add scrollbar-gutter: stable to JobsView\n\nRequired by scrollbarGutter.guard.test added in #428.\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* style(jobs): impeccable critique fixes\n\n- Replace <a href=\"#\"> with <button> for job name (a11y)\n- Remove amber text doubling on needs detail (Status Owns Color Rule)\n- Add focus-visible ring on action buttons\n- Bump group-count opacity 0.5 → 0.7 for readability\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n* chore: remove accidental screenshot\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-31T10:43:33+08:00",
          "tree_id": "504bc05d8a4df2fd91485aa2ef761b2263e67291",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/c1e506decc84d5a37f402b160cf2d1f98e375222"
        },
        "date": 1780195622560,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 112.7,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1110,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4843,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.842,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.231,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 47.62,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.89,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 292.4,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1221,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3358,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3213,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 40180,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 61.38,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 67.89,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 676.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6773,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 218.3,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 2193,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 65.47,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 660.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 6605,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 128.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1292,
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
            "value": 8651,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 908.2,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8866,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 980.6,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 48.62,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 507.9,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 95.39,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 962.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9640,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 190.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1328,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12880,
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
          "id": "8f980a101aa6650ca42a1084e59c37a3fafeb16d",
          "message": "chore(release): 0.6.1 (#434)\n\n* chore: integrate CHANGELOG into release pipeline\n\n- release-bump.sh auto-converts [Unreleased] to versioned heading + date + links\n- opsx-apply-cadence adds step 8: write CHANGELOG entry per user-visible PR\n- release-runbook skill updated with CHANGELOG automation docs\n- CHANGELOG [Unreleased] populated with 0.6.1 changes\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix(server-mode): add missing jobs_rx to spawn_event_bridge\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* chore(release): 0.6.1\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-31T11:07:19+08:00",
          "tree_id": "9c19c08b873f132bc70be40cd73d06e0b6a8fecd",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/8f980a101aa6650ca42a1084e59c37a3fafeb16d"
        },
        "date": 1780197041380,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 113,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1106,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4819,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 1.561,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.101,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 47.8,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 34.26,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 297.1,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1225,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3200,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2975,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 39780,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 62.24,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 62.77,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 636,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6428,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 190.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1921,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 51.92,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 525.9,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5262,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 129.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1299,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 7.69,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 76.66,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 8689,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 904.6,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8577,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 882.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 48.2,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 511.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 94.77,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 963.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9620,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 187.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1366,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12800,
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
          "id": "6545c76c0996a56fa9153be31b942b3a9bf5ab88",
          "message": "fix(jobs): resolve stop button, stale status, and empty name issues (#435)\n\n* fix(jobs): resolve stop button, stale status, and empty name issues\n\nThree bugs fixed:\n1. Stop/delete buttons: resolve claude CLI full path (macOS GUI app\n   PATH doesn't include npm bin dirs). Searches CLAUDE_CLI_PATH env,\n   which, known platform paths, and ~/.npm/_npx glob.\n2. Stale status: add 30s polling fallback when Jobs panel is visible\n   (file-watcher events can be missed with no recovery path).\n3. Empty job name: fallback to intent field when name is not yet set\n   by daemon auto-naming. Also fix tab label when opening job session.\n4. Terminal state preserved: state=failed/done/stopped no longer\n   overridden by stale tempo=active (daemon may not clean up tempo\n   on unclean exit).\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* docs: add CHANGELOG entries for jobs panel fixes\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-31T12:58:50+08:00",
          "tree_id": "a6a425b559bc172f3d160e7782f54f60f4322948",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/6545c76c0996a56fa9153be31b942b3a9bf5ab88"
        },
        "date": 1780203737434,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 112.6,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1061,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4995,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.844,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.081,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 57.9,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 30.87,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 254.5,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1155,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 2466,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2508,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 42050,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 47.94,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 53.89,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 541.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 5421,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 194.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1948,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 57.13,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 574.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5772,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 99.32,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 997.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.897,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 68.75,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 7631,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 795,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 7719,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 871.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 44.79,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 595.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 88.28,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 868.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 8672,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 183.9,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1224,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 11950,
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
          "id": "07a89396bb4cef9103417df31cdffab655c38e33",
          "message": "fix: remove accidentally committed node_modules and update .gitignore (#436)\n\nRoot-level node_modules was not in .gitignore, causing a vitest cache\nfile to be tracked. Add node_modules to .gitignore and remove the file.\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-31T13:19:06+08:00",
          "tree_id": "334a0c9041b47f4bb21d637d41d6706745439856",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/07a89396bb4cef9103417df31cdffab655c38e33"
        },
        "date": 1780204955269,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 115.2,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1117,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4621,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.872,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.795,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 46.2,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 32.79,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 291.3,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1177,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3326,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3007,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 37590,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 65.75,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 61.51,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 617.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6091,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 195.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1963,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 53.93,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 547.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5468,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 117.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1183,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.812,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 67.84,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 9406,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 1122,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9552,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 941.6,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 47.54,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 499.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 97.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 977.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9793,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 212.2,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1384,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 13190,
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
          "id": "676bb1db8787cc52e5546adcfda6778ac08724b7",
          "message": "chore(release): 0.6.2 (#437)\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>",
          "timestamp": "2026-05-31T13:30:44+08:00",
          "tree_id": "d7ab07b490a88e32a05efe4e19f89629500b562a",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/676bb1db8787cc52e5546adcfda6778ac08724b7"
        },
        "date": 1780205649599,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 115.7,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1114,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4904,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 1.537,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.242,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 47.48,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.87,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 288.7,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1216,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3264,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3159,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 39700,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 64.41,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 62.25,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 629.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6325,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 195.3,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1951,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 51.86,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 522.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5269,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 131.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1324,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 7.69,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 76.66,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 8706,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 936.3,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9041,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 1002,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 50.44,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 523.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 97.99,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 990.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9920,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 194.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1402,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 13110,
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
          "id": "d5ebbcc5da1e38fbcd6a9c78de0b5a61c67b32d2",
          "message": "feat(ui): add context window usage progress bar to ContextPanel (#441)\n\n* feat(ui): add context window usage progress bar to ContextPanel\n\nDisplay a progress bar showing current context window utilization\n(input_tokens / model_context_limit) in the ContextPanel header.\nColor-coded by threshold: green (<50%), amber (50-80%), red (>80%).\n\nCloses #394\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix: handle 1M context models in token progress bar\n\nModels with extended context (e.g., claude-opus-4-6[1m]) now correctly\nreport 1,000,000 token limit instead of defaulting to 200K.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-31T17:32:00+08:00",
          "tree_id": "4b66122e82a8ea6308bfbf7237afdc7af65097cf",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/d5ebbcc5da1e38fbcd6a9c78de0b5a61c67b32d2"
        },
        "date": 1780220126915,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 112.9,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1111,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4807,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 1.516,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.411,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 47.53,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.21,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 295.6,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1229,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3223,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2871,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 39780,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 61.88,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 62.16,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 629.3,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6308,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 191.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1927,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 51.56,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 525,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5410,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 131.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1322,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 7.691,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 76.66,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 8812,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 984.8,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8692,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 898.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 49.99,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 527.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 98.16,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 983.2,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9857,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 194.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1373,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 13270,
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
          "id": "e6af4ec195a6170ccdaface283dd51a8ad1f05de",
          "message": "fix(perf): cap grouper concurrency + add groups cache (#439) (#442)\n\n* fix(perf): cap grouper concurrency + add groups cache (#439)\n\nCold-start CPU spike (78 threads / 32%) caused by WorktreeGrouper's\nunbounded join_all dispatching ~54 spawn_blocking tasks simultaneously.\n\n- Add Semaphore(8) to group_by_repository limiting concurrent blocking\n  tasks, reducing peak thread count from ~54 to ≤8\n- Add generation-keyed groups cache (root_gen + ctx_gen + scan_inv_gen\n  + 10s TTL) to skip redundant grouper execution on repeated\n  list_group_sessions calls\n- Expose ProjectScanCache::invalidation_generation() as public API for\n  cache key composition\n\nBench: cold total 112ms (budget ≤150ms), grouper phase 5ms.\n\nCloses #439\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix: address codex race conditions in groups cache + CI rustfmt\n\n- Cache hit path: read generations AFTER active_fs_and_policy() to ensure\n  cached groups and (fs, ctx) belong to same generation snapshot\n- Cache write path: conditional write (compare-and-swap) — only store if\n  generations unchanged since computation started, preventing stale data\n  from being tagged as fresh\n- Fix import ordering in test file (CI rustfmt nightly)\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix: move struct def before statements to satisfy clippy items_after_statements\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* chore(opsx): archive cold-start-cpu-spike\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-31T17:38:25+08:00",
          "tree_id": "f500526e7afdb1903497cf65cdc87d996e1875fe",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/e6af4ec195a6170ccdaface283dd51a8ad1f05de"
        },
        "date": 1780220508547,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 115.8,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1132,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4769,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.861,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 9.757,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 40.76,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 36.29,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 315.2,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1298,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3163,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3219,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 37910,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 5.089,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 58.88,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 608.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6120,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 196.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1976,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 54.09,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 547.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5469,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 119.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1198,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.812,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 67.84,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 9316,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 995.8,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 10090,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 1117,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 47.59,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 503.7,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 94.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 956.7,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9704,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 209.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1354,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12890,
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
          "id": "a82513d9b6deb1645e745786980b760def0829b9",
          "message": "refactor(cdt-api): replace hand-rolled LRU with lru crate (#444)\n\n* refactor(cdt-api): replace hand-rolled LRU with lru crate\n\nMetadataCache, ParsedMessageCache, and SignatureCache all used identical\nHashMap+VecDeque manual LRU implementations (~50 lines each). Replace\nwith the `lru` crate already in workspace dependencies, cutting 115\nlines of maintenance surface while preserving identical LRU semantics.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix: preserve LRU no-bump semantics on signature mismatch\n\nlookup_with_known_signature should only promote the entry when\nsignature matches (original behavior). Use peek() to check first,\nthen get() to promote only on match.\n\nFound by codex review.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* style: use NonZeroUsize::MIN and deduplicate lookup_trust_cached\n\n- Replace verbose NonZeroUsize::new(1).unwrap() with NonZeroUsize::MIN\n- lookup_trust_cached now delegates to lookup (identical after lru migration)\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* style: cargo fmt\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-31T18:19:31+08:00",
          "tree_id": "6ef040e562b0f169a69cea01d7f66fc17860c60a",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/a82513d9b6deb1645e745786980b760def0829b9"
        },
        "date": 1780222982350,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 114.1,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1127,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 5112,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 1.452,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.291,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 39.8,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.82,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 295.4,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1235,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3170,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3249,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 40750,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 5.006,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 62.87,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 633,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6362,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 192.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1950,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 51.92,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 524.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5259,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 127.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1287,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 7.69,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 76.66,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 8575,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 887.8,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8671,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 854.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 55.36,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 583.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 97.76,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 979.2,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9817,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 192.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1341,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12920,
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
          "id": "742159192448e8b6672c856f6e76c99a761f64c8",
          "message": "perf: workflow lazy-loading — skeleton + on-demand detail (#443)\n\n* perf: workflow lazy-loading — skeleton in session detail, full detail on demand (#440)\n\nReplace resolve_workflow_items in get_session_detail with lightweight skeleton\ngeneration (one stat per workflow for status, zero journal/script reads). Full\nworkflow detail (agents, phases, tokens) is now fetched on demand via the new\nget_workflow_detail IPC when the WorkflowCard is expanded.\n\nBackend:\n- resolve_workflow_skeletons: stat manifest → status (completed/running), extract\n  name from script path, return WorkflowItem with detail_omitted=true\n- New get_workflow_detail Tauri command + HTTP route\n  GET /api/sessions/{session_id}/workflows/{run_id}\n- resolve_single_detail: public wrapper around existing resolve_single\n- WorkflowItem gains detail_omitted: bool field (serde skip_serializing_if false)\n\nFrontend:\n- WorkflowCard: lazy-load full detail on expand, poll every 3s for running\n  workflows, stop on collapse/terminal state/unmount\n- computeChunksFingerprint includes workflow skeleton status\n- getWorkflowDetail API function + transport HTTP mapping + mock handler\n\nPerformance: workflow running period session detail refresh drops from ~5-20ms/wf\n(read+parse journal+script) to ~0.1ms/wf (single stat). CPU during 7-agent\nparallel workflow execution drops from sustained 32%+ to normal idle levels.\n\nCloses #440\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix: completed workflows use full resolve from cache, not skeleton\n\nCompleted workflow manifests are immutable + cached by FileSignature,\nso full resolve is near-zero cost. Only running workflows need skeleton\nto avoid high-frequency journal reads.\n\nThis fixes:\n- Collapsed completed workflows showing \"0 phases · 0 agents\"\n- Phase/agent counts jumping when expanding\n- WorkflowCard showing blank on expand for completed workflows\n\nAlso hides phase/agent summary for skeleton items (detail_omitted=true)\nsince they have no meaningful counts to display.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix: workflow agent trace lookup continues searching across project dirs\n\nWhen a session exists in multiple project directories (e.g., main project\n+ worktree project), the agent JSONL files may only exist in one of them.\n\nPreviously, get_workflow_agent_trace broke out of the search loop after\nfinding ANY project with the session_dir, even if the agent file wasn't\nthere. Similarly, get_workflow_detail stopped at the first project with\na session dir even if workflow files were in a different project.\n\nFix: only break when the target file/directory is actually found.\nThis fixes \"No trace data\" for sessions visible from multiple projects.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix: use projectId for direct path lookup in workflow IPC commands\n\nget_workflow_detail and get_workflow_agent_trace now accept projectId\nand use it to construct the path directly (O(1)) instead of scanning\nall project directories (O(N) exists calls, problematic over SSH).\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* refactor: remove skeleton/lazy-loading, keep poll + projectId fix\n\nThe skeleton approach didn't address the actual hot path (fingerprint\nshort-circuits before workflow resolution runs). Revert to original\nresolve_workflow_items for session detail.\n\nWhat remains (the actual bug fixes):\n- get_workflow_detail IPC + 3s poll for expanded running WorkflowCards\n  (bypasses fingerprint, fixes \"agent status not updating\")\n- projectId direct path lookup (O(1), fixes \"No trace data\")\n\nRemoved:\n- resolve_workflow_skeletons (replaced by restored resolve_workflow_items)\n- detail_omitted field on WorkflowItem\n- computeChunksFingerprint workflow status (unnecessary, chunks change\n  naturally on workflow completion)\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* style: cargo fmt\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-31T20:04:15+08:00",
          "tree_id": "94cbf094b74ace2e82c7334d042f997d59167437",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/742159192448e8b6672c856f6e76c99a761f64c8"
        },
        "date": 1780229271792,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 114.5,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1131,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 5596,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.841,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.422,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 47.39,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 35.3,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 307,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1298,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3203,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2915,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 39700,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 4.295,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 62.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 633.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6335,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 195.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1953,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 51.9,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 524.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5266,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 128.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1300,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 7.691,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 76.66,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 8627,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 986.9,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8833,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 912.2,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 49.33,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 524.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 103.9,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 1041,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 10440,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 188.9,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1329,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12740,
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
          "id": "eb6cbee24f8f0a00ca2b5e5920d4baa3559a9dc4",
          "message": "fix: 非当前项目新会话不触发项目列表自动刷新 (closes #445) (#446)\n\n* fix: 非当前项目新会话不触发项目列表自动刷新 (closes #445)\n\n三层修复：\n\n1. Sidebar handler: 把 sessionListChanged/deleted 的 loadProjects 检查\n   从 !inGroup guard 之后移到之前，确保非当前项目结构事件也能\n   触发项目列表刷新（PR #291 引入的放置错误）\n\n2. App.svelte: 注册全局 file-change handler 兜底，确保无论哪个\n   页面 mounted（DashboardView unmount 时 Sidebar 是唯一刷新源），\n   结构事件都能触发 projectDataStore 刷新\n\n3. ProjectScanCache: 新增 bump_invalidation_generation() 方法，\n   track_unknown=false 时如果 watcher 已标记 session_list_changed=true\n   仍 bump generation，让 groups_cache 正确失效而不清 entry\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix: add catch to global file-change handler (codex nit)\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-31T21:19:53+08:00",
          "tree_id": "1b0fe61d62e042c99ded192317374d3042f8f17f",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/eb6cbee24f8f0a00ca2b5e5920d4baa3559a9dc4"
        },
        "date": 1780233798645,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 113.4,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1067,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4823,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.832,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.045,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 66.64,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 30.62,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 253.5,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1196,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 2364,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2465,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 42220,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 4.962,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 53.3,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 537.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 5389,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 193.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1943,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 56.86,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 571.9,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5788,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 106.3,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1071,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.897,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 68.76,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 7793,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 769.9,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 7621,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 829.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 45.64,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 513.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 87.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 868.7,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 8702,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 187,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1231,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 11750,
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
          "id": "3e124a408b81cf3ec795ab5c38361af34ed4b90e",
          "message": "chore(release): 0.6.3 (#447)\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-31T21:40:07+08:00",
          "tree_id": "4b6d6870f7115910dc7472c4d5a84d4db1cf0dda",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/3e124a408b81cf3ec795ab5c38361af34ed4b90e"
        },
        "date": 1780235014539,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 115,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1132,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4928,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.862,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 9.857,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 47.4,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.24,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 286.7,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1202,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3042,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2806,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 37840,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 4.989,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 60.27,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 618.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6073,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 196.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1963,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 53.94,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 548.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5462,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 118.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1194,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.812,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 67.84,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 10040,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 1079,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9128,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 999.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 47.79,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 501.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 93.94,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 939,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9424,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 201.7,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1316,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12940,
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
          "id": "a4c1fe3a628e3b3d9d8517a5e1414412748ce453",
          "message": "feat(cli): flexible setup with --scope local|project|user (#448)\n\n* feat(cli): flexible setup with --scope local|project|user\n\n- `cdt setup` (no subcommand) now does both MCP + Skills in one step\n- `--scope` flag (local/project/user) controls where config lands:\n  - local: ~/.claude/settings.local.json (private, default)\n  - project: .mcp.json + .claude/skills/ (team-shared, committable)\n  - user: ~/.claude/settings.json + ~/.claude/skills/ (global)\n- `--dry-run` replaces old dry-run-by-default (setup means \"do it\")\n- `--force` for skill overwrites is now a top-level flag\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix(cli): address codex review — combined mode resilience + home_dir safety\n\n- cmd_setup_mcp now returns Result instead of process::exit, so combined\n  mode (cdt setup) continues to install skills even if MCP registration fails\n- skills_target_dir returns Result — errors on None home_dir instead of\n  silently falling back to cwd\n- MCP registration uses current_exe() absolute path so the registered\n  server command works regardless of PATH at Claude Code invocation time\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* style: cargo fmt\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-05-31T23:15:04+08:00",
          "tree_id": "193a9f89ceb6dbbf0e32dba0505f307505046b16",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/a4c1fe3a628e3b3d9d8517a5e1414412748ce453"
        },
        "date": 1780240714499,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 113.6,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1120,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 5270,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 1.547,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.252,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 47.01,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 34.12,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 290,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1245,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3246,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3072,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 39870,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 4.536,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 62.37,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 630.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6325,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 196,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1978,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 51.88,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 525.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5257,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 129.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1307,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 7.691,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 76.66,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 9250,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 951.3,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8898,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 912.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 49.75,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 522.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 99.25,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 993.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9983,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 190,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1364,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 13090,
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
          "id": "fb1a9af7ea875ff0fec2894309c394e559c374d1",
          "message": "chore(release): 0.6.4 (#449)\n\n* docs: add 0.6.4 changelog entry\n\n* chore(release): 0.6.4\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>",
          "timestamp": "2026-05-31T23:36:23+08:00",
          "tree_id": "1352446f5e6af0378760d4448731ea411e50d552",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/fb1a9af7ea875ff0fec2894309c394e559c374d1"
        },
        "date": 1780241988910,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 111.6,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1104,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4810,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.841,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.271,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 47.05,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.73,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 293.5,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1236,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3254,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3083,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 39700,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 4.851,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 62.58,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 630.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6336,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 192,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1921,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 51.71,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 527.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5270,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 127.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1278,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 7.691,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 76.66,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 8688,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 929.9,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8687,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 928.7,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 48.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 510.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 96.57,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 968.6,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9712,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 203.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1355,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12980,
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
          "id": "567f6f848aa627fa7b8abd1aa0a09bf7f04c82ce",
          "message": "feat(cli): add shell completion support (zsh/bash/fish/powershell) (#450)\n\nImplements dynamic shell completions using clap_complete's CompleteEnv\nwith custom env var CDT_COMPLETE (following `just`'s pattern to avoid\ncollision with other tools).\n\nFeatures:\n- `cdt setup completions` — auto-detect shell, install to correct path\n- `cdt completions <shell>` — output registration script to stdout\n- `cdt self-update` — auto-refresh installed completions after update\n- Dynamic `--project` completion with decoded project names\n- Dynamic session ID completion with [project] title preview (top 20 by mtime)\n\nArchitecture (following community best practices):\n- CompleteEnv at main() entry with custom var CDT_COMPLETE\n- Completions subcommand uses subprocess self-invocation (official pattern)\n- Default `cdt setup` gracefully skips on unsupported shells\n- Session title extraction skips malformed JSONL lines\n\nPerformance (release build, 293 projects / 794 sessions):\n- Static completion: ~110ms\n- Project name completion: ~110ms\n- Session ID completion: ~140ms\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-01T09:45:45+08:00",
          "tree_id": "c96b23e5a96ff2e0ab81cad4bc435552a4a7b07c",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/567f6f848aa627fa7b8abd1aa0a09bf7f04c82ce"
        },
        "date": 1780278575141,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 111.9,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1115,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 6858,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 1.542,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.522,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 46.43,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.57,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 291.7,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1235,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3372,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3551,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 40600,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 5.848,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 62.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 634.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6358,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 198.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1945,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 52.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 531.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5319,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 128.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1295,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 7.691,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 76.66,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 8747,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 893.8,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9494,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 918,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 48.78,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 514.2,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 94.91,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 952,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9561,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 192.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1364,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 13260,
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
          "id": "6d6472d73c3ac86ff8712a3a4595683ab91da588",
          "message": "chore(release): 0.6.5 (#452)\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>",
          "timestamp": "2026-06-01T10:00:13+08:00",
          "tree_id": "b4deacf71cc8e56b66077a4d467c960a58a022ce",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/6d6472d73c3ac86ff8712a3a4595683ab91da588"
        },
        "date": 1780279419590,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 118.6,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1150,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4697,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.852,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 9.898,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 40.9,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 32.54,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 287.1,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1197,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3070,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3301,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 37920,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 4.588,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 58.64,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 594.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6020,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 196.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1945,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 54.04,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 547.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5487,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 118.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1193,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.812,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 67.84,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 10300,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 1077,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 10560,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 986.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 46.32,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 498.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 95.21,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 959.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9712,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 208.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1326,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12790,
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
          "id": "68e167eb2f0cf346b29999ac6ef8f4a291dc1a8e",
          "message": "fix(cli): accept hyphen-prefixed --project values and reduce completion noise (#456)\n\n* fix(cli): accept hyphen-prefixed --project values and reduce completion noise\n\nTwo fixes:\n1. Add allow_hyphen_values to --project arg so encoded IDs like\n   -Users-zhaohejie-... are no longer rejected as unknown flags.\n2. ProjectCompleter now outputs display names (e.g. \"claude-devtools-rs\")\n   instead of full encoded paths, skips worktree directories, and dedupes\n   by name — reducing candidates from ~300 to ~50.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix(cli): handle duplicate display names and improve worktree filter\n\nCodex review findings:\n- When multiple projects share the same display name (e.g. /foo/app and\n  /bar/app both named \"app\"), fall back to encoded IDs for those entries\n  instead of silently dropping duplicates.\n- Replace substring match on encoded name with decoded path check for\n  /.claude/worktrees/ to avoid false positives on unlikely but possible\n  legitimate paths.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix(cli): simplify project completion to match UI display names\n\nAlways use the short display name (e.g. \"claude-devtools-rs\") as the\ncompletion value — same as what the project list UI shows. Help text\nuses ~/relative paths for context. Removes fallback to encoded IDs\nwhich was noisy and unfriendly.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix(cli): remove allow_hyphen_values to prevent flag swallowing\n\nWith completions now using display names (no hyphen prefix), this flag\nis no longer needed for normal usage. Encoded IDs can still be passed\nvia --project=<id> (= form). Removing avoids the footgun where a\nmissing value silently consumes the next flag.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix(cli): guard against empty home_dir in make_home_relative\n\nWhen home_dir() returns empty PathBuf, strip_prefix(\"\") succeeds on any\nstring, causing all help text to be incorrectly prefixed with ~.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix(cli): use encoded name for worktree filter, fix Windows path in help\n\nPR review found two bugs:\n1. decode_path is lossy (all `-` become `/`), so decoded paths never\n   contain `/.claude/worktrees/` — the filter was dead code. Fix: match\n   on the encoded directory name directly (`--claude-worktrees-`).\n2. On Windows, home_dir returns backslashes but decode_path uses forward\n   slashes, so strip_prefix never matched. Fix: normalize to `/`.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* refactor(discover): extract worktree path detection to path_decoder\n\nMove the worktree encoded path detection logic (split on\n`-.claude-worktrees-` / `--claude-worktrees-`) from project_scanner\ninto path_decoder as reusable functions:\n- is_worktree_encoded_path: bool check (used by CLI completer)\n- split_worktree_encoded_path: returns (repo, worktree) parts (used by\n  scanner's decode_historical_worktree_dir)\n\nThis ensures the CLI completion filter uses the exact same logic as the\nproject list panel, not a reimplemented heuristic.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-01T12:39:53+08:00",
          "tree_id": "b1eed03e205ac2cd738e17b2bdb81abf0acd777c",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/68e167eb2f0cf346b29999ac6ef8f4a291dc1a8e"
        },
        "date": 1780289026802,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 113,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1111,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 5240,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.861,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.272,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 48.06,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 34.1,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 293.1,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1212,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3254,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3253,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 40640,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 4.341,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 61.91,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 627,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6305,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 192.7,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1947,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 51.55,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 525.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5269,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 131.2,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1321,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 7.691,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 76.66,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 8913,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 939.7,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8888,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 964.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 49.72,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 523.9,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 96.11,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 973.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9776,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 203.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1348,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12830,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ae028126ff45e49beb293576cf04e11097e523f7",
          "message": "chore(deps/tauri): bump the tauri-deps group across 1 directory with 3 updates (#457)\n\nBumps the tauri-deps group with 3 updates in the /src-tauri directory: [log](https://github.com/rust-lang/log), [uuid](https://github.com/uuid-rs/uuid) and [tar](https://github.com/composefs/tar-rs).\n\n\nUpdates `log` from 0.4.29 to 0.4.30\n- [Release notes](https://github.com/rust-lang/log/releases)\n- [Changelog](https://github.com/rust-lang/log/blob/master/CHANGELOG.md)\n- [Commits](https://github.com/rust-lang/log/compare/0.4.29...0.4.30)\n\nUpdates `uuid` from 1.23.1 to 1.23.2\n- [Release notes](https://github.com/uuid-rs/uuid/releases)\n- [Commits](https://github.com/uuid-rs/uuid/compare/v1.23.1...v1.23.2)\n\nUpdates `tar` from 0.4.45 to 0.4.46\n- [Release notes](https://github.com/composefs/tar-rs/releases)\n- [Commits](https://github.com/composefs/tar-rs/compare/0.4.45...0.4.46)\n\n---\nupdated-dependencies:\n- dependency-name: log\n  dependency-version: 0.4.30\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: tauri-deps\n- dependency-name: uuid\n  dependency-version: 1.23.2\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: tauri-deps\n- dependency-name: tar\n  dependency-version: 0.4.46\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: tauri-deps\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-06-01T12:47:11+08:00",
          "tree_id": "c7b9f41755a9ccd0ca612cbdbc645679a90e6001",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/ae028126ff45e49beb293576cf04e11097e523f7"
        },
        "date": 1780289442564,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 117.4,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1158,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 5785,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.856,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 9.908,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 46.35,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 25.19,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 298.3,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1216,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 2987,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 3095,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 38520,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 4.287,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 62.04,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 624.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6233,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 200.3,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 2004,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 54.23,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 547.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5485,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 118.8,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1206,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 6.811,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 67.84,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 10220,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 953.3,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 9275,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 943.7,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 47.76,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 514.9,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 93.52,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 934.7,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9436,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 231.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1430,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 13460,
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
          "id": "3a5173ae8d05df508e5b0349172ad249e5813298",
          "message": "chore(release): 0.6.6 (#458)\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-01T13:02:51+08:00",
          "tree_id": "cbfe550f294365b9e0fb85a6f950fa7399dc1f72",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/3a5173ae8d05df508e5b0349172ad249e5813298"
        },
        "date": 1780290383221,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 114.8,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1109,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 5555,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.831,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.291,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 47.55,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.53,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 296.4,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1245,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3124,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2850,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 41130,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 5.227,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 62.62,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 632.3,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6344,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 203.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 2040,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 52.52,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 530.5,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5324,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 127,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1284,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 7.691,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 76.66,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 8662,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 957.2,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8609,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 952.9,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 49.38,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 525.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 97.45,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 981,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9850,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 191.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1339,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 13140,
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
          "id": "c6b4ab560cdad0f2de7ff504427aa8c7a3b8eab9",
          "message": "fix(cli): resolve correct project name from JSONL cwd for completions (#459)\n\n* fix(cli): resolve correct project name from JSONL cwd for completions\n\ndecode_path replaces ALL hyphens with slashes, causing projects with\nhyphens in their name (e.g. claude-devtools-rs) to produce wrong\ncompletion candidates (e.g. \"rs\" instead of \"claude-devtools-rs\").\n\nFix by reading the real cwd from the most recent session JSONL file\n(first 20 lines, ~1-3ms per project) which is the authoritative source.\nAlso fix dedup logic: when two projects share the same display name,\nuse the encoded directory name as completion value (resolve_project\nalready supports exact id match) instead of silently dropping one.\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n* fix(cli): address review findings - multi-file fallback, case-insensitive dedup\n\n- Try up to 3 JSONL files (mtime desc) before giving up, so a half-written\n  latest file doesn't prevent correct name resolution\n- Use case-insensitive name collision detection (matches resolve_project's\n  eq_ignore_ascii_case behavior)\n- Remove redundant decode_path call\n- Add tests for malformed JSON resilience and multi-file mtime selection\n\nCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>\nCo-authored-by: Claude Opus 4.6 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-01T14:53:41+08:00",
          "tree_id": "4357db30274a140a150ff976fcdab172d744e45c",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/c6b4ab560cdad0f2de7ff504427aa8c7a3b8eab9"
        },
        "date": 1780297042958,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 114.6,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1128,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 9098,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 0.851,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.096,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 45.43,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 33.7,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 289.5,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1303,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3123,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2946,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 40050,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 4.971,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 69.67,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 699.6,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6427,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 201.1,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 2035,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 51.64,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 525,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5262,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 128.3,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1290,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/1000",
            "value": 7.69,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/validate_encoded_path/10000",
            "value": 76.66,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_large",
            "value": 8678,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 901.1,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8782,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 924.6,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 49.38,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 516.8,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 96.36,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 969.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9733,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 193.4,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1351,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12850,
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
          "id": "0d104ab5e05c1c0ac4bd3a31deff27513deba2d7",
          "message": "chore(release): 0.6.7 (#460)\n\nCo-authored-by: 赵和杰 <zhaohejie.zhj@taobao.com>",
          "timestamp": "2026-06-01T15:50:34+08:00",
          "tree_id": "022af47ce367af292ef3bc581b47ca164f79beb5",
          "url": "https://github.com/snowzhaozhj/claude-devtools-rs/commit/0d104ab5e05c1c0ac4bd3a31deff27513deba2d7"
        },
        "date": 1780300438058,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "cdt-analyze/build_chunks/50",
            "value": 113.2,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/500",
            "value": 1120,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/build_chunks/2000",
            "value": 4861,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/50",
            "value": 1.542,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/500",
            "value": 8.432,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/check_messages_ongoing/2000",
            "value": 47.5,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/50",
            "value": 34.07,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/500",
            "value": 288.1,
            "unit": "µs"
          },
          {
            "name": "cdt-analyze/pair_tool_executions/2000",
            "value": 1235,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_project_scan",
            "value": 3042,
            "unit": "µs"
          },
          {
            "name": "cdt-api/cold_scan_and_group",
            "value": 2928,
            "unit": "µs"
          },
          {
            "name": "cdt-api/get_session_detail",
            "value": 39320,
            "unit": "µs"
          },
          {
            "name": "cdt-api/list_repository_groups",
            "value": 4.736,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/100",
            "value": 62.85,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/1000",
            "value": 637.9,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/decode_path_throughput/10000",
            "value": 6434,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/100",
            "value": 191.3,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_decode_roundtrip/1000",
            "value": 1930,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/100",
            "value": 52.45,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/1000",
            "value": 571.4,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/encode_path_throughput/10000",
            "value": 5269,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/1000",
            "value": 136.3,
            "unit": "µs"
          },
          {
            "name": "cdt-discover/extract_project_name_throughput/10000",
            "value": 1324,
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
            "value": 8712,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/direct_read_small",
            "value": 869.9,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_large",
            "value": 8741,
            "unit": "µs"
          },
          {
            "name": "cdt-fs/dyn_read_small",
            "value": 899.3,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/500",
            "value": 52.81,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/dedupe_by_request_id/5000",
            "value": 556.5,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/50",
            "value": 97.67,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/500",
            "value": 984.1,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_entry_lines/5000",
            "value": 9858,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/50",
            "value": 184.6,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/500",
            "value": 1343,
            "unit": "µs"
          },
          {
            "name": "cdt-parse/parse_file_async/5000",
            "value": 12910,
            "unit": "µs"
          }
        ]
      }
    ]
  }
}