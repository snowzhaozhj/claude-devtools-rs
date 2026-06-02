## MODIFIED Requirements

### Requirement: Auto refresh session list on file change

当后端 `file-change` 事件命中**当前选中的项目**时，Sidebar SHALL 重拉
`listGroupSessions` 刷新会话列表，无论命中事件中的 `sessionId` 是否已存在于现有
列表（覆盖"新会话首次写入"场景）。同一 project 短时间内多次事件 SHALL 合并
为一次 `listGroupSessions` 调用。

**合并窗口（debounce）分层**：
- **结构性事件**（`sessionListChanged === true` 或 `deleted === true`）SHALL 使用 `scheduleRefresh` key `"sidebar-structural:${groupId}"` + 250ms 合并窗口（保持响应性）
- **非结构性事件**（`sessionListChanged === false` 且 `deleted === false` 且 `projectListChanged === false`，即普通 JSONL 追加）SHALL 使用 `scheduleRefresh` key `"sidebar-append:${groupId}"` + 1000ms 合并窗口（降低高频写入场景的 IPC 压力）

两类事件使用**独立 key** 避免 trailing timer 冲突——`scheduleRefresh` 已有 `trailingTimers.has(key) → return` 语义，同 key 混用不同窗口会导致结构性事件被非结构性的 1000ms trailing 卡住。两个 key 调用同一个 `loadSessions(groupId, true)` 函数；如两个 timer 巧合同时 fire，`dedupeRefresh` 内层合并为一次 IPC。

#### Scenario: 当前 project 命中时刷新列表
- **WHEN** 用户当前选中 `selectedGroupId = "groupA"`
- **AND** 收到 `file-change` payload `{ projectId: "projectA" (属于 groupA), sessionId: <任意>, deleted: false }`
- **THEN** Sidebar SHALL 调用 `listGroupSessions("groupA")` 并更新 `sessions` 状态

#### Scenario: 非当前 project 命中时不刷新
- **WHEN** 用户当前选中 `selectedGroupId = "groupA"`
- **AND** 收到 `file-change` payload 的 projectId 不属于 groupA
- **THEN** Sidebar SHALL NOT 触发 `listGroupSessions`

#### Scenario: 新 session 写入时出现在列表
- **WHEN** `~/.claude/projects/projectA/` 下首次创建一个新 session 文件 `<newSid>.jsonl` 并写入第一行
- **AND** 用户当前选中的 group 包含 projectA
- **THEN** 该 `newSid` 对应的 SessionSummary SHALL 出现在 Sidebar 列表中（根据 timestamp 落到对应日期分组）

#### Scenario: 同 project 多次 file-change 合并刷新
- **WHEN** 同一 project 在短时间内连续收到 3 次非结构性 `file-change` 事件
- **THEN** Sidebar SHALL 只发起 1 次 `listGroupSessions` IPC 调用（1000ms 合并窗口）

#### Scenario: 结构性事件保持 250ms 响应
- **WHEN** 收到 `file-change` payload `{ sessionListChanged: true }` 或 `{ deleted: true }`
- **THEN** Sidebar SHALL 在 250ms 合并窗口内触发 `listGroupSessions`（不等待 1000ms）

#### Scenario: 删除事件也触发刷新
- **WHEN** 收到 `file-change` payload `{ projectId: "projectA", sessionId: "sessionX", deleted: true }`，且当前 group 包含 projectA
- **THEN** Sidebar SHALL 在 250ms 内触发 `listGroupSessions` 让 `sessionX` 从列表中消失

#### Scenario: 切换 project 后旧 project 的事件不再刷新
- **WHEN** 用户已经从 groupA 切到 groupB
- **AND** 此时延迟到达一条 groupA 内 project 的 `file-change` 事件
- **THEN** Sidebar SHALL NOT 调用 `listGroupSessions`
