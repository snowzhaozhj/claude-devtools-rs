# file-watching

## MODIFIED Requirements

### Requirement: Watch directories for changes

FileWatcher SHALL 监听 `projects/`、`todos/` 和 `jobs/` 三个目录的文件变更。每个目录在 `start()` 时以 `is_dir()` 判断是否存在——不存在则跳过（不 panic、不建目录）。

#### Scenario: Jobs directory exists at startup

- **WHEN** `~/.claude/jobs/` 在 `start()` 时存在
- **THEN** FileWatcher SHALL `watcher.watch(&jobs_dir, Recursive)`

#### Scenario: Jobs directory does not exist at startup

- **WHEN** `~/.claude/jobs/` 在 `start()` 时不存在
- **THEN** FileWatcher SHALL 跳过 watch（不 panic、不建目录）

## ADDED Requirements

### Requirement: Route jobs events filtering only state.json

FileWatcher SHALL 对 `jobs/` 目录下的事件严格过滤：只认 `<job_id>/state.json`（strip prefix 后 components.len() == 2 且 file_name == "state.json"），其它路径（timeline.jsonl / pins.json / tmp/ / recap.trigger）全忽略。

#### Scenario: state.json change emits JobChangeEvent

- **WHEN** `~/.claude/jobs/<job_id>/state.json` 被修改
- **THEN** FileWatcher SHALL 通过 `jobs_tx` 发送 `JobChangeEvent { job_id }`

#### Scenario: timeline.jsonl change is ignored

- **WHEN** `~/.claude/jobs/<job_id>/timeline.jsonl` 被修改
- **THEN** FileWatcher SHALL 不发送任何事件

#### Scenario: Nested path beyond 2 components is ignored

- **WHEN** `~/.claude/jobs/<job_id>/tmp/foo.json` 被修改
- **THEN** FileWatcher SHALL 不发送任何事件

### Requirement: Expose jobs broadcast subscription

FileWatcher SHALL 提供 `subscribe_jobs() -> broadcast::Receiver<JobChangeEvent>` 方法。

#### Scenario: Subscriber receives job change

- **WHEN** state.json 变更触发 JobChangeEvent
- **AND** 有 subscriber 监听
- **THEN** subscriber SHALL 收到该事件
