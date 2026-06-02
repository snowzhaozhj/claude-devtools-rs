## ADDED Requirements

### Requirement: `extract_session_cwd` 进程级缓存

系统 SHALL 维护一个进程级 `cwd_cache: LruCache<PathBuf, String>`（容量 2048），缓存 `extract_session_cwd` 成功抽取到 cwd 的结果。`list_sessions` 调用 `extract_session_cwd` 时 SHALL 先查缓存：cache hit 直接返回，跳过 `read_lines_head` I/O；cache miss 时执行现有 head-read 逻辑，成功提取到 cwd（`Some(cwd)`）时写入缓存。

缓存 key 为 session JSONL 文件的完整路径（`PathBuf`），value 为 `String`（成功提取的 cwd 值）。

**仅缓存正结果**：head-read 失败（I/O 错误）、解析失败、或文件不含 cwd 字段（返回 `None`）时 SHALL NOT 写入缓存——确保瞬时故障不被永久固化，下次调用可重试。

缓存 SHALL 由 `LocalDataApi` 持有并通过构造器传给每次创建的 `ProjectScanner`（类似 `shared_read_semaphore` 模式），确保跨 IPC 调用共享。所有生产构造 `ProjectScanner` 的路径（`build_group_session_page`、`list_sessions_skeleton`、`scan_projects_cached` 等）SHALL 传入同一个共享 cwd cache 实例。

缓存使用 LRU 淘汰策略：容量满时淘汰最久未访问条目，确保新活跃 session 始终能入缓存。

缓存仅适用于 `FsKind::Local`；SSH 远端的 `extract_session_cwd` 调用 SHALL NOT 使用此缓存。

#### Scenario: cache hit 跳过文件 I/O

- **WHEN** `extract_session_cwd` 被调用且 `cwd_cache` 中已存在该路径的条目
- **THEN** 系统 SHALL 直接返回缓存值，不执行 `read_lines_head`

#### Scenario: cache miss 成功提取后写入缓存

- **WHEN** `extract_session_cwd` 被调用且 `cwd_cache` 中无对应条目，且 head-read 成功返回 `Some(cwd)`
- **THEN** 系统 SHALL 将 `(path, cwd)` 写入 `cwd_cache`，并返回 `Some(cwd)`

#### Scenario: head-read 失败或 cwd 为空时不缓存

- **WHEN** `extract_session_cwd` 被调用且 head-read 返回 `None`（I/O 错误、解析失败、或文件不含 cwd 字段）
- **THEN** 系统 SHALL 返回 `None`，且 SHALL NOT 将该条目写入 `cwd_cache`

#### Scenario: 缓存容量满时 LRU 淘汰

- **WHEN** `cwd_cache` 已有 2048 条目，且新 session 路径不在缓存中，且 head-read 成功
- **THEN** 系统 SHALL 淘汰最久未访问的条目后写入新条目

#### Scenario: SSH 路径不使用缓存

- **WHEN** `ProjectScanner` 的 `fs.kind() == FsKind::Ssh`
- **THEN** `extract_session_cwd` SHALL NOT 查询或写入 `cwd_cache`，始终执行远端读取
