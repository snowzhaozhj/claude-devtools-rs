## MODIFIED Requirements

### Requirement: `FsError` 提供错误语义元方法

`FsError` enum SHALL 至少含以下 variants：`NotFound(PathBuf)` / `Io { path, source: io::Error }` / `Utf8 { path, source }` / `Unsupported(&'static str)` / `Disconnected { path, reason }` / `TransientExhausted { path, attempts, last_reason }`。每个 variant SHALL 提供以下 inherent 元方法：

- `fn is_retryable(&self) -> bool` —— 返回 `true` 表示这个错误是瞬时的（caller 重试一次有意义），`false` 表示永久（不要重试）
- `fn should_invalidate_cache(&self) -> bool` —— 返回 `true` 表示 cache entry 对应这个 path 应该被清掉（文件可能不存在或损坏），`false` 表示 cache 保留（仅是临时网络抖动等）
- `fn is_likely_channel_dead(&self) -> bool` —— 返回 `true` 表示底层 transport channel（典型 SSH SFTP）很可能已死 / 半死，caller（典型 `cdt-discover::ProjectScanner` SSH 分支）SHALL 据此 fail-fast 而非 silent continue 凑半成品列表。语义独立于 `is_retryable`：channel-dead 是更强的"该不该 abort 整轮 scan"信号，与"该不该再 retry 一次"不同（`Disconnected.is_retryable() == true` 但 `is_likely_channel_dead() == true` 同时成立——表达"重连后能 retry，但当前 scan 已不该继续"）

`is_likely_channel_dead` 命中规则：

- `Disconnected { .. }`：恒 true（已经是显式 channel 断开信号）
- `TransientExhausted { last_reason }`：`last_reason.to_ascii_lowercase()` 含 `session closed` / `eof` / `broken pipe` / `epipe` / `connection reset` / `econnreset` 任一关键字时 true（`with_retry` 3 次后仍是 transport-dead 即视同 channel 真死）；纯 `timeout` / `eagain` 等不含 transport-dead 关键字的 transient exhausted 返 false（保留"远端短暂不可达"的容错语义）
- `Io { source }`：`source.kind()` 是 `BrokenPipe` / `ConnectionReset` / `ConnectionAborted` 时 true
- `NotFound` / `Utf8` / `Unsupported`：恒 false（与 channel 状态无关）

#### Scenario: NotFound 不重试，清 cache

- **WHEN** `fs.stat(path)` 返回 `FsError::NotFound(path)`
- **THEN** `err.is_retryable()` SHALL 返回 `false`
- **AND** `err.should_invalidate_cache()` SHALL 返回 `true`（文件不存在，cache 任何 entry 都应该清）

#### Scenario: Disconnected 重试，不清 cache

- **WHEN** SSH 连接突然断开，操作返回 `FsError::Disconnected { ... }`
- **THEN** `err.is_retryable()` SHALL 返回 `true`（重连后可能恢复）
- **AND** `err.should_invalidate_cache()` SHALL 返回 `false`（数据仍可能有效，只是当前连不上）

#### Scenario: TransientExhausted 不重试，不清 cache

- **WHEN** SSH 操作 with_retry 耗尽 3 次仍失败，返回 `FsError::TransientExhausted { attempts: 3, ... }`
- **THEN** `err.is_retryable()` SHALL 返回 `false`（已经重试过了，再试也无意义）
- **AND** `err.should_invalidate_cache()` SHALL 返回 `false`（数据仍可能有效，远端可能恢复）

#### Scenario: Disconnected 触发 channel-dead

- **WHEN** 操作返回 `FsError::Disconnected { path, reason }`
- **THEN** `err.is_likely_channel_dead()` SHALL 返回 `true`（无论 reason 内容）

#### Scenario: TransientExhausted 含 transport-dead 关键字触发 channel-dead

- **WHEN** 操作返回 `FsError::TransientExhausted { last_reason: "broken pipe", attempts: 3, .. }` 或 `FsError::TransientExhausted { last_reason: "session closed", .. }` 或类似含 `eof` / `epipe` / `connection reset` / `econnreset` 关键字的 reason
- **THEN** `err.is_likely_channel_dead()` SHALL 返回 `true`
- **AND** caller 若是 `ProjectScanner` SSH 分支 SHALL 据此 abort 整轮 scan（详 `project-discovery::Scan Claude projects directory` Requirement 中的 SSH 模式分流规则）

#### Scenario: TransientExhausted 仅含纯 timeout 不触发 channel-dead

- **WHEN** 操作返回 `FsError::TransientExhausted { last_reason: "timeout", .. }` 或 `last_reason: "etimedout"` 或 `last_reason: "eagain"` 等不含 transport-dead 关键字的 reason
- **THEN** `err.is_likely_channel_dead()` SHALL 返回 `false`（保留"远端短暂不可达"的容错语义，留给 polling watcher 的独立 timeout counter 在持续 18s 后自行触发 dead_signal，与 scanner 一次性 abort 解耦）

#### Scenario: Io BrokenPipe / ConnectionReset / ConnectionAborted 触发 channel-dead

- **WHEN** 操作返回 `FsError::Io { source }`，`source.kind()` 是 `BrokenPipe` / `ConnectionReset` / `ConnectionAborted` 任一
- **THEN** `err.is_likely_channel_dead()` SHALL 返回 `true`

#### Scenario: NotFound / Utf8 / Unsupported 不触发 channel-dead

- **WHEN** 操作返回 `FsError::NotFound(_)` 或 `FsError::Utf8 { .. }` 或 `FsError::Unsupported(_)`
- **THEN** `err.is_likely_channel_dead()` SHALL 返回 `false`（这些错误与底层 transport 状态无关）
