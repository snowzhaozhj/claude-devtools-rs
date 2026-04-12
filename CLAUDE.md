# claude-devtools-rs

Rust port of [claude-devtools](../claude-devtools) — the Electron app that
visualizes Claude Code session execution. This repo ports the **data and
logic layer** (13 capabilities); UI technology is undecided and deferred.

## Goal

Reproduce the behavior frozen in `openspec/specs/` — one capability at a
time — in idiomatic Rust, while **fixing** the known TS implementation bugs
listed in `openspec/followups.md` rather than replicating them.

## Parent repo

The TypeScript source is at `/Users/zhaohejie/RustroverProjects/claude-devtools`.
It is the historical reference only; all behavioral contracts live here in
`openspec/specs/` now. When in doubt, read the spec — not the TS source.

## Workspace layout

```
claude-devtools-rs/
├── Cargo.toml                # workspace root
├── rust-toolchain.toml       # stable channel
├── crates/
│   ├── cdt-core/             # shared types + traits (no runtime deps)
│   ├── cdt-parse/            # session-parsing
│   ├── cdt-analyze/          # chunk-building + tool-linking + context-tracking + team-metadata
│   ├── cdt-discover/         # project-discovery + session-search
│   ├── cdt-watch/            # file-watching
│   ├── cdt-config/           # configuration-management + notification-triggers
│   ├── cdt-ssh/              # ssh-remote-context
│   ├── cdt-api/              # ipc-data-api + http-data-api (facade + HTTP server)
│   └── cdt-cli/              # binary entrypoint (bin = cdt)
├── openspec/
│   ├── specs/                # 13 capability specs (authoritative)
│   ├── followups.md          # TS impl-bugs to fix, not replicate
│   └── README.md             # workflow + capability map
└── .claude/rules/rust.md     # Rust coding conventions
```

## Capability → crate map

| Capability                     | Owning crate    | Port status |
|--------------------------------|-----------------|-------------|
| session-parsing                | `cdt-parse`     | done ✓      |
| chunk-building                 | `cdt-analyze`   | done ✓      |
| tool-execution-linking         | `cdt-analyze`   | done ✓ †    |
| project-discovery              | `cdt-discover`  | done ✓      |
| context-tracking               | `cdt-analyze`   | done ✓ ‡    |
| team-coordination-metadata     | `cdt-analyze`   | done ✓      |
| session-search                 | `cdt-discover`  | done ✓      |
| file-watching                  | `cdt-watch`     | done ✓      |
| configuration-management       | `cdt-config`    | done ✓      |
| notification-triggers          | `cdt-config`    | done ✓      |
| ssh-remote-context             | `cdt-ssh`       | not started |
| ipc-data-api                   | `cdt-api`       | not started |
| http-data-api                  | `cdt-api`       | not started |

† tool-execution-linking 的 pair / resolver / filter 都是纯函数，已完整实现且有单测覆盖。`filter_resolved_tasks` 已在 `port-team-coordination-metadata` 接入 `build_chunks_with_subagents`；`resolve_subagents` 的 candidate 装载由调用方（`cdt-api` 层）负责。

‡ context-tracking 的 6 类 injection、phase 管理、compaction token delta 已在 `cdt-analyze::context` 完整实现且有单测覆盖；CLAUDE.md 文件 / `@mention` 的真实磁盘扫描依赖 `initial_claude_md_injections` 外部注入，已由 `port-configuration-management` 的 `ClaudeMdReader` 提供数据源。

## Remaining port order

剩余 8 个 capability 按依赖链推进（已完成 5 项见上表）。每步 ship 成一个 `port-<capability>` opsx change，spec 行为与 TS 不一致时写 MODIFIED delta。

1. **file-watching** — 100ms debounce + event broadcast
2. **session-search** — scope 化搜索 + mtime cache
3. **configuration-management** — config persist + CLAUDE.md reader + `@mention` sandbox（会接入 `port-context-tracking` 的 `initial_claude_md_injections`）
4. **notification-triggers** — error detector + trigger evaluator
5. **team-coordination-metadata** — teammate 检测 + `Process.team` 富化 + team 工具摘要；同时接尾 port 3 的 Task filter / `tool_count` 语义、以及 context-tracking 的 `teammate_message` display item
6. **ssh-remote-context** — 为 `FileSystemProvider` 实现 SSH 后端（seam 已在 `port-project-discovery` 落地）
7. **ipc-data-api** — trait surface
8. **http-data-api** — axum server mirroring IPC

