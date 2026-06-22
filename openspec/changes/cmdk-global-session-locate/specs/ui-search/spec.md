## MODIFIED Requirements

### Requirement: Command Palette 搜索模式

Command Palette SHALL 以组合视图展示搜索结果：项目区 + 会话区。项目区为本地过滤。会话区在有查询时 SHALL 跨当前 active context 的**所有项目**做 sessionId 全局定位（不依赖当前是否选中项目），并保留"已选中项目时的组内正文搜索"。会话区展示的数据 SHALL 来自前端已加载的 `list_repository_groups` 快照，**不得**为渲染会话区触发任何会读取 session jsonl 的 metadata 扫描。

会话结果行 SHALL 显示足以跨项目定位的信息：title（已加载时）或**完整 sessionId**（无 title 时兜底，不截断）+ 所属项目名。结果行 SHALL 把命中查询的子串高亮显示。打开某会话 SHALL 使用该结果行**自身**的所属项目，不得使用当前选中项目。

"全局"范围 SHALL 限定为当前 active context 的 group 快照：SSH 远程上下文下 SHALL NOT 包含未连接的其他 host。

#### Scenario: 项目过滤
- **WHEN** 用户输入文本
- **THEN** 项目区 SHALL 显示 displayName 或 path 包含查询文本的项目（大小写不敏感），最多 5 条

#### Scenario: 全局 sessionId 定位（跨所有项目）
- **WHEN** 用户输入长度 ≥ 4 的查询文本
- **THEN** 会话区 SHALL 显示当前 active context 下**所有项目**中 sessionId 包含该查询文本的会话（大小写不敏感），无论是否选中项目
- **AND** 结果 SHALL 按确定性顺序排序后再截断到上限（worktree 最近活动时间倒序，同值按项目名 + sessionId 稳定排序）

#### Scenario: 查询过短时不触发全局 id 定位并给出提示
- **WHEN** 查询文本非空但长度 < 4
- **THEN** 会话区 SHALL NOT 启用全局 sessionId 子串匹配（避免 hex id 海量命中）
- **AND** 会话区 SHALL 维持"已选中项目时组内搜索、未选中项目时为空"的行为
- **AND** 未选中项目时 SHALL 显示可见提示（例如"输入 ≥4 个字符按 Session ID 全局定位"），不留无解释空白

#### Scenario: 保留组内正文搜索且不回归
- **WHEN** 用户已选中项目并输入查询文本
- **THEN** 会话区 SHALL 仍包含该项目（组）内正文匹配的会话
- **AND** 该结果与全局 sessionId 命中合并展示

#### Scenario: 跨 worktree 同会话去重（worktree 级确定性）
- **WHEN** 同一 sessionId 在某 group 的多个 worktree 中出现且被同一查询命中
- **THEN** 会话区 SHALL 仅保留一条，不重复展示
- **AND** 保留版本 SHALL 按确定性规则选择（优先 main / repo-root worktree，否则取遍历顺序首条），不依赖前端不存在的 per-session 时间戳

#### Scenario: title 已加载时显示且不发起补齐 IPC
- **WHEN** 全局命中的会话恰在组件当前已加载的会话数据中（已带 title）
- **THEN** 该结果行 SHALL 显示该 title
- **AND** 系统 SHALL NOT 为会话区渲染调用 `listGroupSessions` / `getSessionSummariesByIds` 等补 title 的接口

#### Scenario: title 未加载时的 best-effort 展示
- **WHEN** 全局命中的会话其 title 不在组件已加载数据中
- **THEN** 该结果行 SHALL 显示**完整 sessionId**（不截断）+ 所属项目名（及 worktree/branch）作为定位信息
- **AND** 命中查询的子串 SHALL 被高亮
- **AND** 系统 SHALL NOT 为补齐该 title 触发读取 jsonl 的 metadata 扫描

#### Scenario: 命中数超过上限时显式提示
- **WHEN** 会话区命中数超过展示上限
- **THEN** 会话区 SHALL 仅展示上限条数，并显示"仅显示前 N 条"之类的可见提示
- **AND** SHALL NOT 静默丢弃超出部分而不告知

#### Scenario: 按结果行自身项目打开会话
- **WHEN** 用户在会话区选中并打开一条跨项目命中的会话
- **THEN** 系统 SHALL 以该结果行自身所属的项目 / group 打开该会话
- **AND** SHALL NOT 以当前选中项目作为打开 scope

#### Scenario: 双路命中同会话合并后仍按自身归属打开
- **WHEN** 同一 sessionId 同时被全局 id 定位与当前组正文搜索命中并合并为一条
- **THEN** 合并条目 SHALL 保留正文匹配数（hits）
- **AND** 打开该条目 SHALL 使用其自身的 projectId / groupId，不因合并而错置 scope

#### Scenario: 已打开面板随数据刷新同步
- **WHEN** Command Palette 已打开期间发生 file-change 导致项目 / 会话列表刷新
- **THEN** 已打开面板的会话区 SHALL 反映刷新后的数据（新增会话可被定位、已删除会话不再出现）

#### Scenario: 后端正文搜索失败或滞后时不展示陈旧结果
- **WHEN** 用户修改查询后，上一查询的正文搜索结果尚未被新结果替换（搜索进行中或失败）
- **THEN** 会话区 SHALL NOT 把上一查询的正文命中当作当前查询结果展示
- **AND** 全局 sessionId 定位（纯前端）SHALL 不受后端搜索失败影响，仍可用

#### Scenario: 空查询
- **WHEN** 搜索框为空
- **THEN** SHALL 显示全部项目和（当前选中项目的）会话（受数量限制）
