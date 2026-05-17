## ADDED Requirements

### Requirement: Display config exposes time format preference

系统 SHALL 在 `display` 配置段暴露 `timeFormat` 字段，类型为枚举 `"24h" | "12h"`，控制 UI 渲染绝对时间戳时是否使用 12 小时制（带 AM/PM 前缀）。**默认值 SHALL 为 `"24h"`**。该字段缺失（旧配置文件兼容）SHALL 反序列化为默认值 `"24h"`。任何非 `"24h"` / `"12h"` 的字符串 SHALL 被 `update_display` 拒绝并返回 validation error，已存储值保持不变。

#### Scenario: 默认配置物化包含 timeFormat 字段

- **WHEN** 系统首次启动且 `~/.claude/devtools-config.json` 不存在
- **THEN** 物化的默认配置 SHALL 包含 `display.timeFormat = "24h"`

#### Scenario: 旧配置文件缺字段时落默认值

- **WHEN** 已有配置文件解析成功但 `display` 段缺少 `timeFormat` 字段
- **THEN** `getConfig` 返回的 `display.timeFormat` SHALL 为 `"24h"`，且后续 `updateConfig` 写入新值后字段 SHALL 持久化到磁盘

#### Scenario: 合法值切换到 12 小时制

- **WHEN** 调用方通过 `update_display` 把 `display.timeFormat` 设为 `"12h"`
- **THEN** 调用 SHALL 成功，下一次 `getConfig` 返回的 `display.timeFormat` 为 `"12h"`

#### Scenario: 合法值切换回 24 小时制

- **WHEN** 调用方把已设为 `"12h"` 的 `display.timeFormat` 重新设为 `"24h"`
- **THEN** 调用 SHALL 成功，下一次 `getConfig` 返回的 `display.timeFormat` 为 `"24h"`

#### Scenario: 非法字符串被拒绝且已存储值不变

- **WHEN** 调用方把 `display.timeFormat` 设为 `"bogus"` 或空字符串
- **THEN** 整次 `update_display` 调用 SHALL 被拒绝并返回 validation error；错误信息 SHALL 包含字段名 `timeFormat`；磁盘上已存储的 `display.timeFormat` 值 SHALL 保持不变

#### Scenario: 重置回默认时 timeFormat 回到 24h

- **WHEN** 调用方调用 reset-to-defaults 入口（如有）或物化全新默认配置
- **THEN** `display.timeFormat` SHALL 为 `"24h"`

### Requirement: IPC contract exposes timeFormat in camelCase

`getConfig` 与 `updateConfig` 的 IPC 响应 SHALL 在 `display` 段以 camelCase 字段名 `timeFormat` 暴露时间格式偏好，值为字符串 `"24h"` 或 `"12h"`。

#### Scenario: getConfig 响应包含 timeFormat 字段

- **WHEN** 前端调用 `getConfig`
- **THEN** 响应 `display` 段 SHALL 包含 `timeFormat` 键，值为 `"24h"` 或 `"12h"` 之一

#### Scenario: updateConfig 接受 camelCase timeFormat patch

- **WHEN** 前端调用 `updateConfig({ display: { timeFormat: "12h" } })`
- **THEN** 调用 SHALL 成功，下一次 `getConfig` 返回的 `display.timeFormat` 为 `"12h"`
