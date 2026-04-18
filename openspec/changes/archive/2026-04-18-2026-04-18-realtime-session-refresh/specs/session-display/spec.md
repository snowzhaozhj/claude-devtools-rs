## ADDED Requirements

### Requirement: Auto refresh on file change

SessionDetail SHALL 在收到命中当前 `(projectId, sessionId)` 的 `file-change`
事件时自动重拉 `getSessionDetail` 并刷新渲染，无需用户手动操作。**同一会话**
短时间内的多次 file change SHALL 合并成一次刷新（in-flight dedupe）。

#### Scenario: 文件追加新消息时自动刷新
- **WHEN** 用户已经打开 session tab `(projectA, sessionX)`
- **AND** 后端 `FileWatcher` 检测到 `~/.claude/projects/projectA/sessionX.jsonl`
  被追加新行，emit `file-change` payload `{ projectId: "projectA",
  sessionId: "sessionX", deleted: false }`
- **THEN** SessionDetail SHALL 调用 `getSessionDetail("projectA", "sessionX")`
  并把返回的 chunks 替换到 `tabStore` 缓存与组件 `$state`
- **AND** 新消息 SHALL 在视觉上追加到对话流末尾

#### Scenario: 非当前会话的事件不触发刷新
- **WHEN** 用户打开 session tab `(projectA, sessionX)`
- **AND** 收到 `file-change` payload `{ projectId: "projectA",
  sessionId: "sessionY", deleted: false }`（同 project 但不同 session）
- **THEN** 当前 SessionDetail SHALL NOT 触发 `getSessionDetail` 重拉

#### Scenario: 同会话多次 file-change 合并刷新
- **WHEN** 同一 session 在 < 200 ms 内连续收到 3 次 `file-change` 事件
- **THEN** SessionDetail SHALL 只发起 1 次 `getSessionDetail` 网络/IPC 调用
  （后续事件复用 in-flight Promise 直至 resolve）

#### Scenario: 用户贴底时刷新后保持贴底
- **WHEN** 刷新触发的瞬间，对话容器满足
  `scrollTop + clientHeight >= scrollHeight - 16`（视为 pinned-to-bottom）
- **AND** `getSessionDetail` 返回新内容并完成渲染
- **THEN** SessionDetail SHALL 在下一帧 (`tick`) 把 `scrollTop` 设为
  `scrollHeight`，让用户继续看到最新消息

#### Scenario: 用户已向上滚动时刷新不抢焦点
- **WHEN** 刷新触发的瞬间，用户已经向上滚动（不满足 pinned-to-bottom 条件）
- **AND** `getSessionDetail` 返回新内容并完成渲染
- **THEN** SessionDetail SHALL NOT 修改 `scrollTop`，用户视图位置保持不变

#### Scenario: 刷新失败保留旧 detail
- **WHEN** 自动刷新过程中 `getSessionDetail` 抛错
- **THEN** SessionDetail SHALL 继续显示旧 `detail`，SHALL NOT 切到 error
  状态；错误 SHALL 通过 `console.warn` 记录但不阻断后续刷新

#### Scenario: tab 关闭后不再刷新
- **WHEN** 用户关闭一个 session tab
- **THEN** 该 tab 对应的 file-change handler SHALL 被注销，后续命中同
  `(projectId, sessionId)` 的事件 SHALL NOT 触发任何刷新
