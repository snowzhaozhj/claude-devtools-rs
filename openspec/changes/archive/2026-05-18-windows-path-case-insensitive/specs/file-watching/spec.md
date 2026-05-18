## ADDED Requirements

### Requirement: Route watch events case-insensitively on Windows

`FileWatcher::route_event` SHALL 在判定一个 `notify` 回调路径是否落入被监视目录时使用跨平台规范化的前缀匹配 helper，使**Windows 平台**上 `notify` 回调返回的大小写与 `dunce::canonicalize` 后的 `projects_dir` / `todos_dir` 不一致时仍能正确路由事件，**非 Windows 平台**保持字节精确比较。

`FileWatcher` 的 `known_projects: HashSet<PathBuf>` 去重容器 SHALL 使用同一规范化策略——同一 project 目录在 Windows 上无论以何种大小写出现都 SHALL 只占一个 HashSet 条目；首次见到该 project 的 mark 语义不被大小写漂移破坏。

跨平台规范化 helper SHALL 与 `project-discovery::Compare paths case-insensitively on Windows` Requirement 共享同一来源（`cdt-discover::path_compare`），不允许 `cdt-watch` 自行实现 lowercase / startsWith 逻辑。

#### Scenario: Windows 上 notify 大小写漂移仍正确路由

- **WHEN** 在 Windows 平台运行，`projects_dir = C:\Users\Alice\.claude\projects`，`notify` 回调返回路径 `c:\users\alice\.claude\projects\-Users-Alice-app\session-1.jsonl`
- **THEN** `FileWatcher::route_event` SHALL 把该事件归入 `projects` 命名空间
- **AND** SHALL 正确剥离前缀提取出 `project_id = "-Users-Alice-app"` 与 `session_id = "session-1"`

#### Scenario: 非 Windows 平台保持精确前缀匹配

- **WHEN** 在 Linux 或 macOS 平台运行，`projects_dir = /home/alice/.claude/projects`，`notify` 回调返回路径 `/home/Alice/.claude/projects/-foo-bar/session.jsonl`（注意 `Alice` vs `alice`）
- **THEN** `FileWatcher::route_event` SHALL 不把该事件视为 `projects_dir` 子项（前缀不匹配）
- **AND** 不发出 `FileChangeEvent`

#### Scenario: known_projects 在 Windows 上对大小写漂移去重

- **WHEN** 在 Windows 平台运行，`mark_project_seen` 先以 `C:\projects\foo` 的形式插入，后以 `c:\projects\FOO` 查询
- **THEN** 第二次查询 SHALL 报告 "已见过"，`mark_project_seen` SHALL 返回 `false`，`known_projects` 内部 SHALL 仅含一个条目
