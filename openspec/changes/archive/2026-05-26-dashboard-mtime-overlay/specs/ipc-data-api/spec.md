## ADDED Requirements

### Requirement: ProjectScanCache 维护 per-project mtime overlay 让 cache 命中路径返回新鲜 mtime

系统 SHALL 在 project scan cache 主体之外维护每个 context、每个 project 的最大已观测 session mtime hint（"overlay"）。带 mtime 的非删除 file-change event SHALL 单调推进事件所属 context 对应 project 的 hint；不带 mtime 或删除事件 SHALL NOT 推进该 hint。

`list_projects` 与 `list_repository_groups` 返回数据时，系统 SHALL 使用 cache snapshot 中的 `most_recent_session` 与该 hint 的较大值作为对外返回的 `most_recent_session` 字段；`RepositoryGroup` 维度下的聚合 mtime SHALL 从合成后的 worktree 值再次取最大。本路径在 cache 命中与未命中两种情形下对外契约一致——调用方不感知合成发生在何处。

cache 重新扫描完成后，系统 SHALL 按以下规则合并 hint 与 fresh snapshot：

- fresh snapshot 已反映或超过 hint（snapshot value ≥ hint）→ SHALL 丢弃该 hint
- hint 仍大于 fresh snapshot（snapshot value < hint）→ SHALL 保留该 hint，避免 scan 期间发生的 append 被回退

cache 主体 invalidate 与 hint 的解耦规则：

- 三档 invalidate（`projectListChanged || deleted || unknown_session`，详 `ProjectScanCache 按事件语义分级失效` Requirement）触发的清空 SHALL **不**清 hint——hint 是 watcher 单调观测的中间结果，丢失无法重建；snapshot 是 fs 真相的快照，可重新 scan
- 显式 context 切换路径（`ssh_disconnect` / `reconfigure_claude_root` / 公开方法 `invalidate_project_scan_cache()`）SHALL 清空 hint 与 snapshot——上下文已切换，旧 hint 不再适用

context 隔离不变量：Local 与 SSH context 的 hint 互不影响——Local file-change event 仅推进 Local context 下的 hint；SSH polling event 仅推进对应 SSH context 下的 hint。本不变量与 spec `ProjectScanCache 按事件语义分级失效` 中"SSH event 跳过 local cache hint"的守护保持一致：跨 context 守护仅作用于"SSH event 是否参与 Local cache 三档 invalidate / cache hint OR"（不参与），SSH event 自身的 mtime hint 仍 SHALL 写入对应 SSH context overlay 让 SSH 用户的 dashboard 同样受益。

跨 context 同名 project 的边界（accepted limitation）：file-change event 不携带 source ContextId，invalidator 通过既有 `is_local_project` 字符串级守护推断 event 来源——SSH 远端与 local 同名 project 共存时旧 SSH event 进入队列后可能误推进 local hint。本 Requirement 接受该 edge case；根治留 followup（需 watcher 注入 ContextId 字段做精确 dispatch）。

#### Scenario: 已知 session 普通 append 推进 hint 但不 invalidate

- **WHEN** 本地 watcher 发出 file-change event：project `pa`、session `sa`、`deleted=false`、`projectListChanged=false`、`sessionListChanged=false`、`mtimeMs=Some(t1)`，且 cache 已含 Local context 的 entry，且 Local context 下 `pa` 的 hint 当前为 `t0 < t1`
- **THEN** 系统 SHALL 把 Local context 下 `pa` 的 hint 单调推进到 `t1`
- **AND** SHALL NOT 触发 cache 主体 invalidate
- **AND** 紧接着的 `list_repository_groups` cache hit 返回的 `RepositoryGroup.most_recent_session` SHALL ≥ `t1`
- **AND** SHALL NOT 增加 `project_scan_cache.invalidate.structural` counter

#### Scenario: 删除事件不推进 hint

- **WHEN** 本地 watcher 发出 file-change event：project `pa`、session `sa`、`deleted=true`、`mtimeMs` 缺省
- **THEN** 系统 SHALL NOT 改写任何 context 下 `pa` 的 hint
- **AND** 仍按既有规则触发 cache 主体 invalidate（`deleted` 命中三档第一档）

#### Scenario: cache hit 路径合成 hint 让用户看到最新 mtime

- **WHEN** cache snapshot 中 project `pa` 的 `most_recent_session=Some(t0)`，watcher 多次 append 事件让 Local context 下 `pa` 的 hint 推进到 `t2 > t0`，期间无结构性事件命中
- **AND** 调用方调 `list_repository_groups`
- **THEN** 返回的 `RepositoryGroup.most_recent_session`（聚合自该 project 对应 worktrees）SHALL 等于 `t2`
- **AND** 合成 SHALL 仅修改返回数据的 `most_recent_session` 字段；底层 cache snapshot 主体 SHALL NOT 被改写

