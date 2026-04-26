## ADDED Requirements

### Requirement: 持久化「启动时自动检查更新」开关

`ConfigData` SHALL 包含字段 `auto_update_check_enabled: bool`，序列化为 JSON 字段名 `autoUpdateCheckEnabled`，缺省值通过 `#[serde(default = "<fn>")]` 物化为 `true`。该字段 SHALL 控制应用启动后 5 秒后台自动检查更新行为，但 MUST NOT 影响手动「检查更新」按钮的可用性。

#### Scenario: 默认值为启用

- **WHEN** 首次启动或老配置文件中无该字段
- **THEN** `ConfigData::auto_update_check_enabled` SHALL 反序列化为 `true`
- **AND** 后端启动 5 秒后台检查 SHALL 正常执行

#### Scenario: 关闭开关并持久化

- **WHEN** 调用方通过 `update_config` IPC 把 `autoUpdateCheckEnabled` 设为 `false`
- **THEN** `ConfigManager` SHALL 把 `config.auto_update_check_enabled = false` 持久化到磁盘
- **AND** 下次启动 SHALL 跳过后台自动检查

#### Scenario: 开启开关并持久化

- **WHEN** 调用方把 `autoUpdateCheckEnabled` 设为 `true`
- **THEN** `ConfigManager` SHALL 持久化为 `true`
- **AND** 下次启动 SHALL 恢复后台自动检查

#### Scenario: 与既有字段合并

- **WHEN** 老配置文件含其它字段但无 `autoUpdateCheckEnabled`
- **THEN** 加载逻辑 SHALL 与默认配置合并，该字段取默认值 `true`，其它已有字段 SHALL NOT 被覆盖

### Requirement: 持久化跳过的更新版本号

`ConfigData` SHALL 包含字段 `skipped_update_version: Option<String>`，序列化为 JSON 字段名 `skippedUpdateVersion`，遵循既有 schema 演进约定（`#[serde(default, skip_serializing_if = "Option::is_none")]`），用于记录用户主动「跳过此版本」的目标版本号。

#### Scenario: 默认值为空

- **WHEN** 首次启动或老配置文件中无该字段
- **THEN** `ConfigData::skipped_update_version` SHALL 反序列化为 `None`
- **AND** 持久化时 `skippedUpdateVersion` 字段 SHALL NOT 出现在 JSON 中（`skip_serializing_if`）

#### Scenario: 写入跳过版本

- **WHEN** 调用方通过 `update_config` IPC 传 `{ section: "skippedUpdateVersion", data: "0.3.0" }` 或在 patch 中包含 `skippedUpdateVersion: "0.3.0"`
- **THEN** `ConfigManager` SHALL 把 `config.skipped_update_version = Some("0.3.0".into())` 持久化到磁盘
- **AND** 下次读取 SHALL 返回该值

#### Scenario: 清空跳过版本

- **WHEN** 调用方传 `skippedUpdateVersion: null`
- **THEN** `ConfigManager` SHALL 把 `config.skipped_update_version = None` 持久化
- **AND** 下次读取 SHALL 返回 `None`

#### Scenario: 与既有字段合并

- **WHEN** 老配置文件含 triggers / pinned 等字段但无 `skippedUpdateVersion`
- **THEN** 加载逻辑 SHALL 与默认配置合并，保留已有字段，`skippedUpdateVersion` 取默认 `None`，**不**覆盖其他字段
