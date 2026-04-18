## Context

现有零件：

- `cdt-watch::FileWatcher::subscribe_files() -> broadcast::Receiver<FileChangeEvent>`
  已是 MPMC，自动通知管线（`NotificationPipeline`）已是其中一个订阅者
- `FileChangeEvent { project_id, session_id, deleted }` 已 derive
  `serde::Serialize/Deserialize`，可直接通过 Tauri `emit` 透传
- Tauri 侧已有现成的"setup 内 spawn 后台 task → 桥接 broadcast 到 emit"模式
  （见 `src-tauri/src/lib.rs::326-379` 的 `error_rx → notification-added` 桥）
- `tauri-plugin-log`、`tauri::async_runtime::spawn`、`AppHandle::emit` 全部已就位
- 前端已有相同模式的全局 listener（`App.svelte::44-46` 监听 `notification-update`
  / `notification-added`）；本 change 把同类逻辑抽到 `fileChangeStore` 是因为
  路由组件而非 root 组件需要订阅，多组件订阅必须共享一个底层 `listen()` 句柄

缺口：

- **后端**：watcher 已经在跑，subscribe_files 也开放了，但 Tauri 侧 setup 完
  全没订阅（`src-tauri/src/lib.rs` grep `subscribe_files` = 0 命中）。前端想
  收到 file-change 没渠道
- **前端**：`SessionDetail.svelte:54-69` `onMount` 一次性 `getSessionDetail`，
  无任何 file-change listener；同理 Sidebar `loadSessions` 只在 `$effect` 切
  project 时跑一次

约束：

- `cdt-api` 不能依赖 `tauri`（同一约束已在 notifier change 里被遵守）；
  `file-change` emit 必须在 `src-tauri` 里做
- `LocalDataApi` 已经持有 watcher 的引用
  （`new_with_watcher(...)` → `subscribe_detected_errors`），但 watcher 句柄并
  未存在 api 内（仅在 ctor 内 subscribe 一次）。本 change 不改 `LocalDataApi`
  签名——直接复用 `src-tauri::run` 里已经存在的 `let watcher = Arc::new(...)`
  在 setup 之前 `watcher.subscribe_files()` 拿一份 receiver
- `FileChangeEvent` 字段已是 snake_case (`project_id` 等)，但 `cdt-core` 整个
  仓库的对外 IPC 类型是 camelCase（`#[serde(rename_all = "camelCase")]`）。
  `FileChangeEvent` 当前 **没有** `rename_all` 注解，序列化出去会是
  `project_id` / `session_id`——保留 snake_case 与现有"前端约定 camelCase"不
  一致。本 change 给 `FileChangeEvent` 与 `TodoChangeEvent` 加
  `#[serde(rename_all = "camelCase")]`，前端按 `event.payload.projectId /
  sessionId / deleted` 读取

## Goals / Non-Goals

**Goals:**
- 打开会话窗口期间，session 文件被追加 → UI 在 ≤ ~150 ms 内（100ms debounce +
  parse + emit + render）显示新增的 chunks
- 创建新会话或现有会话有更新 → 当前选中 project 的 sidebar 列表自动刷新
- 同一 session 短时间内多次 file change → 合并成一次 `getSessionDetail`，避免
  并发 refetch 浪费 CPU 与 IPC
- 用户已经滚到底（"pinned-to-bottom"）的对话窗口 → 刷新后保持贴底；用户已经
  向上滚动查看历史 → 刷新后保持滚动位置（不抢焦点）
- 不破坏现有 `notification-update` / `notification-added` 桥；不破坏现有 4 个
  `cdt-api` 集成测试（不动 `LocalDataApi` 签名）

**Non-Goals:**
- 不做"显示 toast 提示有更新"——直接刷新即可，原版也不显示
- 不做按 line offset 的增量刷新——全量 `getSessionDetail`（与 notifier 全量
  parse 决策一致；session < 几千行可接受）
- 不实现新增 session 时"自动打开 tab" 之类副作用——只刷新 sidebar 列表，是否
  打开由用户点击决定
- 不引入持久化的滚动锚点；pinned-to-bottom 仅以"刷新瞬间贴底"判定
- 不实现 ongoing/interruption 检测（依赖本 change，但属于下一个 change）
- 不动 cdt-watch 的 debounce、不动 broadcast 容量

## Decisions

### 1. `FileChangeEvent` serde 改 camelCase

**选择**：在 `crates/cdt-core/src/watch_event.rs` 给 `FileChangeEvent` 与
`TodoChangeEvent` 加 `#[serde(rename_all = "camelCase")]`。

**原因**：
- 项目约定 `所有面向前端（Tauri IPC）的 struct 必须 camelCase`（CLAUDE.md
  Conventions），`FileChangeEvent` 一旦走 IPC 就属于这条约束的目标
