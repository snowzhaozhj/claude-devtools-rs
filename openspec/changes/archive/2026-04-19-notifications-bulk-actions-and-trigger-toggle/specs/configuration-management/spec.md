## ADDED Requirements

### Requirement: Update notifications SHALL accept full triggers replacement

`ConfigManager::update_notifications` SHALL 处理传入 JSON payload 的 `triggers` 字段：反序列化为 `Vec<NotificationTrigger>`、对每条调用 `validate_trigger` 拒绝非法条目、整体替换 `self.config.notifications.triggers`、并调用 `TriggerManager::set_triggers(list)` 同步内存状态，最后 `save()` 持久化。未识别的键 SHALL 仍被忽略但 MUST 通过 `tracing::warn!(key = %k, "unknown notifications update key ignored")` 记录，避免再次静默丢字段。

#### Scenario: triggers 字段被整体替换并落盘

- **WHEN** 调用方向 `update_config` IPC 发送 `section="notifications", data={ "triggers": [<新数组>] }`
- **THEN** `ConfigManager` SHALL 将 `config.notifications.triggers` 替换为该数组、同步 `TriggerManager::triggers`、写入磁盘
- **AND** 下一次调用 `get_enabled_triggers()` SHALL 返回新数组中 `enabled=true` 的子集

#### Scenario: 非法 trigger 拒绝整组写入

- **WHEN** 新 triggers 数组中任意一条经 `validate_trigger` 返回 `valid=false`
- **THEN** `update_notifications` SHALL 返回 `ConfigError::validation` 携带该 trigger id 与失败原因
- **AND** `self.config.notifications.triggers` 与 `TriggerManager::triggers` SHALL 保持修改前状态（不部分写入）
- **AND** 磁盘文件 SHALL 不被更新

#### Scenario: 未知通知键发出 warn 但不报错

- **WHEN** payload 中含除 `enabled / soundEnabled / includeSubagentErrors / snoozeMinutes / triggers` 外的其它键（例如 `fooBar`）
- **THEN** 该键 SHALL 被忽略，操作仍返回成功
- **AND** 日志 SHALL 以 `warn` 级别包含被忽略的键名
