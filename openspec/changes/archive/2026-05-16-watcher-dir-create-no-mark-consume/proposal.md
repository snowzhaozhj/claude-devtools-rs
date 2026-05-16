## Why

新建 project 时 sidebar 永不刷新，必须重启应用才能看到新出现的 project（用户截图复现：「选择项目 ▾」下拉与会话列表都看不到刚通过 `claude` 在新目录创建的 project）。这是发版后用户视角的明显回归。

根因：`crates/cdt-watch/src/watcher.rs::parse_project_event` 的顶层 dir-create 分支调用了 `mark_project_seen(project_id)`，把"首次见到 project"的标记**消耗**掉了。等紧随而来的第一条 `.jsonl` 写入事件再调用 `mark_project_seen` 时拿到 `false` → emit `project_list_changed=false` → 前端 `Sidebar.svelte` 的 file-change handler 既不进 `loadProjects(true)` 分支（要求 `projectListChanged=true`），又因 `payload.projectId !== currentProjectId` 走不到 session 增量分支 → sidebar 永不刷新。

而 dir-create 事件 emit 的 `project_list_changed=true` 触发的 `loadProjects(true)` 也救不了：`crates/cdt-discover/src/project_scanner.rs::scan_project_dir` 对没有 `.jsonl` 的空目录 `if session_stats.is_empty() { return Ok(Vec::new()); }` 直接跳过，新 project 进不了 scan 结果。

实际触发：用户在某新目录第一次跑 `claude`，Claude Code 先 mkdir、之后用户输第一条消息才写 jsonl，dir-create 与 jsonl-create 之间间隔常超 100ms debounce 窗口；即使在同一窗口内，dir 事件先 flush 也会触发空 scan。

## What Changes

- `crates/cdt-watch/src/watcher.rs::parse_project_event` 顶层 dir-create 分支硬编码 `project_list_changed: true`，**不**调用 `mark_project_seen`，把首次 mark + emit `project_list_changed=true` 的权利留给紧随的 jsonl 事件。
- 后续第一条 jsonl 事件首次到达时 `mark_project_seen` 第一次返回 `true` → emit `project_list_changed=true` → 前端再次 `loadProjects(true)` → 此时 scan 能看到 jsonl → 新 project 出现在 sidebar。
- spec `file-watching` 的 `Watch project directory additions` Requirement 加新 Scenario，明确"dir-create 后接 jsonl-create 时，jsonl 事件 SHALL 仍 emit `project_list_changed=true`"以及"dir-create 事件 MUST NOT 通过 `mark_project_seen` 消耗首次标记"。
- 单测覆盖：(a) dir-create + 紧随的 jsonl-create 两次 emit 都 `project_list_changed=true`；(b) dir-create 不写入 `known_projects`。

无 IPC schema / Tauri command / 前端 API 变更。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `file-watching`：MODIFIED Requirement `Watch project directory additions` 收紧 dir-create 与 jsonl 首次写入的协同语义。

## Impact

- code: `crates/cdt-watch/src/watcher.rs` 顶层 dir-create 分支单行修改 + 单测扩展。
- spec: `openspec/specs/file-watching/spec.md`（通过本 change 的 delta `MODIFIED Requirement: Watch project directory additions`）。
- 影响面极小：不改任何"已知 project + jsonl 增量"路径；嵌套 subagent 路径硬编码 `false` 行为完全不变；不改 `mark_project_seen` 自身实现。
- 不引入新依赖、不改 IPC 字段名、不影响 Tauri command 协议。