## Known TS impl-bugs — FIX, do not replicate

From `openspec/followups.md`。已修项带 ✓，剩余是后续 port 的 MUST 项：

- ✓ **session-parsing**：`deduplicateByRequestId` 已在 `crates/cdt-parse/src/dedupe.rs` 接入 `parse_file` 主路径。
- ✓ **tool-execution-linking**：duplicate `tool_use_id` 由 `pair_tool_executions` `tracing::warn!` + `duplicates_dropped` 计数。
- ✓ **project-discovery**：路径解码 spec-gap 已落地 —— `path_decoder::decode_path` best-effort + `ProjectPathResolver::resolve` 通过 session `cwd` 字段消歧；新增 `FileSystemProvider::read_lines_head` 修正 SSH 模式全文件读取的隐性性能 bug。
- ✓ **context-tracking**：TS 侧完全无测试的 coverage-gap 已补齐 —— `cdt-analyze::context` 的 aggregator / stats / session 三层各自单测（12 单测 + 7 集成测试），compaction delta、路径去重、camelCase JSON shape 都有硬断言。
- ◐ **chunk-building**：Task 过滤纯函数 `filter_resolved_tasks` 已实现，但默认 `build_chunks` 路径未接入；端到端接入留给 `port-team-coordination-metadata`。
- ✓ **configuration-management**: `ConfigManager.loadConfig()` on corrupted
  file should back up the bad file before loading defaults. TS only logs.
  Rust port 已实现备份（`manager.rs` `backup_corrupted_file`）。
- ✓ **notification-triggers**: `is_error=true` on tool_result should trigger
  error detection。Rust port 已实现 `is_error` flag 检查
  （`error_trigger_checker.rs` `check_tool_result_trigger`）。

## Common commands

```bash
cargo build --workspace              # build all crates
cargo test --workspace               # run tests
cargo clippy --workspace --all-targets  # lint (workspace-level lints in Cargo.toml)
cargo fmt --all                      # format
cargo run -p cdt-cli                 # run the CLI binary
cargo build -p cdt-parse             # build one crate in isolation
cargo test -p cdt-analyze            # test one crate
```

## macOS 开发陷阱

- `TempDir` 返回 `/var/...` 但 `notify`/FSEvents 返回 `/private/var/...`（symlink canonicalization）。涉及路径比较时必须 `canonicalize()`。
- `notify-debouncer-mini` 的 timer 不受 `tokio::time::pause()` 控制，测试不确定。优先用 `notify` 裸接 + 自实现 tokio debounce。

## Conventions

