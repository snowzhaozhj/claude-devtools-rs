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
| project-discovery              | `cdt-discover`  | not started |
| session-parsing                | `cdt-parse`     | not started |
| chunk-building                 | `cdt-analyze`   | not started |
| tool-execution-linking         | `cdt-analyze`   | not started |
| context-tracking               | `cdt-analyze`   | not started |
| team-coordination-metadata     | `cdt-analyze`   | not started |
| session-search                 | `cdt-discover`  | not started |
| file-watching                  | `cdt-watch`     | not started |
| configuration-management       | `cdt-config`    | not started |
| notification-triggers          | `cdt-config`    | not started |
| ssh-remote-context             | `cdt-ssh`       | not started |
| ipc-data-api                   | `cdt-api`       | not started |
| http-data-api                  | `cdt-api`       | not started |

## Recommended port order

Port in dependency order (each step unblocks the next):

1. **session-parsing** — JSONL → `ParsedMessage`. This unblocks everything downstream. While porting, introduce the core types in `cdt-core` (`ParsedMessage`, `ContentBlock`, `ToolCall`, `ToolResult`, `TokenUsage`, `MessageCategory`).
2. **chunk-building** — `ParsedMessage` stream → `Chunk` enum (User/AI/System/Compact). Introduces `Chunk`, `EnhancedChunk`, metrics.
3. **tool-execution-linking** — pair `tool_use`/`tool_result`; three-phase Task→subagent matcher. Introduces `ToolExecution`, `Process`.
4. **context-tracking** — 6-category injection classifier + phase resets.
5. **project-discovery** — scan, decode paths, worktree group. Introduces `FileSystemProvider` trait in `cdt-core`.
6. **file-watching** — 100ms debounce, event broadcast.
7. **session-search** — in-session / per-project / global search with mtime cache.
8. **configuration-management** — config persist, CLAUDE.md reader, @mention resolver (with sandboxing).
9. **notification-triggers** — error detector + trigger evaluator + regex safety validation.
10. **team-coordination-metadata** — teammate message detection, `Process.team` enrichment, team tool summaries.
11. **ssh-remote-context** — implement `FileSystemProvider` over SSH.
12. **ipc-data-api** — trait surface covering the full operation set.
13. **http-data-api** — axum server mirroring IPC operations under `/api`.

Each step should ship as one opsx change named `port-<capability>` with a
MODIFIED Requirements delta whenever Rust semantics force a clarification or
the port intentionally diverges from the TS baseline.

## Known TS impl-bugs — FIX, do not replicate

From `openspec/followups.md`:

- **session-parsing**: `deduplicateByRequestId` exists in TS but is never
  called from `SessionParser.processMessages`. Rust port MUST wire dedup in.
- **chunk-building**: Task tool filtering (hide resolved Task calls from the
  AIChunk tool list) is spec'd but not implemented in TS. Rust port MUST
  filter.
- **tool-execution-linking**: duplicate `tool_use_id` must log a warning —
  TS silently takes the first. Rust port MUST log.
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

1. Run `cargo build --workspace` to confirm the bootstrap still compiles.
2. Read `openspec/specs/session-parsing/spec.md` and `openspec/followups.md`.
3. Propose the first port: `/opsx:propose port-session-parsing`.
4. Work through it with `/opsx:apply`, writing code in `crates/cdt-parse/` and core types in `crates/cdt-core/`.
