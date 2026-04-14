# claude-devtools-rs

Rust port of [claude-devtools](../claude-devtools) — the Electron app that
visualizes Claude Code session execution.数据层 13 个 capability 已全部完成，
当前工作重心是 UI 层（Tauri 2 + Svelte 5 桌面应用）。

## Parent repo

The TypeScript source is at `/Users/zhaohejie/RustroverProjects/claude-devtools`.
It is the historical reference only；所有行为契约以 `openspec/specs/` 为准。
TS 侧的 7 个 impl-bug 已全部在 Rust port 中修复（详见 `openspec/followups.md`）。

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
├── ui/                       # Svelte 5 + Vite 前端
├── src-tauri/                # Tauri 2 Rust 后端 (excluded from workspace)
├── openspec/
│   ├── specs/                # 13 capability specs (authoritative)
│   ├── followups.md          # TS impl-bugs to fix, not replicate
│   └── README.md             # workflow + capability map
└── .claude/rules/rust.md     # Rust coding conventions
```

## Capability → crate map

13/13 全部 done。Capability→crate 映射：`cdt-parse`(session-parsing)、`cdt-analyze`(chunk-building/tool-linking/context-tracking/team-metadata)、`cdt-discover`(project-discovery/session-search)、`cdt-watch`(file-watching)、`cdt-config`(configuration-management/notification-triggers)、`cdt-ssh`(ssh-remote-context)、`cdt-api`(ipc-data-api/http-data-api)。详见 `openspec/changes/archive/`。

## UI 层 (Tauri 2 + Svelte 5)

- `ui/`：Svelte 5 + Vite 前端（暖灰主题，Soft Charcoal 配色，约 40 个 CSS 变量）
- `src-tauri/`：Tauri 2 Rust 后端（独立 Cargo.toml，excluded from workspace）
- `cargo tauri dev`：启动开发模式（Vite HMR + Rust hot reload）
- `src-tauri/` 通过 path deps 引用 `crates/` 下的数据层 crate
- Tauri IPC commands 直接调用 `LocalDataApi`，不走 HTTP
- **布局**：Sidebar（项目选择器 + 日期分组会话列表）+ Main（SessionDetail）双栏持久化
- **已有组件**：BaseItem（可展开项）、StatusDot、OutputBlock（代码块容器）、SearchBar（Cmd+F）、ContextPanel（右侧边栏）、SidebarHeader、Tool Viewer（Read/Edit/Write/Bash/Default）
- **SVG 图标**：`ui/src/lib/icons.ts` 导出 lucide 风格 SVG path 常量（Wrench/Brain/Bot/Terminal 等），BaseItem 通过 `svgIcon` prop 渲染
- **Context Panel 数据流**：后端 `cdt-api` 调用 `cdt-analyze::context::process_session_context_with_phases` 计算 `ContextInjection[]`（6 类结构化数据），通过 `SessionDetail.contextInjections` 传给前端。CLAUDE.md 文件通过 `cdt-config::read_all_claude_md_files` 从文件系统扫描（不在 JSONL 中）。
- **session 元数据**：后端 `cdt-api/session_metadata.rs` 轻量扫描 JSONL 提取标题（前 200 行）+ 消息计数，前端直接使用
- **陷阱**：`src-tauri/` 必须在 workspace `Cargo.toml` 的 `exclude` 列表里；`beforeDevCommand` 从 `src-tauri/` 目录执行，路径用 `../ui`
- **陷阱**：浏览器直接访问 `localhost:5173` 会报 `invoke` undefined——`@tauri-apps/api` 只在 Tauri webview 内可用，测试必须通过 `cargo tauri dev` 的窗口
- **陷阱**：Vite HMR 只更新前端代码；后端 Rust crate 改动后需要 `pkill -f claude-devtools-tauri && cargo tauri dev` 重启（Tauri 的 file watcher 有时不触发自动重编译）。

## Common commands

```bash
cargo build --workspace              # build all crates
cargo test --workspace               # run tests
cargo clippy --workspace --all-targets  # lint (workspace-level lints in Cargo.toml)
cargo fmt --all                      # format
cargo run -p cdt-cli                 # run the CLI binary (HTTP server)
npm install --prefix ui              # install frontend dependencies (first time)
cargo tauri dev                      # launch Tauri desktop app (dev mode)
cargo tauri build --debug            # build desktop app (debug)
cargo build -p cdt-parse             # build one crate in isolation
cargo test -p cdt-analyze            # test one crate
npm run check --prefix ui            # svelte-check + tsc (前端类型检查)
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
- **Serde camelCase**：所有面向前端（Tauri IPC）的 struct 必须 `#[serde(rename_all = "camelCase")]`；enum 用 `rename_all_fields = "camelCase"` 给字段、`rename_all = "snake_case"` 给 tag 值。例外：`TokenUsage` 保持 snake_case（与 Anthropic API 原始格式一致）。
- **`ContextInjection` serde 格式**：`#[serde(tag = "category", rename_all = "kebab-case")]` 是 internally-tagged，JSON 为 `{ "category": "claude-md", "id": "...", ... }`（不是 `{ "ClaudeMd": {...} }`）。前端按 `inj.category` 字段 switch 匹配。
- **`is_meta` 消息过滤**：JSONL 中 `isMeta: true` 的 user 消息（skill prompt、system-reminder 注入）在 `build_chunks` 中跳过，不产出 `UserChunk`；但其中的 `tool_result` 仍合并到 assistant buffer。
- **Svelte 5 `$effect` 依赖陷阱**：`$effect` 中读取的所有响应式变量自动成为依赖。若需要在 effect 中读取但不触发重跑的变量，用 `untrack(() => variable)` 包裹。典型场景：session 切换 effect 中清理搜索状态。
- **Svelte 5 `@const` 位置限制**：`{@const}` 只能是 `{#if}`/`{:else}`/`{#each}`/`{#snippet}`/`<Component>` 的直接子级，不能放在 `<div>` 等 HTML 元素内。需要在块开头集中声明。
- **前端渲染依赖**：`marked`（markdown→HTML）+ `highlight.js`（语法高亮，按需加载语言）+ `dompurify`（XSS 防护）。highlight.js 不引入预制主题 CSS，用 `app.css` 中自定义 Soft Charcoal token 颜色。
- **原版 UI 参考**：前端文本清洗逻辑移植自 `../claude-devtools/src/shared/utils/contentSanitizer.ts`（`sanitizeDisplayContent`）。扩展 UI 功能时优先查原版 `src/renderer/` 和 `src/shared/` 对应实现，直接移植而非自己造轮子。
- **Tauri IPC 透传**：`src-tauri/src/lib.rs` 的 commands 返回 `serde_json::Value`，`cdt-api` 类型扩展字段（如 `SessionSummary` 加 `title`）自动透传，不需要改 Tauri 层。
- **clippy pedantic**：workspace 开启 pedantic，PostToolUse hook 会在每次 `.rs` 编辑后自动跑 clippy 报错。最常踩的：`doc_markdown`（注释里标识符要反引号）、`cast_possible_wrap`（`u64 as i64` → `i64::try_from`）、`uninlined_format_args`（`format!("{}", x)` → `format!("{x}")`）。其余照 clippy 输出修即可。
- **insta 快照接受**：没装 `cargo-insta` 就用 `INSTA_UPDATE=always cargo test -p <crate>`；提交生成的 `tests/snapshots/*.snap`。
- **同步解析入口**：`cdt-analyze` 的集成测试不引入 tokio——用 `cdt_parse::parse_entry_at(line, n)` 逐行解析 fixture，再跑 `dedupe_by_request_id`。
- **自动化**：
  - Hooks（`.claude/hooks/`）：`.rs` 编辑后自动跑所属 crate 的 `cargo clippy -- -D warnings`；`git commit` 前自动跑 `openspec validate --strict`。**`openspec/specs/**` 的直接编辑由约定（不是 hook）约束** —— spec 变更必须走 `openspec/changes/<name>/specs/` 的 delta，由 `/opsx:archive` 时 sync 回主 spec。
  - Subagent：`spec-fidelity-reviewer` 按 capability 审计 scenario→test 覆盖。
  - Skill：`/ts-parity-check <capability>` 对比 TS 源与 Rust 端口 + followups。
  - MCP：`.mcp.json` 注册 GitHub MCP，需要 `GITHUB_PERSONAL_ACCESS_TOKEN` 环境变量。
