# crates/ — Rust workspace（cdt-*）

仅在 Claude 读写 `crates/**` 下的文件时由 Claude Code 自动加载。跨域共识在根 `CLAUDE.md`；IPC payload 模式在 `src-tauri/CLAUDE.md`；spec 工作流在 `openspec/CLAUDE.md`。本文合并自原 `.claude/rules/rust.md`。

## crate 边界（capability → crate）

- `cdt-core`：共享类型 + traits，**no runtime deps**（不依赖 `tokio` / `axum` / `notify` / `ssh2` 等）
- `cdt-parse`：session-parsing（含 JSONL 流式 + dedupe）
- `cdt-analyze`：chunk-building / tool-linking / context-tracking / team-metadata（**sync**，不引 tokio）
- `cdt-discover`：project-discovery / session-search / path-decoder（跨平台路径工具单一源）
- `cdt-watch`：file-watching（自实现 tokio debounce，不用 notify-debouncer-mini）
- `cdt-config`：configuration-management / notification-triggers
- `cdt-ssh`：ssh-remote-context
- `cdt-api`：ipc-data-api + http-data-api facade
- `cdt-telemetry`：application-telemetry — Counter / Histogram / Event Signal Registry + tracing bridge layer（hot path < 0.2% 增量；详 `openspec/specs/application-telemetry/spec.md`）
- `cdt-cli`：binary entrypoint（`bin = cdt`，`anyhow::Result` + `tracing_subscriber` init）

`unsafe` workspace-wide 禁用（`#![forbid(unsafe_code)]`）。

## Rust 约定（仅列项目偏离 / 强调点）

通用规范跟 [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) + clippy pedantic（详 §clippy pedantic 速查）。以下是**项目特有**或**易忽略**：

- **crate 边界**：`cdt-core` / `cdt-analyze` 保 **sync** + no runtime deps；`tokio` 只加到真做 I/O 的 leaf crate；跨 crate import 只走 public API（`pub use` from `lib.rs`），**不**伸进 `crate::internal::...`；共用类型放 `cdt-core`，不跨 leaf re-export
- **Error 二分**：library crate 用 `thiserror` 定 `pub enum <Crate>Error`，非 test 路径**禁** `panic!` / `unwrap()`；binary `cdt-cli` 用 `anyhow::Result<()>` + `.context()`。验证只在系统边界（external input / fs / IPC / HTTP / SSH）
- **`tracing_subscriber` 仅在 `cdt-cli::main` 初始化一次**——library crate 不装 global subscriber；用结构化字段 `tracing::info!(session_id = %id, ...)`
- **日志级别纪律（边界 + 语义双闸门，防 CLI/MCP 噪音回归）**：
  - **边界**：`cdt-cli` 默认**全静默**（`init_logging` 默认 filter `off`），诊断日志是 opt-in（`-v`/`-vv`/`-vvv` = warn/info/debug，或 `RUST_LOG`）。这保证 `--format json/jsonl`、`mcp serve`（stdio JSON-RPC）、管道、终端永不被 tracing 污染，且**与 library 各处用何级别无关**——不靠"默认值恰好够低"赌。命令自身错误走 `anyhow::Result` 由 main 打印，与诊断日志两条路，static `off` 不吞它。**禁止**把这个默认改成 `info`/`warn` 基线（v0.6/0.7 `stats` 刷 321 行 WARN 的根因就是默认 `info` 基线泄漏 library WARN）。
  - **语义**：处理外部数据的**预期内瑕疵**（坏 JSONL 行、重复 tool_use/tool_result id、schema 漂移）SHALL 记 `debug!` **不记 `warn!`**——这类是常态不是异常，逐条 warn 是 warning fatigue 反模式。有计数器的（如 `ToolLinkingResult::duplicates_dropped`）走聚合，不逐条刷。`warn!` 只留给"意外且需要有人采取行动"的事件。参照已对齐的 `cdt-parse::file` / `cdt-discover::project_scanner`。
- **解析双形态**：`parse_entry(line) -> Result<_>`（sync per-line）+ `parse_file(path) -> impl Stream`（async per-file）
- **依赖版本住 workspace 根**：`[workspace.dependencies]`，crate-level 用 `dep = { workspace = true }`；新依赖要 justify
- **测试**：每 capability spec scenario 至少一个 `#[test]`，命名用 prose（如 `fn user_question_then_ai_response_emits_two_chunks`）；snapshot-heavy 用 `insta`
- **注释**：默认无——名字 + 类型承载意义；`pub` 不 self-explanatory 时 `///`（trait contract / 不变量），模块头 `//!` 链 `openspec/specs/<cap>/spec.md`；写 WHY 不写 WHAT

