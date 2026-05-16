## Context

`cdt-watch` 的 `FileWatcher` 在 100 ms debounce 后，以两条独立路径把 `~/.claude/projects/` 下的变更归一化为 `FileChangeEvent`：

1. 顶层 dir-create（`components.len() == 1 && path.is_dir()`）→ emit `{session_id: "", project_list_changed}`
2. 二层 jsonl 写入（`<project>/<sess>.jsonl`）→ emit `{session_id: <id>, project_list_changed}`

两条分支都通过 `mark_project_seen(project_id)` 派生 `project_list_changed`：HashSet 第一次 insert 返回 `true`、之后 `false`。`known_projects` 在 watcher 启动时由 `initial_projects(projects_dir)` 预热为已存在目录集合。

前端 `Sidebar.svelte::registerHandler` 严格按 `payload.projectListChanged === true` 触发 `loadProjects(true)`；`payload.projectId !== currentProjectId` 时 session 增量分支直接 return。`scheduleRefresh("sidebar:projects", ...)` leading + 250 ms trailing 节流，第一次 leading 立即跑、窗口内后续合并。

`cdt-discover::ProjectScanner::scan_project_dir` 对没有 `.jsonl` 的空目录返回 `Vec::new()`（`session_stats.is_empty()` 短路），不会产出 `Project` 条目。

实测时序：用户在新目录第一次跑 `claude` 时 Claude Code 先 mkdir 编码 project 目录、用户输第一条消息后才落 `.jsonl`，两者间隔常 > 100 ms（甚至几秒）；即使在同一 100 ms debounce 窗口内，dir 事件因 `pending` 中较早的 `Instant` 也总是先 flush。

## Goals / Non-Goals

**Goals：**

- 修复"新建 project 时 sidebar 永不刷新"的回归。
- 收紧 spec：明确 dir-create 与 first-jsonl 协同下"两次都 emit `project_list_changed=true`"的契约，避免未来再次回归。
- 修法范围最小、不引入新状态、不改 IPC schema。

**Non-Goals：**

- 不重写 debounce / 事件路由结构。
- 不改 `mark_project_seen` 自身实现或 `known_projects` 数据结构。
- 不修改 `ProjectScanner::scan_project_dir` 对空目录的"返回空"行为（保留 spec `project-discovery` 现有契约不变；本次只在事件源头让 jsonl-create 总能触发"重扫信号"）。
- 不动嵌套 subagent JSONL 的"硬编码 `project_list_changed=false`"分支。

## Decisions

### D1：dir-create 分支硬编码 `project_list_changed=true`，**不**调用 `mark_project_seen`

- **方案**：`parse_project_event` 顶层 dir-create 分支直接构造 `project_list_changed: true`，删除 `let project_list_changed = self.mark_project_seen(&project_id);` 行。`known_projects` 的首次 insert 由紧随的 jsonl 事件（`watcher.rs:207`）独占。
- **效果**：
  - 单纯 dir-create（不带 jsonl）：emit `project_list_changed=true` → 前端 `loadProjects(true)`，scan 看到空目录会跳过——但这是无 jsonl 的空目录，本来就没 session 可显示，符合 `project-discovery` spec。
  - dir-create 后接 first-jsonl：dir 事件 emit `project_list_changed=true`（前端 leading 跑一次，scan 此刻可能看不到 jsonl）；jsonl 事件 `mark_project_seen` 第一次返回 `true` → emit `project_list_changed=true`（前端 trailing/下一轮 leading 再跑一次，scan 看到 jsonl）。两次刷新被前端 250 ms 节流合并为 1～2 次 IPC，最终 sidebar 出现新 project。
  - 单纯 jsonl-create（无独立 dir-create 事件，FSEvents 合并场景）：jsonl `mark_project_seen` 第一次返回 `true`，行为不变。
