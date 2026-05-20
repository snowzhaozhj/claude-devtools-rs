## ADDED Requirements

### Requirement: Migrate composite project IDs in pinned sessions on load

`ConfigManager::load` SHALL 在反序列化配置后、暴露给消费方之前，扫描 `SessionsConfig.pinned_sessions: HashMap<String, Vec<PinnedSession>>` 中所有 key（project_id），把含 `"::"` 分隔的 composite id（形如 `{baseDir}::{hash8}`）fold 为 base_dir（即 `"::"` 之前的部分）。fold 时若多个 composite key 共享同一 base_dir，SHALL 把它们的 `Vec<PinnedSession>` 合并并按 `(session_id, pinned_at)` 去重，**保留 `pinned_at` 最早**的条目（即用户最早 pin 的时间戳）。

迁移触发（即检测到至少一个 composite key）SHALL 在写回配置文件前把当前文件备份到 `<config-path>.pre-merge-composite.bak`（覆盖已存在的同名备份），再 atomic-write 新内容。备份命名与现有"损坏配置自动备份到 `.bak.<timestamp_ms>`"机制独立，便于人工识别本次迁移的回滚点。

迁移 SHALL 是幂等的——纯粹基于 input 重写，不依赖任何"已迁移"标志位。写盘失败时 SHALL 通过 `tracing::warn!` 记录，**不**阻塞启动；下次启动 `ConfigManager::load` 命中同样的 composite key 时再次尝试 fold + 写盘。

`HiddenSession` 等其它 `HashMap<String, _>` key 为 project_id 的配置字段 SHALL 同样应用本迁移规则。`NotificationTrigger.repository_ids` 存的是 `RepositoryGroup.id`（git-common-dir 绝对路径，详见 `project-discovery` spec `Group projects by git repository identity` Requirement），与 composite project id 形态完全不同，SHALL NOT 被本迁移触及。

#### Scenario: pinned_sessions 含 composite key 被 fold 为 base_dir

- **WHEN** 配置文件 `pinned_sessions` 含 `"-Users-foo-repo::abcd1234": [{ sessionId: "s1", pinnedAt: 1000 }]` 与 `"-Users-foo-repo::ef567890": [{ sessionId: "s2", pinnedAt: 2000 }]`
- **AND** `ConfigManager::load` 被调用
- **THEN** load 完成后内存中的 `pinned_sessions` SHALL 含 `"-Users-foo-repo": [{ sessionId: "s1", pinnedAt: 1000 }, { sessionId: "s2", pinnedAt: 2000 }]`（顺序不要求，按 `session_id` 字典序或 mtime 倒序均可）
- **AND** SHALL NOT 残留 `"-Users-foo-repo::abcd1234"` 或 `"-Users-foo-repo::ef567890"` key

#### Scenario: 同 session_id 重复条目去重保留 pinned_at 最早

- **WHEN** 配置文件含 `"D::h1": [{ sessionId: "s", pinnedAt: 200 }]` 与 `"D::h2": [{ sessionId: "s", pinnedAt: 100 }]`
- **AND** `ConfigManager::load` 被调用
- **THEN** load 完成后 `pinned_sessions["D"]` SHALL 含**且仅含**一条 `{ sessionId: "s", pinnedAt: 100 }`

#### Scenario: 触发迁移时备份原文件

- **WHEN** 配置文件 `<path>` 含至少一条 composite key 且 fold 后内容与原内容不同
- **AND** `ConfigManager::load` 被调用
- **THEN** 系统 SHALL 在写回前把原文件内容写入 `<path>.pre-merge-composite.bak`
- **AND** 备份写盘 SHALL 在主文件 atomic-write 之前完成

#### Scenario: 未含 composite key 不写盘

- **WHEN** 配置文件 `pinned_sessions` 所有 key 均不含 `"::"`
- **AND** `ConfigManager::load` 被调用
- **THEN** 系统 SHALL NOT 写回主配置文件、SHALL NOT 创建 `.pre-merge-composite.bak`

#### Scenario: 写盘失败不阻塞启动

- **WHEN** fold 检测到 composite key 需要写回
- **AND** atomic-write 失败（磁盘满 / 权限拒绝）
- **THEN** 系统 SHALL 通过 `tracing::warn!` 记录失败原因
- **AND** 内存中的 fold 后状态仍 SHALL 暴露给消费方（避免运行时仍持有 composite key）
- **AND** `ConfigManager::load` SHALL 正常返回（不返回 Err）

#### Scenario: 迁移是幂等的

- **WHEN** 已 fold 的配置文件（不含 composite key）再次被 `ConfigManager::load` 加载
- **THEN** 系统 SHALL NOT 触发任何写盘
- **AND** 内存中的 `pinned_sessions` SHALL 与配置文件内容字节一致

#### Scenario: NotificationTrigger repository_ids 不受迁移影响

- **WHEN** 配置文件含 `NotificationTrigger { repository_ids: Some(vec!["/Users/foo/repo/.git"]), ... }`
- **AND** `pinned_sessions` 同时含 composite key
- **AND** `ConfigManager::load` 被调用
- **THEN** load 完成后该 trigger 的 `repository_ids` SHALL 保持 `["/Users/foo/repo/.git"]` 字节不变（无论是否含 `"::"`）
