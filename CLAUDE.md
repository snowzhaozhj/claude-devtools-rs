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
| project-discovery              | `cdt-discover`  | not started |
| context-tracking               | `cdt-analyze`   | not started |
| team-coordination-metadata     | `cdt-analyze`   | not started |
| session-search                 | `cdt-discover`  | not started |
| file-watching                  | `cdt-watch`     | not started |
| configuration-management       | `cdt-config`    | not started |
| notification-triggers          | `cdt-config`    | not started |
| ssh-remote-context             | `cdt-ssh`       | not started |
| ipc-data-api                   | `cdt-api`       | not started |
| http-data-api                  | `cdt-api`       | not started |

† tool-execution-linking 的 pair / resolver / filter 都是纯函数，已完整实现且有单测覆盖；但默认 `build_chunks` 路径只接入了 pair。`resolve_subagents` 的 candidate 装载与 `filter_resolved_tasks` 的端到端接入，以及 `ChunkMetrics::tool_count` 的过渡语义修正，留给 `port-team-coordination-metadata`（对应 change archive 里 tasks.md section 11）。

## Remaining port order

剩余 10 个 capability 按依赖链推进（已完成 3 项见上表）。每步 ship 成一个 `port-<capability>` opsx change，spec 行为与 TS 不一致时写 MODIFIED delta。

1. **project-discovery** — 引入 `FileSystemProvider` trait，解锁 session-search / ssh-remote-context
2. **context-tracking** — 6-category injection classifier + phase resets
3. **file-watching** — 100ms debounce + event broadcast
4. **session-search** — scope 化搜索 + mtime cache
5. **configuration-management** — config persist + CLAUDE.md reader + `@mention` sandbox
6. **notification-triggers** — error detector + trigger evaluator
7. **team-coordination-metadata** — teammate 检测 + `Process.team` 富化 + team 工具摘要；同时接尾 port 3 的 Task filter / `tool_count` 语义
8. **ssh-remote-context** — 实现 `FileSystemProvider` over SSH
9. **ipc-data-api** — trait surface
10. **http-data-api** — axum server mirroring IPC

## Known TS impl-bugs — FIX, do not replicate

From `openspec/followups.md`。已修项带 ✓，剩余是后续 port 的 MUST 项：

- ✓ **session-parsing**：`deduplicateByRequestId` 已在 `crates/cdt-parse/src/dedupe.rs` 接入 `parse_file` 主路径。
- ✓ **tool-execution-linking**：duplicate `tool_use_id` 由 `pair_tool_executions` `tracing::warn!` + `duplicates_dropped` 计数。
- ◐ **chunk-building**：Task 过滤纯函数 `filter_resolved_tasks` 已实现，但默认 `build_chunks` 路径未接入；端到端接入留给 `port-team-coordination-metadata`。
- **configuration-management**: `ConfigManager.loadConfig()` on corrupted
  file should back up the bad file before loading defaults. TS only logs.
  Rust port MUST back up.
- **notification-triggers**: `is_error=true` on tool_result should trigger
  error detection; TS relies on content-pattern matching instead. Rust port
  MUST check the flag.

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

## Conventions

- **Error types**: library crates use `thiserror` enums; the `cdt-cli` binary uses `anyhow::Result`.
- **Async runtime**: `tokio` is added only to leaf crates that need I/O; `cdt-core` stays sync.
- **Logging**: `tracing`; subscriber initialized once in `cdt-cli`.
- **No `unwrap()` in library code** — use `?` or typed errors.
- **No cross-crate imports of internal modules** — go through each crate's public API.
- **clippy pedantic 陷阱**：`doc_markdown` 要求 doc/module 注释里出现的 `CamelCase` 或 `snake_case` 标识符都用反引号包裹，中文注释也不例外（`AIChunk` / `tool_count`）。
- **insta 快照接受**：没装 `cargo-insta` 就用 `INSTA_UPDATE=always cargo test -p <crate>`；提交生成的 `tests/snapshots/*.snap`。
- **同步解析入口**：`cdt-analyze` 的集成测试不引入 tokio——用 `cdt_parse::parse_entry_at(line, n)` 逐行解析 fixture，再跑 `dedupe_by_request_id`。
- **自动化**：
  - Hooks（`.claude/hooks/`）：`.rs` 编辑后自动跑所属 crate 的 `cargo clippy -- -D warnings`；直接编辑 `openspec/specs/**` 会被 PreToolUse 拒绝（走 delta）。
  - Subagent：`spec-fidelity-reviewer` 按 capability 审计 scenario→test 覆盖。
  - Skill：`/ts-parity-check <capability>` 对比 TS 源与 Rust 端口 + followups。
  - MCP：`.mcp.json` 注册 GitHub MCP，需要 `GITHUB_PERSONAL_ACCESS_TOKEN` 环境变量。
- Detailed rules: `.claude/rules/rust.md`.

## What to do first in a fresh session

1. Run `cargo build --workspace` 确认 bootstrap 仍可编译；`cargo test -p cdt-core -p cdt-analyze` 跑一遍既有回归。
2. 看顶部 Capability → crate map 的进度栏，决定下一个 port（当前 3/13 done）。
3. 对目标 capability 跑 `/ts-parity-check <cap>` 查 TS 源对照与 followups。
4. `/opsx:propose port-<cap>` → `/opsx:apply` → `/opsx:archive`。跨 port 之间 `/clear`，port 内保持同会话。
