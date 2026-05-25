# ipc-data-api Specification Changes

## MODIFIED Requirements

### Requirement: Emit push events for file changes and notifications

系统 SHALL 从 main 进程向 renderer 推送以下事件：session 文件变更、todo 文件变更、新通知、SSH 状态变化、context 切换、updater 进度。

桌面（Tauri）host SHALL 在 `setup` 阶段订阅内部 file-change broadcast（enriched event 流——见 `Unified invalidator 作为 LocalDataApi.file_tx 唯一生产者` Requirement），并向前端 webview `emit("file-change", payload)`。Payload 形态契约见 `[[push-events::file-change]]`。

桌面 host SHALL 在 `setup` 阶段订阅 SSH 状态 broadcast，并向前端 webview `emit("ssh_status", payload)`。Payload 形态契约见 `[[push-events::ssh-status-change]]`。同样订阅 context-changed broadcast emit `context_changed` 事件 payload `{ activeContextId, kind }`。

**lag 兜底契约（transport 层行为）**：

- **Tauri host file-change bridge** 在 broadcast receiver `Lagged(n)` 路径 SHALL 通过 `app.emit("sse-lagged", payload)` 通知前端 webview（payload 形态见 `[[push-events::sse-lagged]]`）；`Closed` 时退出 loop
- **Tauri host 其它 bridge**（如 metadata / error）在同类 `Lagged` 路径 SHALL 走同形 sse-lagged 通知
- 前端 Sidebar 收到 `sse-lagged` SHALL 按 `[[sidebar-navigation]]` 已有 Requirement 走保守 silent refresh，覆盖 lag 期间错过的 structural 信号

#### Scenario: New notification while renderer is subscribed

- **WHEN** renderer 已订阅通知事件，期间产出一条新通知
- **THEN** renderer SHALL 在 debounce 窗口内收到一条 push 事件，携带通知 payload

#### Scenario: Tauri 转发 file-change 事件

- **WHEN** 后端 file-change broadcast 产出一条事件
- **AND** Tauri host 在 `setup` 已 spawn 桥任务订阅该 broadcast
- **THEN** webview SHALL 通过 `listen("file-change", ...)` 收到 payload，字段形态与 `[[push-events::file-change]]` 一致

#### Scenario: file_tx 满时 Tauri bridge 通知前端 sse-lagged

- **WHEN** Tauri host file-change bridge 的 broadcast receiver 返回 Lagged(n)（broadcast capacity 满 + slow subscriber）
- **THEN** bridge SHALL emit `sse-lagged` 通知前端（payload 形态见 `[[push-events::sse-lagged]]`）
- **AND** bridge SHALL NOT 退出 loop，继续处理后续 event

#### Scenario: file-change 桥与通知管线并存

- **WHEN** Tauri host 同时持有 file-change bridge 与 detected-error bridge 两个订阅
- **THEN** 两个桥 SHALL 独立运行，文件变更不会因通知 pipeline 的 lag 被丢弃，反之亦然

#### Scenario: ssh_status event broadcast on connect

- **WHEN** 后端 SSH 连接状态从 `connecting` 切到 `connected`
- **AND** Tauri host 在 setup 已 spawn 桥任务订阅 SSH 状态 broadcast
- **THEN** webview SHALL 通过 `listen("ssh_status", ...)` 收到 payload，字段形态与 `[[push-events::ssh-status-change]]` 一致
- **AND** 成功路径下 payload `error` 与 `authChain` 字段 SHALL 为 null 或省略

#### Scenario: ssh_status event carries error detail on failure

- **WHEN** SSH 连接失败导致状态切到 `error`
- **THEN** webview 收到的 `ssh_status` payload SHALL 含 `error` 字段（错误详情）
