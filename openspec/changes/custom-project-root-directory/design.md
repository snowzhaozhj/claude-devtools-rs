## Context

当前 Rust 端已有 `GeneralConfig::claude_root_path` 字段和基本校验，但多个运行时入口仍各自调用默认路径 helper，实际行为固定在用户 home 下的 `.claude`。TS 原版通过配置中的 Claude root override 统一派生 `projects` 与 `todos` 路径，并在配置更新后重配本地服务上下文。本 change 需要把 Rust 端的路径来源收敛为一个可注入的运行时配置，同时保持未配置时的默认行为不变。

## Goals / Non-Goals

**Goals:**

- 默认行为保持使用 home 下 `.claude`。
- `general.claudeRootPath` 非空时，所有本地 Claude 数据读取入口使用该 root 下的 `projects` / `todos`。
- Settings UI 支持查看、通过系统目录选择器选择、手动填写、清空 Claude root，并通过既有 `update_config` 持久化。
- 配置更新后运行中的 scanner/search/API 立即切换到新 root；watcher 在下次启动时使用新 root。
- 用单元/集成/IPC/UI 测试覆盖默认、自定义、清空、非法相对路径场景。

**Non-Goals:**

- 不改变配置文件自身的位置；`claude-devtools-config.json` 仍位于默认用户级配置位置。
- 不新增跨平台文件夹选择器作为唯一入口；如果平台 dialog 成本过高，Settings 可先提供文本输入。
- 不迁移或复制用户现有 `~/.claude` 数据。
- 不改变 SSH 远端上下文路径语义。

## Decisions

### D1: 以 Claude root 派生路径，而不是直接配置 projects root

`general.claudeRootPath` 表示 Claude 数据根目录，运行时从它派生 `<root>/projects` 与 `<root>/todos`。备选方案是新增 `projectsRootPath` 与 `todosRootPath` 两个字段，但会与原版字段脱节，并增加用户配置不一致的风险。选择单 root 后，默认 fallback、自定义路径、清空恢复默认都能与 TS 原版对齐。

### D2: 在 `cdt-discover::path_decoder` 暴露带参数的路径 helper

保留现有 `get_projects_base_path()` / `get_todos_base_path()` 作为默认路径入口，同时新增接受 `Option<&Path>` 或 `Option<&str>` 的 helper，用于从当前配置派生路径。备选方案是在每个 crate 内自行 `root.join("projects")`，但会重新产生路径规则分叉，尤其是 Windows home fallback 与 path encoding 的一致性风险。

### D3: 运行时服务持有可重配的本地上下文

Tauri 启动时读取配置并构建 scanner/searcher/watcher/API 使用的当前路径；`update_config("general", { claudeRootPath })` 成功后重建 scanner/searcher 或更新共享状态，使后续项目列表、session 搜索、CLAUDE.md/auto-memory IPC 使用新 root。watcher 热替换需要管理长期 `start()` task 的取消与广播订阅重接，本轮不实现，改为下次启动使用新 root。备选方案是所有路径都提示用户重启，但会让核心发现/搜索路径与原版差距过大。

### D4: 搜索和 CLAUDE.md 读取不再直接调用全局默认路径

`SessionSearcher` 与 `read_all_claude_md_files` 类入口应接受当前 Claude root/projects root 或由调用侧注入，避免绕过 Tauri 配置。备选方案是在这些函数内部读取全局 config，但会引入配置层反向依赖和测试困难。注入式设计更符合现有 `ProjectScanner::new(projects_dir)` 模式。

### D5: Settings UI 走既有 optimistic update 模式

Settings 中新增 Claude root 控件：主按钮调用系统目录选择器，选中后乐观更新本地 state 并调用 `updateConfig("general", { claudeRootPath })`；文本输入保留为高级/回退路径，提交绝对路径时走同一保存逻辑；失败时重新 `getConfig` 回滚并显示错误；恢复默认表示写入 `null`。备选方案是只提供文本输入，但这与原版体验不一致，用户需要手动复制路径。

## Risks / Trade-offs

- [Risk] root 更新后 watcher 仍监听旧目录直到应用重启。→ Mitigation: Settings 文案不承诺 watcher 热切换；项目列表和搜索路径立即重配，file-change 自动刷新在下次启动后对齐新 root。
- [Risk] 某些低层函数仍调用默认 `get_projects_base_path()`，导致路径混用。→ Mitigation: grep 全局默认 helper 调用点，业务路径改为注入式，并用自定义 root 集成测试覆盖。
- [Risk] 相对路径被归一化为 `None` 可能让用户误以为保存成功但实际恢复默认。→ Mitigation: spec 改为非法相对路径更新 SHALL 拒绝并保持旧值，UI 显示错误。
- [Risk] 配置文件仍在默认 `.claude` 下，用户可能误解为随 root 切换。→ Mitigation: Settings 文案说明该字段仅控制 Claude 数据根目录，不迁移 devtools 自身配置。