- `notifier` 内部消费 `FileChangeEvent` 是直接 Rust 结构访问 (`event.session_id`)，
  不经 serde，所以 rename 对内部 0 影响
- 既有 `notifications.json` 不持久化 `FileChangeEvent`，迁移 0 风险

**代价**：极轻微 BREAKING：若有外部代码反序列化 `FileChangeEvent` JSON，需要
切到 camelCase。**实际仓内 grep 结果**：除测试外没有其他反序列化点（事件只在
进程内通过 broadcast 传，到了 Tauri 边界才序列化一次）。

### 2. 前端单例 `fileChangeStore`，不直接在每个组件 listen

**选择**：模块级 `let unlisten` + 一次性 `listen("file-change", ...)`，对外暴
露 `registerHandler(key: string, fn: (e: FileChangeEvent) => void)` 与
`unregisterHandler(key)`。组件 `onMount` 注册、`onDestroy` 取消。

**原因**：
- Tauri 的 `listen` 每次调都会向 main 加一个订阅句柄，多组件分别 listen 会产
  生 N 个 handle；通过共享 dispatcher 减少 IPC fan-out
- 模块级状态在 Svelte 5 SSR/HMR 下天然单例（与 `tabStore`、`sidebarStore` 同
  模式）
- key 用调用方提供的字符串（如 `session-detail-${tabId}` / `sidebar`），方便
  组件 unregister 自己

**替代方案**：
- A. 每组件直接 `listen` —— 简单但产生多份 handle，多次反序列化
- B. 在 `App.svelte` 集中处理并通过 prop 下传 —— 路由组件嵌套深，prop drilling
  噪音大

### 3. 同 session 并发 refetch 的 dedupe 策略

**选择**：`fileChangeStore` 内部 `Map<sessionKey, Promise<void>>`（key =
`${projectId}|${sessionId}`，sidebar 用单独 key `sidebar:${projectId}`）。
handler 内若该 key 已有 in-flight Promise，**复用**该 Promise 而不发起新请
求；Promise resolve 后从 map 删除。

**原因**：
- 完全消除 100 ms debounce 内可能产生的 burst（macOS FSEvents 有时一次写产生
  多个 raw event）
- 不引入 setTimeout 二次去抖（watcher 已经做过），保持低延迟
- 实现简单：`getOrCreate(key, () => promise)` 一行

**代价**：刷新过程中真正的"新写入"会被合并掉一轮——可接受，因为下次 file
change 会再次触发，最多延迟一次 debounce 窗口（100 ms）

**替代方案**：
- A. requestAnimationFrame 合并 —— 跨多次 file change 难判断聚合边界
- B. 用 RxJS `auditTime` —— 引依赖不合算

### 4. Pinned-to-bottom 检测

**选择**：handler 触发刷新前，记录 `wasAtBottom = scrollTop + clientHeight
>= scrollHeight - 16`（16 px tolerance），刷新后 `tick().then(() => { if
(wasAtBottom) conversationEl.scrollTop = conversationEl.scrollHeight; })`。

**原因**：
- 16 px tolerance 容忍最后一行 padding/margin 误差
- `tick()` 等 Svelte reactive DOM 更新完成后再读 `scrollHeight`
- 不动用户主动向上滚的位置

**替代方案**：
- A. `IntersectionObserver` 监听一个 sentinel —— 实现复杂、代价高
- B. 保存 `scrollTop` 然后强制 restore —— 新内容追加后 `scrollTop` 数值意义
  不变，但用户体感"内容被推走"

### 5. 后端 emit 时机

**选择**：在 `tauri::Builder::setup` 里 spawn 第三个 task：

```rust
let mut file_rx = watcher.subscribe_files();
let app_handle_for_files = app.handle().clone();
tauri::async_runtime::spawn(async move {
    loop {
        match file_rx.recv().await {
            Ok(event) => { let _ = app_handle_for_files.emit("file-change", &event); }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
});
```

**原因**：
- 与 `error_rx → notification-added` 桥同形态，复用熟悉模式
- 在 `watcher.start()` spawn 之后 subscribe，避免错过初始事件——但 broadcast
  channel 是"订阅时拿独立 receiver，不会丢已发的"，先 spawn watcher 再 subscribe
  也可以（实测 watcher.start() 是 await loop，`spawn` 立即返回，所以两个 spawn
  在事件循环里几乎同时跑，竞争窗口在 100 ms debounce 内可忽略）。**实现时按
  notifier 同样的顺序：先 spawn watcher.start，再 subscribe_files 拿
  receiver，再 spawn 桥 task**

### 6. Sidebar 何时算"命中当前 project"

**选择**：`event.projectId === selectedProjectId`。即使 `event.deleted === true`
也刷新——session 被删除时也希望从列表消失。`event.sessionId` 是否在现有
`sessions[]` 中**不参与判断**：新 session 写第一行时也会触发 file change，但
当时它不在 sessions[]，所以"必须刷新拿到它"。

