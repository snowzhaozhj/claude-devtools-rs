## MODIFIED Requirements

### Requirement: --since/--until 区间交集过滤

`--since` 和 `--until` 参数 SHALL 用区间交集语义过滤 session：session 的时间区间为 `[created, timestamp]`（即 `[birthtime, mtime]`），查询区间为 `[since, until]`。两个区间有交集时该 session SHALL 包含在结果中。

交集条件：`session.created <= until AND session.timestamp >= since`。

此语义同时适用于 CLI `cdt sessions list` 和 MCP `list_sessions`——两者共用 `QueryFilter`。

#### Scenario: 跨午夜会话被 --since/--until 正确包含

- **GIVEN** session A 的 created = 2026-06-07T15:00+08:00，mtime = 2026-06-08T02:30+08:00（跨午夜）
- **WHEN** 用户运行 `cdt sessions list --since 2026-06-07 --until 2026-06-08`
- **THEN** session A SHALL 出现在结果中（created < until 解析值 2026-06-08T00:00+08:00）

#### Scenario: 完全在范围之外的会话被排除

- **GIVEN** session B 的 created = 2026-06-08T01:00+08:00，mtime = 2026-06-08T03:00+08:00
- **WHEN** 用户运行 `cdt sessions list --since 2026-06-07 --until 2026-06-08`
- **THEN** session B SHALL NOT 出现在结果中（created > until）

#### Scenario: --since 单用仍匹配活跃会话

- **GIVEN** session C 的 created = 2026-06-05，mtime = 2026-06-07
- **WHEN** 用户运行 `cdt sessions list --since 2026-06-06`
- **THEN** session C SHALL 出现在结果中（mtime >= since）