- **Error types**: library crates use `thiserror` enums; the `cdt-cli` binary uses `anyhow::Result`.
- **Async runtime**: `tokio` is added only to leaf crates that need I/O; `cdt-core` stays sync.
- **Logging**: `tracing`; subscriber initialized once in `cdt-cli`.
- **No `unwrap()` in library code** — use `?` or typed errors.
- **No cross-crate imports of internal modules** — go through each crate's public API.
- **clippy pedantic 陷阱（本 workspace 反复触发，写的时候就避开）**：
  - `doc_markdown`：doc/module 注释里出现的 `CamelCase` / `snake_case` 标识符都要反引号包裹，中文注释也不例外（`AIChunk` / `tool_count`）。
  - `map_unwrap_or`：`opt.map(f).unwrap_or_else(g)` → `opt.map_or_else(g, f)`。
  - `single_match_else`：`match x { Some(v) => v, None => { ... } }` → `if let Some(v) = x { v } else { ... }`。
  - `needless_continue`：match arm / loop 末尾的 `continue` / `=> continue,` 写成 `{}` / `=> {}`。
  - `assigning_clones`：`a = b.clone()` → `a.clone_from(&b)`（对 `Vec` / `String` 字段尤其敏感）。
  - `cloned_ref_to_slice_refs`：测试里 `&[item.clone()]` → `std::slice::from_ref(&item)`。
  - `cast_possible_wrap`：`u64 as i64` 禁用；用 `i64::try_from(x).unwrap_or(i64::MAX)`。
  - `case_sensitive_file_extension_comparisons`：`name.ends_with(".jsonl")` → `Path::new(&name).extension().is_some_and(|e| e.eq_ignore_ascii_case("jsonl"))`。
  - `uninlined_format_args`：`format!("{}", x)` → `format!("{x}")`；命名参数 `format!("{foo}", foo = bar)` 也要内联。
  - `manual_string_new`：测试中 `"".into()` / `"".to_owned()` → `String::new()`。
  - `manual_pattern_char_comparison`：`trim_end_matches(|c: char| c == '/' || c == '\\')` → `trim_end_matches(['/', '\\'])`。
  - `while_let_loop`：`loop { match x.next().await { Ok(Some(v)) => ..., _ => break } }` → `while let Ok(Some(v)) = x.next().await { ... }`。
- **insta 快照接受**：没装 `cargo-insta` 就用 `INSTA_UPDATE=always cargo test -p <crate>`；提交生成的 `tests/snapshots/*.snap`。
- **同步解析入口**：`cdt-analyze` 的集成测试不引入 tokio——用 `cdt_parse::parse_entry_at(line, n)` 逐行解析 fixture，再跑 `dedupe_by_request_id`。
- **自动化**：
  - Hooks（`.claude/hooks/`）：`.rs` 编辑后自动跑所属 crate 的 `cargo clippy -- -D warnings`；`git commit` 前自动跑 `openspec validate --strict`。**`openspec/specs/**` 的直接编辑由约定（不是 hook）约束** —— spec 变更必须走 `openspec/changes/<name>/specs/` 的 delta，由 `/opsx:archive` 时 sync 回主 spec。
  - Subagent：`spec-fidelity-reviewer` 按 capability 审计 scenario→test 覆盖。
  - Skill：`/ts-parity-check <capability>` 对比 TS 源与 Rust 端口 + followups。
  - MCP：`.mcp.json` 注册 GitHub MCP，需要 `GITHUB_PERSONAL_ACCESS_TOKEN` 环境变量。
- **opsx:apply 推进节拍（硬约束）**：port 内任何多步改动必须按固定流水线推进，**不得**把 PostToolUse clippy hook 的沉默当作"可以停手"的信号。节拍：
  1. `Edit` 源文件（可并行）
  2. `cargo clippy --workspace --all-targets -- -D warnings` 汇总校验（**不是**靠 hook 单文件回显）
  3. `cargo fmt --all`
  4. `cargo test -p <crate>`（或 `--workspace`）
  5. 联动 `openspec/followups.md` + 根 `CLAUDE.md` 的 Capability→crate map 与 "Known TS impl-bugs" 段
  6. `openspec validate <change> --strict`
  7. 勾 `openspec/changes/<change>/tasks.md` 的 checkbox
  8. 发最终文本总结
  每轮 tool call 结束前自检一句"这批之后要么发下批工具、要么发最终文本，二者必居其一"；只发 Edit 没有后续计划 = 禁止。开工时把 tasks.md 的每个 `##` section 作为 `TaskCreate` 入队，完成一个 `TaskUpdate completed` 一个，给自己留显式的"下一步指针"。
- Detailed rules: `.claude/rules/rust.md`.

## What to do first in a fresh session

1. Run `cargo build --workspace` 确认 bootstrap 仍可编译；`cargo test -p cdt-core -p cdt-analyze` 跑一遍既有回归。
2. 看顶部 Capability → crate map 的进度栏，决定下一个 port。
3. 对目标 capability 跑 `/ts-parity-check <cap>` 查 TS 源对照与 followups。
4. `/opsx:propose port-<cap>` → `/opsx:apply` → `/opsx:archive`。跨 port 之间 `/clear`，port 内保持同会话。