- **opsx:apply 推进节拍**：详见 `.claude/rules/opsx-apply-cadence.md`。核心：Edit → clippy → fmt → test → npm check → validate → 勾 checkbox → 文本总结，不得中途停手。
- Detailed rules: `.claude/rules/rust.md`.

## UI 已知遗留问题

- **Subagent 数据为空**：`AIChunk.subagents` 依赖 `resolve_subagents` 做跨 session 解析，当前 API 层未集成
- **Slash 命令不在 chunks 中**：slash 在 `isMeta` user 消息中，被 `build_chunks` 过滤，summary 统计中缺失
- **AI header tool call 计数偏少**：需对比原版 `displaySummary.ts` 的 `buildSummary` 逻辑确认差异来源

## What to do first in a fresh session

1. Run `cargo build --workspace` 确认 data layer 可编译；`cargo test --workspace` 跑一遍回归。
2. 13 个 data layer capability 已全部完成。当前工作重心是 UI 层（Tauri + Svelte）。
3. `cargo tauri dev` 启动桌面应用验证当前状态。
4. UI 功能迭代仍走 openspec 工作流：`/opsx:propose <feature>` → `/opsx:apply` → `/opsx:archive`。纯前端 UI 改动（不涉及数据层 spec）可简化：proposal + tasks 即可，design 可选，specs 跳过。或小改直接写 + commit 不走 openspec。
