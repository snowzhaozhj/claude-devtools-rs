# ipc-data-api

## ADDED Requirements

### Requirement: list_jobs IPC command

系统 SHALL 提供 `list_jobs` IPC command，返回所有可解析的后台任务列表（已按分组排序）。

#### Scenario: list_jobs returns grouped jobs

- **WHEN** 前端调用 `list_jobs`
- **THEN** 返回 `{ jobs: JobSummary[], jobsDirExists: boolean }`
- **AND** jobs 按 Ready for review > Needs input > Working > Completed 顺序排列

#### Scenario: list_jobs with no jobs directory

- **WHEN** `~/.claude/jobs/` 不存在
- **THEN** 返回 `{ jobs: [], jobsDirExists: false }`

#### Scenario: list_jobs tolerates parse failures

- **WHEN** 某些 state.json 解析失败
- **THEN** 跳过失败项，返回成功解析的 jobs
