# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Each release ships prebuilt binaries (macOS / Linux / Windows) on the
[Releases](https://github.com/snowzhaozhj/claude-devtools-rs/releases) page.

## [Unreleased]

### Fixed
- **UI**: polished SessionMetaMenu styling — consistent padding, hover states, and keyboard interaction.
- **UI**: improved markdown prose styling and added code block copy button for assistant messages.
- **UI**: prevented WKWebView focus-scroll jump when clicking copy buttons.

## [0.6.16] — 2026-06-10

### Fixed
- **CLI**: `--group-by project --json=field1,field2` now correctly projects fields into nested session objects instead of returning empty objects.
- **Session activity**: `gitSummary` no longer captures `$(cat <<` from heredoc-style commit commands as false commit messages.
- **Session activity**: `userIntents` no longer includes system noise (`<task-notification>`, slash commands); skill invocations are rendered as `/skill-name`.

## [0.6.15] — 2026-06-10

### Added
- **Stats**: new derived metrics — cache hit rate, average cost/messages per session, and programming language frequency in `cdt stats` and MCP `get_stats`.
- **Session activity**: session list now includes activity summary fields (duration, message/turn counts, tool usage, cost, primary language) exposed via IPC, CLI, and MCP.

### Fixed
- **Parse**: shallow session parser now correctly handles real JSONL entry types (`assistant`/`user`) instead of only matching the legacy `conversation` type.

## [0.6.14] — 2026-06-08

### Added
- **Export**: session export to Markdown, JSON, and HTML formats via context menu or keyboard shortcut.
- **Query**: session date filter now uses interval intersection for more accurate date range matching.

### Removed
- **UI**: removed unusable "Copy Deeplink" option from message context menu (generated `tauri://` URLs have no practical use in desktop app).

## [0.6.13] — 2026-06-08

### Added
- **MCP/CLI**: redesigned tools to intent-oriented surface — tools now map to user goals (analyze, search, compare, extract) instead of raw data endpoints, with richer built-in prompts.

### Fixed
- **UI**: removed broken scroll compensation on expand/collapse toggle that caused content jumping in opposite direction.

## [0.6.12] — 2026-06-07

### Fixed
- **UI**: compensate scroll position on expand/collapse toggle to prevent content jumping.
- **HTTP mode**: release desktop app now serves frontend at `http://localhost:3456/` by bundling `ui/dist` into app resources (was 404 since server-mode was first introduced).

## [0.6.11] — 2026-06-06

### Added
- **CLI**: `--extract` mode for item-level flat output, enabling granular data extraction from sessions.
- **CLI**: improved `session-insights` skill and MCP tool instructions for better AI-assisted analysis.

### Fixed
- **CLI**: `summary` Top Files paths no longer hard-code truncation, respecting actual path lengths.
- **UI**: eliminated scroll jumping during lazy markdown hydration in session detail view.

## [0.6.10] — 2026-06-06

### Fixed
- **CLI**: `self-update` and desktop CLI install now show friendly error messages instead of raw URLs and error chains; connection timeout 10s for fast failure, download timeout 90s for slow networks.
- Dock and tray icons now use transparent dark variant for better visibility.

## [0.6.9] — 2026-06-06

### Added
- **CLI**: shared view layer with field selection and unified output formatting.

### Fixed
- App icons now use transparent background for cleaner appearance on all platforms.

### Changed
- Redesigned app icon and tray icon to Clawd robot design.

## [0.6.8] — 2026-06-06

### Added
- **MCP**: session recall grep and search tool content indexing for richer session queries.
- **CLI**: download and install the CLI directly from the desktop Settings page.
- Redesigned app icon and tray icon.

### Fixed
- Context window now displays correct token counts (missing cache fields caused near-zero display).

### Performance
- CWD cache and sidebar debounce throttle reduce workflow-triggered CPU usage.

## [0.6.7] — 2026-06-01

### Fixed
- CLI completions now resolve the correct project name from JSONL session cwd.

## [0.6.5] — 2026-06-01

### Added
- CLI shell completion support for zsh, bash, fish, and powershell.
- MCP output optimization: pagination, field omission, and compact JSON format.

## [0.6.4] — 2026-05-31

### Added
- CLI setup command now supports `--scope local|project|user` for flexible configuration placement.

## [0.6.3] — 2026-05-31

### Added
- Context window usage progress bar in session detail panel.

### Fixed
- New sessions from other projects no longer trigger unnecessary project list refresh.

### Performance
- Workflow lazy-loading: skeleton placeholder + on-demand detail fetch.
- Cap grouper concurrency + add groups cache to reduce cold-start CPU.
- Replace hand-rolled LRU with `lru` crate in cdt-api (simpler, faster eviction).

## [0.6.2] — 2026-05-31

### Fixed
- Jobs panel: stop button, stale status display, and empty name fallback.

## [0.6.1] — 2026-05-31

### Added
- **Background Jobs panel**: monitor `claude --bg` sessions with live status, logs, and stop/clean actions.
- Queued user messages now render inline within the AI turn (no separate bubble).

### Fixed
- Restored `Unchanged` short-circuit for ongoing sessions (perf regression from 0.6.0).
- CJK text in config values no longer panics on char-boundary truncation.
- Fallback group matching when git status changes mid-scan.
- Workflow `failed_by_heuristic` restricted to completed agents only.
- Scrollbar-gutter jump eliminated across all vertical scroll containers.
- CLI `self-update` without explicit version now follows `releases/latest` redirect (bypasses API rate limit).

### Changed
- Project is now MIT-licensed.
- Release flow: CHANGELOG entries written per-PR; `release-bump.sh` auto-converts `[Unreleased]` to versioned heading.

## [0.6.0] — 2026-05-30

### Added
- **Workflow drilldown**: click an agent chip to view its full conversation trace.
- **Workflow running state**: degraded rendering when manifest is missing (synthesizes Running state from journal + scriptPath).
- **Workflow backend**: `WorkflowAgent` session_id + `get_workflow_agent_trace` IPC command.
- **WorkflowCard**: 6-state rendering with backend manifest parsing.
- **Tool linking**: extract Workflow `runId` to `ToolExecution`.
- **Context tracking**: per-turn context badge with visible context indicator.
- **UI**: scroll arrows on worktree chip cluster.
- **Perf**: parallelize subagent scan + merge double file reads.

### Fixed
- **Perf**: coalesce file events + cache subagent scan (idle CPU 32% → <3%).
- Command palette search and project list are now group-aware.
- File-watcher correctly identifies workflow paths and hides 0ms running state.
- WorkflowCard no longer renders blank on expand or crashes on expand.
- Workflow summary correctly counts workflows.
- Removed empty whitespace row above tool input/output blocks.
- Resolved 3 bugs found by bug-hunt audit.
- CLI displays full session ID in table output.

## [0.5.14] — 2026-05-28

### Added
- `cdt self-update` command to upgrade the CLI in place.

### Fixed
- Copy button now pins to the right side of Bash command blocks.
- Suppressed perf-tracing noise in release builds of the CLI.

## [0.5.13] — 2026-05-28

### Added
- **CLI (`cdt`)**: standalone binary distribution with a one-line install script.
- **CLI**: `cdt setup skills` installs session-aware Claude Code skill templates.
- **MCP**: `cdt mcp serve` exposes a stdio MCP server (built on the `rmcp` SDK)
  so Claude Code can query its own past sessions.
- **CLI**: `session summary`, cost estimation, and `stats` commands.
- **CLI**: `session detail` / `errors` / `search` subcommands.
- **CLI**: `projects` / `sessions` listing via a clap subcommand structure.
- CI benchmark trend tracking and divan microbenchmarks for the parse/analyze/discover crates.

## [0.5.12] — 2026-05-27

### Added
- Copy-to-clipboard buttons on tool viewers and code blocks.
- Optimistic-concurrency `_version` field exposed to the config frontend loop.

### Fixed
- Selection highlight no longer bleeds into the `OutputBlock` border.
- SSH failure paths surface a proper SSH error instead of a generic internal error.
- `agent_configs` file-extension matching is now case-insensitive.

## [0.5.11] — 2026-05-27

### Added
- Dashboard activity time stays fresh on cache-hit paths via an mtime overlay.

### Performance
- Second-level fingerprint short-circuit for chunk building plus adaptive
  file-change debounce — large, actively-written sessions re-render far less.
- `get_session_detail` fingerprint short-circuit skips redundant recomputation.
- `worktree_grouper` runs canonicalization off the tokio worker threads.
- `tabSessionCache` gained LRU eviction to lower WebView memory.

### Fixed
- Strip ANSI escapes before rendering tool output (no more raw colour bytes
  leaking from `nextest` / `cargo` into the desktop app).
- System messages align to the AI thread rail at a 27px baseline.

## [0.5.10] — 2026-05-25

### Added
- File-change events carry `session_list_changed` plus an SSE-lagged fallback.

### Performance
- Typed `SessionDetail` IPC payload and five high-frequency data-API methods.

### Fixed
- Removed the `content-visibility` size estimate that caused Session Detail
  scroll jitter.
- `agent_configs` discovery goes through the shared home-dir decoder and async fs.

## [0.5.9] — 2026-05-24

### Added
- **Right-click context menu (Phase 2)**: wired into five surfaces with shared
  infrastructure and an overall UI polish pass.
- SSH transport keepalive prevents idle channels from being closed.

### Performance
- Unified the tokio runtime and tuned the blocking-pool budget to cut idle CPU.
- Merged cache invalidators to reduce the number of broadcast subscribers.
- Notification unread count moved to a push event; the 30s poll became a 5min
  safety-net (stops the WebView from being kept awake).
- `TelemetryLayer` gained a `WARN` level filter and call-site counter caching.

### Fixed
- Removed the sidebar metadata-pending shimmer.
- Cross-platform keyboard bindings normalised to literal `mod` with a Windows-key guard.
- "Jump to latest" smooth-scrolls then re-arms the bottom-pin fallback.
- SFTP failure detection split into three states; the scanner fails fast on a dead channel.

## [0.5.8] — 2026-05-24

### Added
- **Centralised keyboard-shortcut registry** with a Settings key-recording
  widget, persisted through `cdt-config`.
- **Right-click context menu (Phase 1)**.
- Session Detail top-bar `[⋯]` meta-action menu replaces the long CWD string.

### Performance
- `ProjectScanCache` invalidates by event semantics rather than wholesale.
- CI test runner switched to `cargo-nextest` (~6× wall-time speedup).

### Fixed
- Session title now derives from a single backend source shared with the sidebar.
- Preserve scroll position when switching tabs in Session Detail.
- AI message tool summaries no longer truncate when many tool types are present.

## [0.5.7] — 2026-05-23

### Added
- **Browser Access / server mode**: opt-in local HTTP server (`127.0.0.1`) that
  serves the same UI in a browser over HTTP/SSE, with a CORS-restricted
  contract and static asset serving.
- **SSH remote browsing**: inspect sessions on a remote machine, including
  remote project-memory CRUD.
- **Telemetry Signal Bus (Phase 1)**: counters / histograms / events with an
  IPC snapshot and a Diagnostics tab.
- Windows WSL distro one-click scan with a dedicated modal.
- Sidebar worktree filter chip cluster; "jump to latest message" floating button.

### Performance
- SFTP message-id pipeline cuts single-file remote reads ~14× (8s → 600ms).
- SSH `open_read` K-worker prefetch streaming drops peak RSS from ~5MB to ~1MB.
- `ProjectScanner` memoizes scan results across IPC calls.
- Unified `FileSystemProvider` abstraction; signature-keyed metadata / parse caches.

### Fixed
- HTTP server mode no longer reserves space for macOS traffic-light buttons.
- Startup panic from double logger initialisation removed.
- Numerous SSH reconnect / deadlock / SSE-emit fixes.

## [0.5.6] — 2026-05-18

### Added
- Release automation: `just release-bump` plus a workflow that auto-verifies and publishes.

### Fixed
- Edit tool now shows the error message when an edit fails.

## [0.5.5] — 2026-05-18

### Fixed
- Removed the blank band at the top of the project workspace.

## [0.5.4] — 2026-05-18

### Added
- Dashboard reworked into a project workspace (list / grid + sorting + inline metadata).

### Fixed
- Dashboard sort control no longer overlaps; persistent sidebar selection
  styling de-blued; search box auto-capitalisation disabled.

## [0.5.3] — 2026-05-17

### Fixed
- Prevent long markdown from overflowing.
- Avoid blank expanded tool details after a refresh; restore diff syntax highlighting.
- Stabilise compact-block layout; reduce session-header live noise.
- Unified title bar + status pill replace the old banner chrome.

## [0.5.2] — 2026-05-17

### Added
- Session titles use the first user-visible sentence (slash commands with args
  become the title; interrupted / task-output noise filtered).
- Configurable time format, defaulting to 24-hour.
- Custom `Dropdown` component replaces native `<select>` (the popover no longer
  hides the current value on macOS WebView).
- Context panel redesign.

## [0.5.1] — 2026-05-17

### Changed
- Frontend package manager migrated from npm to pnpm.
- Added stable chunk IDs and support for a custom Claude root directory.

## [0.5.0] — 2026-05-16

First tagged release in the 0.5 line — the Rust + Tauri port reaches feature
parity for core session viewing: project discovery, session list with live
refresh, execution-trace rendering (user / AI / tool-call cards), context panel,
global search, desktop notifications, and the multi-segment IPC payload
slimming that keeps thousand-message sessions fast to open.

[Unreleased]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.6.16...HEAD
[0.6.16]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.6.15...v0.6.16
[0.6.15]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.6.14...v0.6.15
[0.6.14]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.6.13...v0.6.14
[0.6.13]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.6.12...v0.6.13
[0.6.12]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.6.11...v0.6.12
[0.6.11]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.6.10...v0.6.11
[0.6.10]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.6.9...v0.6.10
[0.6.9]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.6.8...v0.6.9
[0.6.8]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.6.7...v0.6.8
[0.6.7]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.6.5...v0.6.7
[0.6.5]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.6.4...v0.6.5
[0.6.4]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.6.3...v0.6.4
[0.6.3]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.5.14...v0.6.0
[0.5.14]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.5.13...v0.5.14
[0.5.13]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.5.12...v0.5.13
[0.5.12]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.5.11...v0.5.12
[0.5.11]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.5.10...v0.5.11
[0.5.10]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.5.9...v0.5.10
[0.5.9]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.5.8...v0.5.9
[0.5.8]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.5.7...v0.5.8
[0.5.7]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.5.6...v0.5.7
[0.5.6]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.5.5...v0.5.6
[0.5.5]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.5.4...v0.5.5
[0.5.4]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/snowzhaozhj/claude-devtools-rs/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/snowzhaozhj/claude-devtools-rs/releases/tag/v0.5.0