**原因**：
- 简单且正确
- `loadSessions` 已是幂等 IPC，重复调用代价小（< 50 ms）

### 7. `cdt-api` 不暴露 `subscribe_files`

**选择**：直接在 `src-tauri::run` 顶层 `let watcher = Arc::new(FileWatcher::new())`
（已有），通过 `watcher.subscribe_files()` 拿 receiver，不在 `LocalDataApi` 上
加 trait/方法。

**原因**：
- `LocalDataApi` 已是 `Arc` 共享给 commands；让它再持 `Arc<FileWatcher>` 然后
  暴露 `subscribe_files` 等于把 `FileWatcher` 类型从 `cdt-watch` re-export 出
  去——污染 `cdt-api` 公共 API
- watcher 的 lifetime 在 `src-tauri::run` 的 `rt.block_on` 之外被
  `tauri::Builder::setup` 的 closure 捕获后保持存活；和 notifier 同 owner

**替代方案**：
- A. `LocalDataApi::subscribe_file_changes()` —— 多一个非 trait 方法，但
  `LocalDataApi` 当前没存 watcher（`new_with_watcher` 只是 subscribe 后丢弃
  watcher 引用），改这个会同步触发"watcher 必须存进 api"的连锁改动

## Risks / Trade-offs

- [**broadcast 256 容量满 risk**] 如果用户开了一个超活跃的项目（多个 session
  同时被写），事件可能堆积超过 256 → 前端会跳几个事件。**缓解**：`Lagged(n)`
  分支只 `continue`，下次 file change 会重新触发刷新；UI 最差延迟一次 debounce
  窗口
- [**重复 emit 占带宽 risk**] 同一会话每追加 1 行就 emit 一次 → IPC fan-out 多
  → 但 payload 只有 3 个字段，单条 < 200 bytes；前端 dedupe 后实际 refetch 一
  次。可接受
- [**滚动跳动 risk**] pinned-to-bottom 检测在 `tick()` 后赋 `scrollTop` 可能
  在用户手动 scroll 的瞬间打架 → 16 px tolerance + 仅在 `wasAtBottom` 为 true
  时赋值，用户向上滚后 `wasAtBottom` 为 false，不动他
- [**新 session 自动出现的 UX risk**] 用户在 sidebar 看到列表突然多一项可能
  会困惑。**判断**：原版同样行为，未引发抱怨；不增加额外 UI 提示
- [**测试覆盖 risk**] 本 change 主路径是 Tauri runtime + 浏览器渲染，难以纯
  Rust 单测覆盖。依赖：`cdt-watch::FileWatcher` 的多订阅者测试 + 手动
  `cargo tauri dev` 验证。这是已有 UI changes 的常态做法（如
  `2026-04-17-auto-notification-pipeline` 的 7.7 任务也只能手工验证）
- [**HMR 重复订阅 risk**] Vite HMR 重新加载组件会再触发 `onMount` →
  `registerHandler`。`fileChangeStore` 模块级 `unlisten` 不重置；如果模块也被
  HMR 重载会重复 `listen`。**缓解**：在模块顶部用 `if (typeof
  window.__fileChangeStoreInited === 'undefined')` 守卫，或接受 dev-only 重复
  订阅（生产构建无 HMR 不影响）

## Migration Plan

1. **改 `FileChangeEvent` serde camelCase**（最小独立改动）：单点 + grep 验证
   无外部反序列化点
2. **后端 spawn 第三个 task** 把事件桥到 emit
3. **新建 `fileChangeStore`**（前端单例 listener + dedupe map）
4. **SessionDetail 注册** + pinned-to-bottom 滚动保持
5. **Sidebar 注册**
6. **手动 `cargo tauri dev` 验证**：开两个终端，一个跑 dev、一个 echo append
   到某 session jsonl，观察 UI 自动追加 + sidebar 计数变化
7. **更新 followups.md / CLAUDE.md**
8. **archive**

**回滚**：
- 后端：删第三个 spawn task
- 前端：删 `fileChangeStore` 文件 + 移除 SessionDetail/Sidebar 的 register 调
  用
- serde 改动可保留（无副作用，且已校齐项目约定）

## Open Questions

- 是否需要在 `file-change` payload 里加 `event_kind: "modified" | "deleted"`
  （比 `deleted: bool` 更扩展）？—— 暂不做，`bool` 已覆盖当前需求；未来加
  `created` 时再扩
- 大量 session 的 sidebar refresh 是否会卡（数百个 session 的项目）？——
  `listSessions` 当前用 `pageSize=50` 默认值，不会一次拉所有；本 change 不调
  pageSize，沿用既有行为
- 是否需要在 `getSessionDetail` 重新成功后通知 user "已更新" toast？—— 不做，
  原版未做，且会打扰用户