#### Scenario: cache 重扫合并保留较大 hint

- **WHEN** Local context 下 `pa` 的 hint 当前值为 `t2`，scan 完成后新 snapshot 中 `pa.most_recent_session=Some(t1)` 且 `t1 < t2`
- **THEN** 重扫结果被接受并作为 fresh snapshot 生效时 SHALL 保留 Local context 下 `pa` 的 hint 为 `t2`
- **AND** 后续 `list_repository_groups` cache hit SHALL 仍返回 `t2`（合成路径继续生效）

#### Scenario: cache 重扫清除已被覆盖的旧 hint

- **WHEN** Local context 下 `pa` 的 hint 当前值为 `t1`，scan 完成后新 snapshot 中 `pa.most_recent_session=Some(t2)` 且 `t2 ≥ t1`
- **THEN** 重扫结果被接受并作为 fresh snapshot 生效时 SHALL 移除 Local context 下 `pa` 的 hint 条目（snapshot 已反映或超过该 mtime）
- **AND** 后续 `list_repository_groups` cache hit SHALL 直接返回 snapshot 内 `t2`，不依赖 hint

#### Scenario: 三档 invalidate 不清 hint

- **WHEN** Local context 下 `pa` 的 hint 含值 `t2`，watcher 收到一条结构性事件触发三档 invalidate
- **THEN** cache snapshot 中 Local entry SHALL 被清空
- **AND** Local context 下 `pa` 的 hint SHALL 保留为 `t2`
- **AND** 下一次 `list_repository_groups` cache miss 重扫后 SHALL 按合并规则处理 hint

#### Scenario: 显式 invalidate 总清同时清 hint

- **WHEN** 调用方调 `invalidate_project_scan_cache()` 公开方法（典型：IPC contract 测试 / SSH context 显式切换前 hook）
- **THEN** cache snapshot SHALL 全部清空（覆盖所有 backend kind）
- **AND** mtime hint SHALL 全部清空（覆盖所有 context）

#### Scenario: SSH event 推进对应 SSH context hint 但不影响 Local invalidate

- **WHEN** SSH polling watcher 发出 file-change event：project `pa`、`mtimeMs=Some(t1)`、`deleted=false`，且当前 active SSH context 已注册
- **THEN** 系统 SHALL 把 SSH context 下 `pa` 的 hint 单调推进到至少 `t1`
- **AND** SHALL NOT 推进 Local context 下 `pa` 的 hint
- **AND** SHALL NOT 因该 SSH event 触发 Local `ProjectScanCache` 三档 invalidate 或 Local cache hint OR

#### Scenario: 缺 mtimeMs 字段的 file-change event 不推进 hint

- **WHEN** 本地 watcher 因运行环境无法取到 mtime 发出 file-change event：`mtimeMs` 缺省
- **THEN** 系统 SHALL NOT 改写任何 context 下任何 project 的 hint
- **AND** 仍按 `mtimeMs` 之外字段（`projectListChanged` / `deleted` / `unknown_session` 判定）走既有三档失效逻辑

#### Scenario: cache 空时收到 mtime hint 仍写 hint

- **WHEN** cache snapshot 为空（冷启 / `reconfigure_claude_root` 后），Local context 下 `pa` 的 hint 也为空
- **AND** 本地 watcher 发出 `mtimeMs=Some(t1)` 的 event
- **THEN** 系统 SHALL 把 Local context 下 `pa` 的 hint 设为 `t1`（即便此时无 entry，hint 提前到位以便后续 scan 完成 populate 时合并阶段保留）
- **AND** 后续 `list_repository_groups` cache miss → 全扫 → 合并阶段按规则处理 hint

#### Scenario: cache 重扫不再含某 project 时清掉对应 hint

- **WHEN** Local context 下含 hint 条目 `pa→t2` 与 `pb→t3`，重扫后 fresh snapshot 不再含 project `pa`（用户已删除该 encoded 目录）
- **THEN** 重扫合并阶段 SHALL 移除 Local context 下 `pa` 的 hint 条目
- **AND** SHALL 保持 `pb` 的 hint 按合并规则处理（`pb` 仍存在）
- **AND** 同 context 下 hint 条目数 SHALL bounded by fresh snapshot 中 live project 数（避免已删除 project 的 hint 永久驻留）
