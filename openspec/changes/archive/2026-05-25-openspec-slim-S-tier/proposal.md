## Why

S+XS 档（≤ 5 capability，但反引号密度仍偏高）共 18 个 spec 文件累计 ~3.4k 行、~1.0k 反引号；同 capability 边界下塞进了大量"内部 fn 名 / 源码路径 / `tracing::xxx!(target:...)` 实现选择 / IPC contract test 级 scenario / 每枚举一个 scenario"等不属于行为契约的内容。后果：

- spec 当 implementation note 用，主 SHALL 句被实现细节冲淡；`openspec/CLAUDE.md::硬约束 1`（spec 是行为契约真相源，不是实现注释）失守。
- reviewer 评估行为变更时要先过滤掉 module 路径噪声；codex / spec-fidelity-reviewer 二审命中假阳性高。
- IPC contract test 级 scenario（`EXPECTED_TAURI_COMMANDS` / `KNOWN_TAURI_COMMANDS` 同步）落在多个 cap spec 内，本属 `frontend-test-pyramid::Rust IPC contract test 守护字段形状` 一处兜底，散落 spec 没增量价值。

不动 capability 边界（边界重构走 GitHub Issue #296）。本 change 仅把 18 个 S+XS cap 各自 spec 内能按 6 条尺子识别为非行为契约的内容删除或下沉，不改 normative SHALL/MUST 句的语义。

## What Changes

按 6 条尺子瘦身 16 个 cap 的 spec.md（其中 2 个 cap—— `session-search` / `notification-ui`——扫描后已干净，**不**写 delta）：

- **MODIFY** `settings-ui`：Diagnostics tab 4 区域 SVG / bucket / loading 中间态实现细节，`KeyRecorderInput.svelte` / `ShortcutRow.svelte` / `KeyboardShortcutsPanel.svelte` 等 svelte 组件名引用，录键状态机 6 步实现细节，`cdt-config::keyboard_shortcuts` 模块路径——降为行为契约
- **MODIFY** `app-auto-update`：`tracing::error!(target: "cdt_tauri::updater", ...)` / `tracing::debug!(...)` / `tracing::warn!(...)` / `tracing::info!(...)` 五处实现选择 SHALL，IPC contract test scenario（`EXPECTED_TAURI_COMMANDS` / `KNOWN_TAURI_COMMANDS` / `invoke_handler!` 三处同步），配置链一致性 scenario 内部 6 行源码路径列举——降为行为契约
- **MODIFY** `team-coordination-metadata`：`team::reply_link::link_teammate_to_send_message` / `parse_all_teammate_attrs` / `parse_teammate_attrs` / `build_pending_teammates` / `cdt-analyze::team::noise` / `detect_noise` / `detect_resend` 等 fn 与 module 路径，原版 `TeammateMessageItem.tsx::RESEND_PATTERNS` TS 路径——降为行为契约
- **MODIFY** `notification-triggers`：`tracing::debug!(target: "cdt_watch::ssh_polling", ...)` 实现选择——删除
- **MODIFY** `file-watching`：`FileWatcher::route_event` / `dunce::canonicalize` / `cdt-discover::path_compare` 实现路径，`tracing::debug!` mtime 缺失提示——降为行为契约
- **MODIFY** `session-parsing`：`cdt-parse` / `cdt_parse::dedupe_by_request_id` 模块路径——保留 fn 名作为公开 API 契约边界，去 `cdt-parse` 前缀引用
- **MODIFY** `memory-viewer`：`cdt-fs::FileSystemProvider` trait / `validate_memory_file_name` / `fs.write_atomic` / `fs.create_dir_all` / `fs.remove_file` 等内部 fn / trait method 名，`memory-viewer` UI（`ui/src/lib/views/MemoryView.svelte`）当前不规约注释——降为行为契约；`Sidebar.memoryCache` 状态名改成行为表述
- **MODIFY** `wsl-distro-discovery`：`normalize_wsl_home_path(input: &str) -> Option<String>` fn 签名——保留行为，删类型签名
- **MODIFY** `tab-management`：滚动状态恢复策略中 `MutationObserver` `subtree: true / attributes: true / attributeFilter` 等具体参数，hotkey 列举 8 个 scenario 合并为"行为类别 + spec id 白名单"
- **MODIFY** `application-telemetry`：`cdt-api/tests/perf_telemetry_overhead.rs` / `crates/cdt-api/src/ipc/local.rs` / `scripts/check-no-hot-event.sh` 等源码路径——降为行为契约（perf 测试存在 + hot-path 不写 event 的检查存在）
- **MODIFY** `server-mode`：`crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS` / `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS` / `src-tauri/src/lib.rs::invoke_handler!` 三处同步段（IPC contract test 兜底）—— 删除；`ui/src/lib/api.ts` / `ui/src/lib/transport.ts` 路径——删除
- **MODIFY** `context-tracking`：`process_session_context_with_phases(chunks, params)` 函数签名——保留行为，删签名形态
- **MODIFY** `frontend-test-pyramid`：第 31 行罗列 22 个 Tauri command 名 + 4 个 listen event 名——合并为"`invoke_handler!` 注册的全部 Tauri command + 已注册 listen event"行为类别；5 个 user story spec 文件名列举——保留（属本 cap 自身契约目标）
- **MODIFY** `ui-search`：`SearchBar.svelte` / `CommandPalette.svelte` 自身 listener / `onBeforeSearch` / `flushAll()` 等内部 fn / 组件名——降为行为表述
- **MODIFY** `agent-configs`：`LocalDataApi::read_agent_configs` / `cdt_discover::agent_configs::read_agent_configs(pairs)` 模块路径——保留 IPC 边界 fn 名（`read_agent_configs`），删模块前缀
- **MODIFY** `app-chrome`：`<div class="app-root">` / `<div class="app-layout">` CSS class 名 / `navigator.userAgent.includes("Macintosh")` 平台检测实现 / `box-shadow: inset 0 -2px 0 var(--color-accent)` CSS 实现——降为行为契约
- **NOT-CHANGED** 6 条尺子之外的内容：`session-search` / `notification-ui` 扫描后已干净；其余 cap 的 IPC 字段名 / 字段语义 / 数据 omit 策略 / Tauri command 协议 / 性能预算等 normative 句一律保留

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `settings-ui` / `app-auto-update` / `team-coordination-metadata` / `notification-triggers` / `file-watching` / `session-parsing` / `memory-viewer` / `wsl-distro-discovery` / `tab-management` / `application-telemetry` / `server-mode` / `context-tracking` / `frontend-test-pyramid` / `ui-search` / `agent-configs` / `app-chrome`：删去内部 fn / 模块路径 / `tracing::xxx!(target:...)` 实现选择 / IPC contract test 级 scenario / "每枚举 / 每按键 / 每 hotkey 一个 scenario" 颗粒过细 scenario / 原版 TS 文件路径；保留 IPC 字段名 / 字段语义 / 行为契约 normative 句