### 测试基础设施陷阱

- **macOS `TempDir` vs FSEvents**：`TempDir` 返 `/var/...` 但 `notify`/FSEvents 返 `/private/var/...`（symlink canonicalization）。涉及路径比较时必须 `canonicalize()`。
- **`notify-debouncer-mini` timer 不受 `tokio::time::pause()` 控制**，测试不确定。优先用 `notify` 裸接 + 自实现 tokio debounce。
- **`cdt-watch` `tests/file_watching.rs` 在 macOS flaky**（FSEvents 时序依赖）；`just test` 单线程补跑也可能 5/6 timeout（**不只是** `burst_of_writes_debounced`）。判真回归：`cargo test -p cdt-watch <test_name>` 单 case 跑——单跑能过即视为环境 flake，可继续 archive；改 watcher 行为时才纠结全套通过。
- **`tokio::time::pause` 测试的 send-advance 顺序**：`#[tokio::test(start_paused = true)]` + `tokio::time::advance` 时，`send → advance` 直觉顺序会失败——loop task 尚未 poll，pending 仍空，`advance` 不触发 flush。正确：`tx.send(...) → yield_now (loop 收 event 写 pending) → advance(duration) → yield_now (sleep_until wake + flush)`。需 `tokio` dev-dep 带 `test-util` feature。例：`cdt-watch::watcher::tests` 5 个 debounce 单元测。
- **insta 快照接受**：没装 `cargo-insta` 就用 `INSTA_UPDATE=always cargo test -p <crate>`；提交生成的 `tests/snapshots/*.snap`。
- **同步解析入口**：`cdt-analyze` 集成测试不引入 tokio——用 `cdt_parse::parse_entry_at(line, n)` 逐行解析 fixture，再跑 `dedupe_by_request_id`。
- **后台服务的本机路径参数化**：涉及 `~/.claude/projects/` 的后台服务（notifier、未来的 history scanner）不要在函数内直接 `path_decoder::get_projects_base_path()`，显式从构造器传 `projects_dir: PathBuf`，否则集成测试会命中真实本机路径。

## Serde / IPC 契约

- **camelCase**：所有面向前端（Tauri IPC）的 struct 必须 `#[serde(rename_all = "camelCase")]`；enum 用 `rename_all_fields = "camelCase"` 给字段、`rename_all = "snake_case"` 给 tag 值。**例外**：`TokenUsage` 保持 snake_case（与 Anthropic API 原始格式一致）。
- **缩写处理**：`auto_expand_ai_groups` → `autoExpandAiGroups`（缩写当普通词，**不**会大写成 `AIGroups`）。所有 `xx_ai_yy` / `xx_http_yy` / `xx_ssh_yy` 类字段在前后端两侧的 key 都按 `xxAiYy` 写；用 ipc_contract round-trip test 拦截大小写错配（历史 bug：`autoExpandAIGroups` 与前端 `autoExpandAiGroups` 错配，toggle 历久不持久化）。
- **`ConfigManager::update_<section>` 是手写白名单 dispatch**：未列出 key 走 `_ => {}` 静默丢弃——加 `GeneralConfig` / `DisplayConfig` / `UpdaterConfig` 等字段时 SHALL 同步在 `crates/cdt-config/src/manager.rs::update_<section>` 加 match 分支（含 enum 字符串校验），并在 `crates/cdt-api/tests/ipc_contract.rs` 加 `update_config_<section>_<field>_round_trip` 测试（默认值 + 改写 + 改回 + 非法值拒绝）。否则 SettingsView 改完看似生效（前端乐观更新），重启后丢失。
- **`ContextInjection` serde 格式**：`#[serde(tag = "category", rename_all = "kebab-case")]` 是 internally-tagged，JSON 为 `{ "category": "claude-md", "id": "...", ... }`（**不是** `{ "ClaudeMd": {...} }`）。前端按 `inj.category` 字段 switch 匹配。
- **`AppConfig::keyboard_shortcuts: HashMap<String, String>`**（change `add-keyboard-shortcut-system`）：camelCase `keyboardShortcuts`；仅 `#[serde(default)]` **不加** `skip_serializing_if`——empty HashMap SHALL 序列化为 `{}` 同时满足 IPC 字段必含 + 文件持久化简洁双约束。

## chunk-building 语义契约（详 `openspec/specs/chunk-building/spec.md`）

