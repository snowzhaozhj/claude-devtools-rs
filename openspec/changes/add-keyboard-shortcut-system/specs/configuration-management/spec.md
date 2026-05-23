## ADDED Requirements

### Requirement: Persist keyboard shortcut overrides

`Config` 结构 SHALL 新增字段 `keyboard_shortcuts: HashMap<String, String>`，序列化时 SHALL 使用 serde `rename_all = "camelCase"`（IPC 字段名 `keyboardShortcuts`）。该字段 SHALL 仅持有用户自定义覆盖（diff），key 为 `keyboard-shortcuts` capability 的 `ShortcutSpec.id`，value 为 normalized binding 字符串（如 `"mod+shift+b"`）。

字段 SHALL 标注 `#[serde(default)]`：

- `default` SHALL 让旧版本 config 反序列化时缺失该字段不报错（兼容性）
- 字段 SHALL NOT 加 `skip_serializing_if`——empty HashMap SHALL 序列化为 `{}` 出现在 `get_config` IPC 响应与 `claude-devtools-config.json` 写入两处。理由：单一 serde 序列化路径不能同时实现"IPC 含 empty / 文件不含 empty"；选择"两处都含 empty `{}`"让前端 / 文件 reader 都不需要 undefined fallback，少 5 字节文件体积是可接受成本。

`get_config` IPC 响应 SHALL 包含 `keyboardShortcuts` 字段；`set_config` IPC 接受 `section="keyboardShortcuts"` + `data` 为 `Record<string, string>` 的整体覆盖更新（同 `notifications` 整体替换 `triggers` 数组的模式）。前端 `ui/src/lib/api.ts` 的 `AppConfig` 类型 SHALL 同步增加 `keyboardShortcuts: Record<string, string>` 字段。

#### Scenario: 字段反序列化兼容旧 config
- **WHEN** 读取一个旧版本写入的 `claude-devtools-config.json`（无 `keyboardShortcuts` 字段）
- **THEN** `Config::keyboard_shortcuts` SHALL 反序列化为 empty HashMap，不报错
- **AND** 启动 SHALL 走 builtin defaults

#### Scenario: 空 HashMap 序列化为 `{}`
- **WHEN** 用户从未改动任何快捷键，`AppConfig::keyboard_shortcuts` 为 empty HashMap
- **AND** 触发 `save()` 写入 config 文件 / 或 `get_config` IPC 响应
- **THEN** 写出的 JSON SHALL 含 `"keyboardShortcuts": {}`（不省略键）

#### Scenario: 持久化用户覆盖
- **WHEN** 用户通过 Settings 改动 `sidebar.toggle` 为 `mod+shift+b`
- **AND** 点击 Save 触发 `set_config`
- **THEN** `Config::keyboard_shortcuts` SHALL 更新为 `{"sidebar.toggle": "mod+shift+b"}`
- **AND** 写入后的 JSON SHALL 包含 `"keyboardShortcuts": {"sidebar.toggle": "mod+shift+b"}`

#### Scenario: get_config IPC 响应字段 camelCase
- **WHEN** 前端调用 `invoke("get_config")`
- **THEN** 响应 JSON SHALL 含 `keyboardShortcuts` 键（camelCase，非 `keyboard_shortcuts`）
- **AND** 前端 `AppConfig` TypeScript 类型 SHALL 含 `keyboardShortcuts: Record<string, string>`

#### Scenario: set_config 接收 camelCase 字段
- **WHEN** 前端调用 `invoke("set_config", { keyboardShortcuts: {"sidebar.toggle": "mod+shift+b"} })`
- **THEN** Rust 端 `Config::keyboard_shortcuts` SHALL 反序列化为 `{"sidebar.toggle": "mod+shift+b"}`
- **AND** `save()` SHALL 持久化新值
