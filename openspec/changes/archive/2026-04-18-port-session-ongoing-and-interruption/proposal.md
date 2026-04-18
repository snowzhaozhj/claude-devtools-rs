## Why

`CLAUDE.md` "UI 已知遗留问题" 仅剩的一条：session "in progress" 状态检测
与 ongoing / interruption UI 未实现。原版 Electron app
（`../claude-devtools/src/main/utils/sessionStateDetection.ts::checkMessagesOngoing`）
通过按序记录活动栈，在最后一个 ending event（`text_output` / `interruption`
/ `exit_plan_mode`）之后若仍有 AI 活动（`thinking` / `tool_use` /
`tool_result`）即判定 ongoing，配合 sidebar 绿点与会话底部 "Session is
in progress..." 蓝色横幅显示。Rust port 缺失该算法、`SessionSummary` /
`SessionDetail` 无 `isOngoing`，UI 没有对应组件。

附带一条 impl-bug：`crates/cdt-parse/src/noise.rs` 把 interrupt marker
（以 `[Request interrupted by user` 起首的 user 消息）归为 `HardNoise`
完全过滤，与原版"保留为 `interruption` semantic step 并作为 ending
event 参与 ongoing 判定"的语义相反。必须先把 interrupt 从 `HardNoise`
拎出来成独立 `MessageCategory::Interruption` 才能接下来把它保留到
`AIChunk.semantic_steps` 里供 UI 渲染。

实时 file-change 桥已在 `2026-04-18-realtime-session-refresh` 就位，
ongoing / interruption 的"绿点变白"依赖已具备，正是推进本 change 的时机。

## What Changes

### 数据层

- `cdt-core::HardNoiseReason` 删除 `InterruptMarker` 变体；
  `MessageCategory` 新增 `Interruption` 一类（非 hard noise，仍需经
  chunk-building 过滤 sidechain 后保留并产出 semantic step）
- `cdt-core::SemanticStep` 新增 `Interruption { text, timestamp }` 变体，
  对齐 TS 的 `SemanticStep.kind === "interruption"` 渲染契约
- `cdt-parse::noise.rs` 不再把 interrupt prefix 归为 `HardNoise`；
  解析入口在 `classify_hard_noise` 返回 `None` 后额外调用
  `classify_interrupt` 判定，命中时赋 `MessageCategory::Interruption`
- `cdt-analyze::chunk::builder` 对 `Interruption` category 消息：flush
  当前 assistant buffer 时给当前 / 前一个 `AIChunk` 的 `semantic_steps`
  末尾 push `SemanticStep::Interruption`；无前驱 AI buffer 则丢弃
- `cdt-analyze` 新模块 `session_state`：`check_messages_ongoing(messages)
  -> bool` 端口 TS 算法——记录活动栈 → 找最后 ending event → 之后若有
  AI 活动则 ongoing

### API + Tauri 透传

- `cdt-api::SessionSummary` 与 `SessionDetail` 新增 `is_ongoing: bool`
  （`rename_all = "camelCase"` 序列化为 `isOngoing`）
- `cdt-api::ipc::session_metadata`：`SessionMetadata` 扫描时额外用
  `check_messages_ongoing` 计算 `is_ongoing`
- `cdt-api::ipc::local::get_session_detail`：parse + build_chunks 之后
  直接对 `&messages` 调 `check_messages_ongoing`
- Tauri 层无需改动：两个 struct 通过 `serde_json::Value` 透传，新字段
  自动抵达前端

### UI

- `ui/src/components/OngoingIndicator.svelte`（新文件）：绿点脉冲动画，
  两个导出 `<OngoingIndicator>`（尺寸 sm/md + 可选 label）与
  `<OngoingBanner>`（底部横幅 + spinner + 蓝色文案）
- `ui/src/components/Sidebar.svelte`：session-item title 前按 `isOngoing`
  渲染绿点（PINNED 与日期分组两处分支）
- `ui/src/routes/SessionDetail.svelte`：底部 conversation 区之后按
  `detail.isOngoing` 渲染 `<OngoingBanner>`；semantic-step 渲染处新增
  `kind === "interruption"` 分支（红/灰色 badge + "Session interrupted
  by user"）
- `ui/src/lib/api.ts`：`SessionSummary` / `SessionDetail` 类型补
  `isOngoing: boolean`

## Capabilities

### Modified

- `session-parsing`：`Classify hard noise messages` Requirement 从 hard
  noise 列表移除 interrupt marker；新增 `Classify interrupt marker
  messages` Requirement 描述 interrupt 独立 `MessageCategory::Interruption`
  分类
- `chunk-building`：`Filter sidechain and hard-noise messages` Scenario
  示例同步（interrupt marker 不再在 hard-noise 删除名单中）；
  `Extract semantic steps for AIChunks` Requirement 增补 `Interruption`
  变体；ADDED `Emit interruption semantic step for interrupt-marker
  messages` Requirement 描述 AIChunk 附着规则
- `ipc-data-api`：`Expose project and session queries` Requirement 增补
  `SessionSummary` / `SessionDetail` 必须携带 `isOngoing` 字段
- `session-display`：ADDED `Ongoing banner at session bottom` 与
  `Interruption semantic step rendering` Requirement
- `sidebar-navigation`：ADDED `Ongoing indicator on session item`
  Requirement

## Impact

- 代码：`cdt-core`（枚举变体增减）、`cdt-parse`（noise.rs + lib.rs 分类
  入口）、`cdt-analyze`（chunk builder + 新 `session_state` 模块）、
  `cdt-api`（types + session_metadata + local 两处 ongoing 计算）、
  `ui`（新组件 + Sidebar/SessionDetail 渲染 + api.ts 类型）
- 依赖：无新增 crate / npm 包
- 测试：新增 `check_messages_ongoing` 单元测试（覆盖原版 5 种信号：
  纯 text ending、interrupt text、tool rejection、shutdown response、
  ExitPlanMode）+ chunk builder 的 Interruption step 追加用例；
  `cdt-parse` 更新 interrupt 分类测试；`cdt-api` 的 `SessionSummary`
  序列化 snapshot（若有）更新；前端走 `npm run check --prefix ui`
- 向后兼容：`HardNoiseReason::InterruptMarker` 被删除属于公共 API
  破坏，但仅 `cdt-parse`/`cdt-analyze` 内部使用，无外部消费者；
  IPC 层只是新增字段，旧前端不关心即忽略