`is_meta` / slash / interruption / teammate-message 四类消息的完整行为契约在 spec（Scenario 级覆盖）。port 专属踩坑：

- **`is_meta` 过滤**：跳过产 `UserChunk`，但 `tool_result` 仍合并到 assistant buffer（spec 待补）
- **Slash 双产出 + 紧邻约束**：slash user 消息既要产 `UserChunk`（UI 气泡）又要挂到下一个 `AIChunk.slash_commands`；`instructions` 来自 `is_meta=true + parent_uuid=slash.uuid` 的 follow-up；普通 user 消息产 `UserChunk` 前必须 `pending_slashes.clear()`。TS 原版通过"只看紧邻前 UserGroup"实现，勿回退
- **Interruption 分类**：`[Request interrupted by user` 起首的 user 消息是 `MessageCategory::Interruption`（**非** hard noise），产 `SemanticStep::Interruption` 追加到前一 AIChunk。TS 侧曾当 hard noise 过滤，port 已反向修复，勿回退
- **Teammate-message 多 block + 嵌入 AIChunk**：`<teammate-message teammate_id="..." ...>body</teammate-message>` 的 user 消息**不**产 `UserChunk`，转化为 N 条（一条 user msg 含多块时各产一条）`TeammateMessage` 注入下一个 flush 出的 `AIChunk.teammate_messages`。**禁止**用朴素 `text.find('>')` + `strip_suffix("</teammate-message>")` 解析——多 block 时会把所有块的 body 串成一段丢失。SHALL 用 `cdt-analyze::team::detection::parse_all_teammate_attrs`（global regex）。回滚开关 `EMBED_TEAMMATES: bool` 在 `chunk::builder` 顶部

## port 状态判定要查兜底

原版纯算法 ts（`sessionStateDetection.ts` / `tokenFormatting.ts` 等）只定结构性判定；最终落到 UI 的字段（`isOngoing` / `messageCount` / `gitBranch`...）常在 `src/main/services/discovery/ProjectScanner.ts` 等**调用方**叠加 mtime / count / threshold 兜底。port 时只看算法文件会漏，必须 grep 调用方"该字段被赋值的地方"——本仓 isOngoing 缺 5min `STALE_SESSION_THRESHOLD_MS` 的根因（详见 change `session-ongoing-stale-check`）。

## subagent JSONL 跨 project_dir 关联当前不支持

Claude Code 父 process cwd 被切（worktree / `EnterWorktree`）后，subagent JSONL 写到 cwd 编码的 project_dir，**不是**父 session 的 project_dir。`scan_subagent_candidates` 当前只扫同 project，跨 dir 找不到 candidate → UI 上 Agent 工具显示为 ToolItem 无明细。普通用户不触发；dev / worktree 场景下会复现。关联线索（值得日后修）：父 sessionId 在路径 `<project>/<父sessionId>/subagents/agent-*.jsonl` 目录名里 + `agent-<id>.meta.json` 含 `{agentType, description}` 可二次匹配父 session 的 Agent `tool_use.input.description`。已在 change `worktree-support-and-cross-project-subagent` 修复（`scan_subagent_candidates_cross_project` + `CROSS_PROJECT_SUBAGENT_SCAN: bool` gate）。

## `LocalDataApi` 构造器扩展

注入新基础设施（FileWatcher、SSH pool 等）时新增 `new_with_<xxx>()` 构造器，**不改** `new()` 签名——旧构造器被 `crates/cdt-api/tests/*.rs` 依赖，改签名会批量破坏集成测试。

## `cdt-core` 核心 struct 加字段先 grep 全构造点

在 `AIChunk` / `ToolExecution` 等核心 struct 加非 `Option` 非 `#[serde(default)]` 字段会让 workspace 所有 `Foo { ... }` 构造点编译失败（典型 `AIChunk` 11 处、`ToolExecution` 9 处）。

工作流：
1. `grep -rn "<StructName> {" crates --include="*.rs"` 先列全清单
2. 一轮 Edit 全部补齐再 `cargo check`——避免 PostToolUse clippy hook 在单文件 Edit 间反复阻塞
3. 新字段尽量 `Option<T> + #[serde(default, skip_serializing_if = "Option::is_none")]` 让加字段对老 fixture / 老前端无破坏

## IPC vs HTTP 行为分叉

