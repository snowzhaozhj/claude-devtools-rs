## ADDED Requirements

### Requirement: `FsMetadata` 携带文件创建时间（birthtime）

`FsMetadata` SHALL 额外携带 `created: Option<SystemTime>` 字段。各 provider 实现 SHALL 从底层 `std::fs::Metadata::created()` 获取；返回 `Err` 时（典型：Linux ext2/ext3、部分网络文件系统）SHALL 填 `None`。

caller 需要 epoch 毫秒时 SHALL 调用 `created_ms()` 方法，该方法 SHALL 返回 `min(created, mtime)` 的 epoch 毫秒值（归一化：防止 cp/rsync 等场景下 birthtime > mtime 产生反向区间）。`created = None` 时 fallback 到 `mtime` 值——确保所有平台都能拿到一个有意义的时间戳。

#### Scenario: macOS/Windows 返回真实 birthtime

- **GIVEN** 运行在 macOS 或 Windows 系统上
- **WHEN** stat 一个正常文件
- **THEN** `FsMetadata.created` SHALL 是 `Some(t)`，其中 `t` <= `mtime`

#### Scenario: 不支持 birthtime 的文件系统 fallback

- **GIVEN** 运行在不支持 birthtime 的 Linux 文件系统上
- **WHEN** stat 一个正常文件
- **THEN** `FsMetadata.created` SHALL 是 `None`
- **AND** `created_ms()` SHALL 返回与 `mtime_ms()` 相同的值

#### Scenario: created > mtime 时归一化

- **GIVEN** 文件被 cp/rsync 复制导致 birthtime > mtime
- **WHEN** 调用 `created_ms()`
- **THEN** SHALL 返回 `min(created, mtime)` 的 epoch 毫秒值（不大于 `mtime_ms()`）
