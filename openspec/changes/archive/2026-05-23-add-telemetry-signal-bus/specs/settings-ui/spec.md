## ADDED Requirements

### Requirement: Diagnostics tab 暴露 telemetry 快照

Settings 页面 SHALL 在 section 导航中新增 `Diagnostics` tab，与 `General` / `Notifications` 同级。Tab 内容 SHALL 由 `DiagnosticsTab.svelte` 渲染，挂载时调用一次 `getTelemetrySnapshot()` IPC 拿当前快照。

Tab 内容 SHALL 包含四个区域：

1. **顶部仪表盘卡片**（4 个）：cache hit rate（基于 `metadata.cache.hit / (hit + miss + sig_mismatch + stat_err)`）/ IPC error rate（基于 `cdt_api.error / cdt_api.warn` 累计）/ panic count（基于 `panic.recovered`）/ SSH 重连次数（基于 `cdt_ssh.reconnect`）。卡片 SHALL 复用现有 settings 的 design token（不引新组件库 / 图表库）。
2. **延迟分布柱状图**：渲染 `histograms["ipc.list_sessions.duration_ns"].buckets[]` 与 `histograms["ipc.get_session_detail.duration_ns"].buckets[]`，用 SVG 自画 32 个矩形（高度按 bucket count 比例，宽度均分）；图下方文字标 p50 / p95 / p99 数值，**MUST** 在数值旁加 hint："power-of-2 bucket upper bound（实际值 ≤ 此值，最坏 2x 偏差）"，避免用户误读为精确测量。
3. **最近 events 列表**：表格渲染 `recentEvents[]`（最多 100 条），列为 timestamp / kind / fields（fields 显示为 `key=value, ...`）；按 timestamp 倒序。
4. **顶部右上"复制完整 snapshot"按钮**：点击 SHALL 调 `navigator.clipboard.writeText(JSON.stringify(snapshot, null, 2))`，并显示 toast "已复制"；用户报 issue 时一键贴 GitHub。

数据获取策略 SHALL：

- Tab 首次 mount 时拉一次 snapshot；可显示 `loading...` 中间态（settings tab 切换是低频显式操作，不在 hot 用户路径）。
- 提供"刷新"按钮触发再拉一次；按钮按下到数据返回期间 SHALL `silent=true` 保留旧数据展示，避免闪屏。
- SHALL NOT 实现轮询 / 自动刷新——避免抢主线程；用户主动 pull 即可。

Tab 仅读不写，SHALL NOT 暴露任何修改 telemetry 状态的操作；不提供"重置 counter / 清空 events"按钮（保留给 dev tools 后续扩展）。

#### Scenario: 用户打开 Diagnostics tab

- **WHEN** 用户在 Settings 页 sidebar 点击 `Diagnostics` 项
- **THEN** 系统 SHALL 切换到 Diagnostics tab 并调一次 `getTelemetrySnapshot()` IPC
- **AND** SHALL 渲染 4 个仪表盘卡片 + 2 个延迟分布柱状图 + 最近 events 表格 + 复制按钮
- **AND** SHALL 在 1 秒内显示数据（loading 中间态可接受）

#### Scenario: 用户点击复制按钮

- **WHEN** 用户在 Diagnostics tab 顶部点击"复制完整 snapshot"按钮
- **THEN** 系统 SHALL 调 `navigator.clipboard.writeText(JSON.stringify(snapshot, null, 2))`
- **AND** SHALL 显示 toast "已复制"持续 2 秒
- **AND** snapshot JSON SHALL 包含完整 schemaVersion / counters / histograms / recentEvents 字段

#### Scenario: 用户点击刷新按钮

- **WHEN** 用户在 Diagnostics tab 点击刷新按钮
- **THEN** 系统 SHALL 重新调 `getTelemetrySnapshot()` 拿新数据
- **AND** 在新数据返回前 SHALL 保持旧仪表盘 / 柱状图 / events 列表的渲染
- **AND** 新数据到达后 SHALL in-place 替换数值（不经"loading..."中间态）

#### Scenario: tab 仅读不写

- **WHEN** 用户在 Diagnostics tab 任意操作（除复制 / 刷新外）
- **THEN** 系统 SHALL 不提供"重置 counter"或"清空 events"按钮
- **AND** SHALL 不调用任何修改 telemetry 状态的 IPC
