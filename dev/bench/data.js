window.BENCHMARK_DATA = {
  "lastUpdate": 1780129212474,
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
      }
    ]
  }
}