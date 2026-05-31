# Background Jobs 测试 Fixture

模拟 `~/.claude/jobs/<job_id>/state.json` 的真实磁盘文件。
用于后端 IPC contract test 和端到端真数据验证。

## 文件列表

| 目录 | 状态 | 用途 |
|---|---|---|
| `job-working/` | state=working, tempo=active | 正在运行的普通 job |
| `job-blocked/` | state=blocked | 等待用户输入 |
| `job-idle/` | state=idle | 暂停态 |
| `job-done/` | state=done | 正常完成 |
| `job-failed/` | state=failed | 执行失败 |
| `job-stopped/` | state=stopped | 手动停止 |
| `job-with-pr/` | state=working + children=[pr] | 有 PR 子任务（Ready for review 分组） |
| `job-cross-project/` | state=working, linkScanPath 指向另一项目 | 跨项目 session 跳转 |

## 字段说明

每个 `state.json` 对齐后端 `cdt-core::BackgroundJob` 的 serde camelCase 格式：
- `state`：working / idle / blocked / done / failed / stopped
- `name`：任务名称
- `detail`：当前步骤描述
- `intent`：用户意图
- `children`：[{ kind: "pr", href: "..." }] — 注意后端用 `href` 不是 `url`
- `sessionId`：关联 session
- `linkScanPath`：提取 projectId 用
- `cwd`：fallback projectId
- `tempo`：活跃度信号
- `inFlight`：当前操作
- `createdAt` / `updatedAt`：ISO 8601 字符串

## 注意：前后端字段差异

以下字段在后端和前端之间有命名/类型差异，后端实现完成后需对齐：
- 后端 `JobChild.href` vs 前端 `JobChild.url`
- 后端 `JobSummary.id` vs 前端 `JobSummary.jobId`
- 后端 `created_at: String`(ISO 8601) vs 前端 `createdAt: number`(Unix ms)
- 后端 `JobsResponse { jobs, badge, badge_count }` vs 前端 `ListJobsResult { jobs, jobsDirExists }`
