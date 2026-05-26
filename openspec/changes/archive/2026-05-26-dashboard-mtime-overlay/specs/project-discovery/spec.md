## ADDED Requirements

### Requirement: Cached snapshot SHALL 反映已知 session 普通 append 推进的 most_recent_session

`Project.most_recent_session` 字段对外承诺反映该 project 下所有 jsonl session 的最新 mtime（毫秒 since UNIX epoch）。当上层经 cache 命中路径返回 `Project` / `RepositoryGroup` 时（典型：`list_projects` / `list_repository_groups` 返回的 `RepositoryGroup.most_recent_session`），系统 SHALL 在用户感知时长内（一次正常 file-change 事件投递时延加合成开销，详 `[[file-watching]]::事件投递时延、远端 polling 频率与停止时延`）让该字段反映自上次 cache 写入以来 watcher 观测到的最新 jsonl mtime。

不变量：

- 已知 session 普通 append（不改变 sessions 集合 / cwd / topology）SHALL NOT 触发 `ProjectScanner::scan()` 重扫——仅推进 `Project.most_recent_session` 显示值
- 已知 session 普通 append SHALL NOT 改变 `Project.sessions` / `Project.distinct_cwds` / `Project.path` / `Project.created_at` 等其它字段
- 用户在 dashboard 项目卡片上看到的"最近活动"时间 SHALL 与 sidebar 当前打开会话的 modified 时间在同一文件追加事件后保持视觉一致（差异 < 一次 debounce 窗口 + 一次合成开销）
- 按 `most_recent_session` 倒序的项目排序 SHALL 反映最新的 mtime——同一组数据下 dashboard 卡片排序与 sidebar 切项目时的 group 排序应一致

SSH context 下，上述用户感知时长以 `[[file-watching]]::事件投递时延、远端 polling 频率与停止时延` 定义的远端 polling 节拍为上界（默认 3 秒，catch-up 30 秒）；两次 poll 之间发生的 append 允许短暂显示上一轮 mtime——这是 SSH 远端无 OS 通知机制的物理上界，本 capability 接受为 limitation。

实现路径（不进 spec 的具体合成机制）由 `[[ipc-data-api]]::ProjectScanCache 维护 per-project mtime overlay 让 cache 命中路径返回新鲜 mtime` Requirement 单独承担——本 Requirement 仅定义用户视角的契约。

#### Scenario: 已知 session 持续追加后 dashboard 项目卡片的 mostRecentSession 跟随推进

- **WHEN** `list_repository_groups` 在 `t0` 时刻被首次调用、写入 cache，返回的 `RepositoryGroup.most_recent_session` 为 `t0_max`
- **AND** 同 project 下某已知 session jsonl 在 `t1 > t0` 时刻被追加，watcher 投递对应 file-change 事件
- **AND** 调用方在 `t2 > t1`（`t2 - t0 < cache TTL`）时再次调用 `list_repository_groups`
- **THEN** 返回的 `RepositoryGroup.most_recent_session` SHALL ≥ `t1`（反映追加事件的 mtime）
- **AND** SHALL NOT 仍为旧的 `t0_max`

#### Scenario: 已知 session 普通追加不改变 sessions 集合

- **WHEN** project `pa` 的 cached snapshot 含 `sessions = ["sa", "sb"]`，已知 session `sa` 被追加内容
- **THEN** 紧接着的 `list_projects` cache hit 路径返回的 `Project { id: "pa", sessions, ... }` SHALL 仍含且仅含 `["sa", "sb"]`
- **AND** `Project.most_recent_session` SHALL 反映 `sa` 追加后的 mtime
- **AND** `Project.distinct_cwds` 与 `Project.created_at` 字段 SHALL NOT 变化

#### Scenario: dashboard 卡片排序按最新活动倒序

- **WHEN** 两个 project `pa` / `pb` 在 cache 写入时刻分别有 `most_recent_session = t_a < t_b`
- **AND** `pa` 后续被持续追加内容，watcher 推进对应 mtime 至 `t_a' > t_b`
- **AND** 调用方此时调 `list_repository_groups`
- **THEN** 返回数组排序 SHALL 把 `pa` 对应 group 排在 `pb` 之前（反映 `pa` 当前最新 mtime 已超过 `pb`）

#### Scenario: 新 session 首次出现仍走结构性 invalidate 路径

- **WHEN** project `pa` 下首次出现新 session `sc.jsonl`（cache snapshot 不含 `sc`）
- **THEN** 对应 file-change 事件 SHALL 被判定为结构性（unknown_session 命中）
- **AND** `ProjectScanCache` SHALL 走 invalidate + 下次 scan 重新拿到含 `sc` 的 fresh snapshot——**不**通过 mtime overlay 路径"假装"看到 sessions 列表更新

#### Scenario: 删除 session 仍走结构性 invalidate 路径

- **WHEN** project `pa` 下已知 session `sa.jsonl` 被删除
- **THEN** 对应 file-change 事件 `deleted=true` SHALL 命中三档第一档
- **AND** `ProjectScanCache` SHALL invalidate + 重扫拿到不含 `sa` 的 fresh snapshot——**不**依赖 overlay