trait 加默认方法 fallback 到通用版本，`LocalDataApi` 自己 override 真版本——其他实现安全降级。例：`DataApi::list_sessions_sync` 默认调 `list_sessions`（骨架），LocalDataApi override 为同步全扫；**`list_sessions_sync` 保留作为 trait fallback，但 axum HTTP route 已切换到与 IPC 共用的 `list_sessions`（骨架 + SSE push）实现**（change `unify-session-list-loading-strategy`）。

## 后台任务 per-key 取消

触发新一轮后台扫描前需 abort 同 key 的旧任务时，用：

```rust
Arc<std::sync::Mutex<HashMap<K, ScanEntry { generation: u64, handle: AbortHandle }>>>
+ Arc<AtomicU64> 计数器
```

**abort 旧 + 分配 generation + `tokio::spawn` + insert 必须在同一把 sync lock 临界区内**——`tokio::spawn` 不 await（只 enqueue task），sync lock 持有期间调用安全；分两次 lock acquire 会让并发 A/B 调用的 insert 互相覆盖，孤立 task 没人能 abort（PR #38 codex 二轮 race）。任务尾部 cleanup 时只在 `entry.generation == my_generation` 才 remove，避免旧 task 误删新 entry。例见 `LocalDataApi::list_sessions` 的 `active_scans`。

## Windows 兼容硬约束（详 change `windows-platform-support`）

- **home 解析**：凡需 `~/.claude/` 或用户 home 的代码都调 `cdt_discover::home_dir()`，**不要**直接 `dirs::home_dir()`。前者四级 fallback `HOME → USERPROFILE → HOMEDRIVE+HOMEPATH → dirs::home_dir()`，对齐 TS `pathDecoder.ts::getHomeDir`；后者在 Windows 上若 `USERPROFILE` 未设但 `HOMEDRIVE+HOMEPATH` 设了会返 None，fallback 到 `.` 导致找不到数据目录。
- **绝对路径判断**：凡校验/接受绝对路径的代码都用 `cdt_discover::looks_like_absolute_path(&str)`，**不要**直接 `Path::is_absolute()`。前者跨平台识别 POSIX `/foo`、Windows `C:[\/]...`、UNC `\\...`；后者只认当前平台风格，Windows 上拒 POSIX（但 SSH 远端 / WSL / JSONL `cwd` 字段都可能是 POSIX）。
- **路径编解码**：`encode_path` / `decode_path` / `is_valid_encoded_path` 是**跨 crate 唯一实现源**，在 `cdt_discover::path_decoder`。`cdt-config::claude_md`、`cdt-api/tests/agent_configs.rs` 等调用方 `use cdt_discover::encode_path`，**禁止**再写私有副本（历史有两份分叉副本，是 Windows auto-memory 找不到文件的根因之一）。
- **Windows NTFS 目录名禁用字符**：`< > : " / \ | ? *` 不能做文件/目录名。测试 fixture 里用 `encode_path(r"C:\Users\...")` 会产 `-C:-Users-...` 含 `:`，Windows CI 上 `create_dir_all` 报 error 267 NotADirectory。凡需"真在磁盘上建 encoded project 目录"的集成测试（见 `crates/cdt-api/tests/agent_configs.rs`），用纯字母/数字/`-` 的 hardcoded 名（如 `-ws-my-proj`），cwd 真实磁盘路径由 JSONL `cwd` 字段提供，scanner 依赖字段不依赖 encoded 名与磁盘路径的对应。

## clippy pedantic 速查

workspace 开启 pedantic，PostToolUse hook 每次 `.rs` 编辑后自动跑 clippy 报错。最常踩的：

- `doc_markdown`（注释里标识符要反引号）
- `cast_possible_wrap`（`u64 as i64` → `i64::try_from`）
- `uninlined_format_args`（`format!("{}", x)` → `format!("{x}")`）
- `map_unwrap_or`（`x.map(f).unwrap_or(d)` → `x.map_or(d, f)`）
- `is_some_and`（`opt.map(f).unwrap_or(false)` → `opt.is_some_and(f)`）
- `if_not_else`（`if x != y { A } else { B }` → 倒顺序）
- `manual_let_else`（`match X { Ok(v)=>v, Err(_)=>return ... }` → `let Ok(v) = X else { return ... }`）

其余照 clippy 输出修即可。

## Spec fidelity

- capability 实现以 `openspec/specs/<cap>/spec.md` 为真相源
- 每个 `SHALL` 至少一个测试
- TS 与 spec 冲突时（见 `openspec/TS_BASELINE_DEVIATIONS.md`）跟 **spec**，不跟 TS；在 change 的 tasks.md 里记下 deliberate divergence
