# tab-management

## ADDED Requirements

### Requirement: Jobs tab type with singleton semantics

系统 SHALL 支持 `"jobs"` 类型的 tab，具有单例语义（最多一个 Jobs tab 存在）。

#### Scenario: Open Jobs tab when none exists

- **WHEN** 用户点击 TitleBar jobs icon 或 ⌘K "Open Jobs"
- **AND** 当前无 Jobs tab
- **THEN** 创建并激活一个新的 Jobs tab

#### Scenario: Open Jobs tab when one already exists

- **WHEN** 用户点击 TitleBar jobs icon
- **AND** 已有一个 Jobs tab
- **THEN** 激活已有的 Jobs tab（不创建新的）