- **候选方案**：

  | 方案 | 做法 | 拒因 |
  |---|---|---|
  | A（**采纳**）：dir 不调 mark | dir 硬编码 `true`，由 jsonl 独占首次 mark | 修法最小；事件源头解决；测试可确定性覆盖 |
  | B：scanner 不跳过空目录 | `scan_project_dir` 返回 `Project { sessions: [] }` | 改 spec `project-discovery` 行为，影响面更大；空 project 在 sidebar 上显示也不合理；不解决"jsonl emit `project_list_changed=false`"导致后续刷新缺失的根本 |
  | C：移除 dir-create 事件源 | 不发送顶层 dir 事件 | 违反现有 spec `Watch project directory additions` Scenario "New project directory created"——发现"目录已建但永远不写 jsonl"的极端场景下用户会以为 watcher 死了；测试也已断言此事件存在 |
  | D：`mark_project_seen` 改成"读但不 insert" | 双 emit `true` 直到某个事件后再 insert | 引入新状态机（"暂态已见 vs 真已见"），复杂度上升；难界定 insert 时机；与 `initial_projects` 预热行为分裂 |

### D2：spec `Watch project directory additions` 加 Scenario 明确组合契约

- 现有 Scenario "New project directory created" 与 "First session file in new project created" 单独看都被实现满足，但未约束两者协同时是否要双 emit `project_list_changed=true`——这正是当前 bug 的盲区。
- 加新 Scenario "dir-create followed by first jsonl both signal project list change"：同一 watcher 内先后触发 dir-create + 紧随的 jsonl-create，两次事件 SHALL 都 `project_list_changed=true`。
- 加内部约束："顶层 dir-create 分支 MUST NOT 调用 `mark_project_seen` 写入 `known_projects`，首次 mark + emit `project_list_changed=true` 的权利属于第一条 jsonl 事件"。
- 这是 MODIFIED Requirement（在原 Requirement body 内追加 SHALL/MUST 句 + 新 Scenario），不替换原有 Scenarios。

### D3：测试覆盖在 cdt-watch 单测层（`#[cfg(test)] mod tests`），不依赖 `notify` 端到端

- `crates/cdt-watch/tests/file_watching.rs` 在 macOS 已知 flaky（FSEvents 时序），CLAUDE.md 明文指向"优先单测覆盖"。
- 直接调用 `parse_project_event` + 检查 `known_projects` HashSet：
  - 新增 `parse_project_event_dir_create_does_not_consume_mark`：dir-create 后 jsonl-create，断言两次 `project_list_changed=true` 且 dir-create 后 `known_projects` 不含该 project。
  - 加固既有 `parse_project_event_marks_new_top_level_project_directory`：追加 `assert!(!watcher.known_projects.lock().unwrap().contains(...))` 断言 dir-create 不写入 `known_projects`。
- `tests/file_watching.rs` 不改（避免引入新 flaky 用例）。

## Risks / Trade-offs

- **重复 IPC 风险**：dir + jsonl 两个事件都 emit `project_list_changed=true`，触发两次 `loadProjects` 调用。前端已有 `scheduleRefresh` leading + 250 ms trailing 节流合并；最坏情况 dir 间隔 > 250 ms 后 jsonl 才到，跑两次完整 IPC。`list_repository_groups` 实测 ~89 ms（v0.4.10 基线，27 project × 534 session），两次 ≈ 178 ms 一次性突发，可接受。→ Mitigation：保留前端节流；不再加后端去抖，避免并发状态机复杂度。
- **dir-only 场景新 project 不显示**：用户极端场景"建空目录但永不写 jsonl"时 sidebar 仍不出现新 project。这与原版行为一致（无 session 的 project 不展示），且符合 `project-discovery::scan_project_dir` 的现有契约。→ Mitigation：non-goal，不在本 change 处理。
- **删除型 race**：用户 mkdir 后立刻 rmdir，dir-create 事件仍会 emit `project_list_changed=true`，前端 scan 时找不到目录——`scan_project_dir` 对不存在目录已经容错（`fs.read_dir_with_metadata` 报错被 `tracing::warn!` 吞掉），UI 看到的就是"啥都没变"，无副作用。

## Migration Plan

无数据迁移、无 IPC 协议变化。改动随 PR merge 立即生效。回滚策略：revert 单 commit 即可。

## Open Questions

无。
