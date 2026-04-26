## ADDED Requirements

### Requirement: Display config exposes user-customizable font family

系统 SHALL 在 `display` 配置段暴露 `fontSans` 与 `fontMono` 两个可选字段，允许用户覆盖 UI 默认 sans / mono 字体。`null` 或缺失字段表示使用应用内置默认字体栈，非空字符串表示用户提供的 CSS `font-family` 值。空白字符串 SHALL 在持久化前归一化为 `null`。

#### Scenario: Font fields default to null on first launch
- **WHEN** 首次启动且配置文件不存在
- **THEN** 物化的默认配置 SHALL 包含 `display.fontSans = null` 与 `display.fontMono = null`

#### Scenario: Old config file without font fields is forward-compatible
- **WHEN** 已有配置文件解析成功但 `display` 段缺少 `fontSans` / `fontMono` 字段
- **THEN** 系统 SHALL 把缺失字段视为 `null`，已有配置值保留不变，无需迁移

#### Scenario: User sets a custom sans font
- **WHEN** 调用方通过 `update_field` 把 `display.fontSans` 设为 `"\"JetBrains Mono\", monospace"`
- **THEN** 该值 SHALL 被持久化，下次读取时返回相同字符串

#### Scenario: Whitespace-only value normalizes to null
- **WHEN** 调用方把 `display.fontSans` 设为 `"   "`（仅空白字符）
- **THEN** 系统 SHALL 把该字段持久化为 `null`，不保留空白字符串

#### Scenario: Restore default by setting null
- **WHEN** 调用方把已设过的 `display.fontMono` 重新设为 `null`
- **THEN** 该字段 SHALL 被持久化为 `null`，前端 SHALL 回落到应用内置默认字体栈

#### Scenario: Excessively long value rejected
- **WHEN** 调用方把 `display.fontSans` 设为长度超过 500 字符的字符串
- **THEN** 更新 SHALL 被拒绝并返回 validation error，已存储值 SHALL 保持不变

#### Scenario: Atomic display patch rejects entire batch on any invalid field
- **WHEN** 调用方一次 `update_display` 同时设置 `fontSans = "<合法值>"` 与 `fontMono = "<超过 500 字符>"`
- **THEN** 整次更新 SHALL 被拒绝并返回 validation error，`display.fontSans` 与 `display.fontMono` 两者已存储值 SHALL 保持不变（不允许半写状态）

#### Scenario: Reset to defaults clears font overrides
- **WHEN** 用户已设过自定义 `fontSans` / `fontMono`，随后触发 `reset_to_defaults`
- **THEN** 重置后 `display.fontSans` 与 `display.fontMono` SHALL 都为 `null`，前端 SHALL 回落到应用内置默认字体栈

### Requirement: IPC contract exposes font fields in camelCase

`getConfig` 与 `updateConfig` 的 IPC 响应 SHALL 在 `display` 段以 camelCase 字段名 `fontSans` 与 `fontMono` 暴露字体配置，类型为 `string | null`。

#### Scenario: getConfig response shape
- **WHEN** 前端调用 `getConfig`
- **THEN** 响应 `display` 段 SHALL 同时包含 `fontSans` 与 `fontMono` 两个键，值为字符串或 `null`

#### Scenario: updateConfig accepts null to clear
- **WHEN** 前端调用 `updateConfig({ display: { fontSans: null } })`
- **THEN** 调用 SHALL 成功，下一次 `getConfig` 返回的 `display.fontSans` 为 `null`

#### Scenario: updateConfig accepts non-empty string
- **WHEN** 前端调用 `updateConfig({ display: { fontMono: "\"Fira Code\", monospace" })`
- **THEN** 调用 SHALL 成功，下一次 `getConfig` 返回该字符串
