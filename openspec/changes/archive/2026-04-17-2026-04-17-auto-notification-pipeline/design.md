## Context

现有零件：

- `cdt-watch::FileWatcher` 已支持 `subscribe_files() -> broadcast::Receiver<FileChangeEvent>`（含 `{project_id, session_id, deleted}`），内部 100ms debounce
- `cdt-config::detect_errors(messages, triggers, session_id, project_id, file_path)` 为纯函数，无状态
- `cdt-config::NotificationManager::add_notification(error)` 自动 prune(100) + 持久化到 `~/.claude/claude-devtools-notifications.json`
- `cdt-parse::parse_file(path)` 异步读整个 JSONL 产出 `Vec<ParsedMessage>`
- Tauri `mark_notification_read` 已 `emit("notification-update", ())`；前端 `tabStore` 已 `listen` 并刷新 badge

缺口：**没有一段代码订阅 FileWatcher 并把事件桥到 detect_errors + add_notification**。`DetectedError.id` 是 UUID，每次重扫都换 id，无法天然去重。

约束：

- `cdt-api` 不能直接依赖 `tauri`（tauri 在 `src-tauri` workspace 外）——事件到 renderer 的桥必须在 `src-tauri` 里做
- `ConfigManager`/`NotificationManager` 都被 `Mutex<>` 包，notifier 要和 Tauri commands 共享它们而不是各持一份
- 增量扫描成本：每次 file change 就全量 parse 整个 session 是 MVP 可接受的（session 通常 < 几千行），优化留后

## Goals / Non-Goals

**Goals:**
- FileWatcher → detect_errors → NotificationManager → 前端 badge 的自动管线通
- 同一错误不重复入库（跨重启、跨 file change）
- 既有手写 notification（从设置页测试、从 CLI 手动添加等路径）不破坏
- 既有 `notifications.json` 文件内历史 uuid id 的数据可正常加载
- 新的集成测试覆盖"写新行到 JSONL → 管线产出 DetectedError"端到端路径

**Non-Goals:**
- 不做按 line offset 的真正增量解析（每次 file change 重解整个 session；详见下方 Decision 2）
- 不改 `detect_errors` 签名（仍是纯函数）
- 不把 `FileWatcher` 或 notifier 拉进 `cdt-cli` 的 HTTP API 路径（本次只覆盖 Tauri 桌面路径）
- 不实现 token_threshold trigger 的 usage 数据源替换（当前用 char len / 4，与 TS 对齐，不动）
- 不做前端"notification-added" 声音/系统级通知（后续可接 tauri-plugin-notification）
- 不做按 trigger 的 per-session 游标存储（`processed_up_to_line` 不持久化，内存即可）

## Decisions

### 1. `DetectedError.id` 改为确定性 hash

**算法**：SHA-256(`session_id + '\0' + file_path + '\0' + line_number + '\0' + tool_use_id.unwrap_or("") + '\0' + trigger_id.unwrap_or("") + '\0' + message`) 取前 16 字节 hex（32 个字符）。

**原因**：
- 同一 (session, line, tool_use, trigger, message) 必然产生相同 id → 重复检测自然去重
- 不同 trigger 匹配到同一行会产生不同 id（正确，应该多条通知）
- `message` 纳入 hash 以容忍 trigger pattern 修改后重跑产生的真正新语义
- SHA-256 够短又够稳，不引 `uuid::v5`（即使 uuid v5 也要引 namespace，写起来更啰嗦）

**代价**：引入 `sha2` workspace dep（大多数 Rust 项目都会有；本仓库未用）。

**替代方案**：
- A. `uuid::v5(NAMESPACE, seed)` —— 需要固定 namespace 常量，可读性不如 hex
- B. blake3 —— 已在 `cdt-analyze`？实际检查后发现未启用，单为 id 生成引 blake3 不合算
- C. 自己拼 `format!` 字符串作为 id —— 可读但冗长（一个 id 几百字节）

### 2. 全量 re-parse vs 增量

**选择**：file change 事件来了 → `parse_file(path)` 全量解析 → 跑 `detect_errors` → `NotificationManager::add_notification` 逐个（每个都会 dedup）。

**原因**：
- Session 文件通常 < 几千行，单次 parse 亚秒级
- 确定性 id + add 时 dedup 让重复扫描零副作用
- 避免维护 per-session 字节偏移 / 行号游标的持久化复杂度
- 与 `cdt-watch` 的 100ms debounce 天然配合——用户敲代码触发的多次 FS 写最终只会触发一次全量 parse

**代价**：大 session（例如 2w+ 行）每次追加会有百 ms 级 parse。如果成为瓶颈，M2 再加游标优化。

**替代方案**：
- A. 每 session 维护 `last_processed_line: usize` 内存游标，只跑新 tail —— 增量复杂度大（怎么判断 `parse_file` 的 `ParsedMessage` 切片与 line 对齐？），收益低
- B. 用 `jsonl` 流式解析器按行 yield —— 需要 `parse_file` 提供 stream API，扩大改动面

### 3. notifier 代码放在 `cdt-api` 而不是新建 `cdt-notify`

**选择**：`crates/cdt-api/src/notifier.rs`，导出 `pub struct NotificationPipeline` + `pub fn start(...)`，在 `LocalDataApi::new_with_watcher` 里 spawn。

**原因**：
- notifier 同时需要 `NotificationManager` 和 `ConfigManager`（读 triggers）——两者都是 `LocalDataApi` 的字段
- 新开 crate 要拆 Mutex 共享或引 `Arc`，绕远路
- `cdt-api` 已经依赖 `cdt-config`、`cdt-parse`；新加 `cdt-watch` path dep 符合已有方向（`cdt-api` 是 facade）
- 原版 TS 也是在"主进程 services"层做的，没单独服务

**替代方案**：
- A. 新 `cdt-notify` crate —— 需要 `Arc<Mutex<NotificationManager>>` 作为入参，`LocalDataApi` 和 `NotificationPipeline` 都持有同一个 `Arc`，改动面更大
- B. 放在 `cdt-watch` —— `cdt-watch` 不应该知道 `ConfigManager`/`NotificationManager`，破坏 capability 边界

### 4. `LocalDataApi` 构造：新增 `new_with_watcher`，旧 `new` 不变

**选择**：保留现有 `LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr)`（已被 4 个集成测试用），新增 `new_with_watcher(scanner, config_mgr, notif_mgr, ssh_mgr, watcher: Arc<FileWatcher>)`。Tauri 侧用新构造器，测试继续用旧的。

**原因**：
- 避免破坏 `crates/cdt-api/tests/{notifications, trigger_crud, session_summary_title, ...}`
- 测试里没 FileWatcher 是正常的（不想监听真实 `~/.claude/projects/`）
- notifier pipeline 在新构造器里 spawn，旧构造器下无 notifier

**对 `LocalDataApi` 字段**：新增 `watcher: Option<Arc<FileWatcher>>` + `error_tx: Option<broadcast::Sender<DetectedError>>`。`subscribe_detected_errors()` 返回 `Option<broadcast::Receiver>` 或在无 watcher 时返回一个永远空的 channel（选择后者以简化 caller 代码）。

### 5. notifier 的并发模型

**选择**：一个 `spawn`ed task，顺序处理 `file_rx.recv()`。每次事件内串行 `parse_file → detect_errors → add_notification`。

**原因**：
- `NotificationManager` 被 `Mutex` 保护，并发 add 也要排队
- Parse 慢时后续事件会堆积，但 `broadcast::channel` 256 容量够用
- 过载时 `RecvError::Lagged(n)` 跳过 n 个——设计上可接受（下次事件来时仍会全量扫该 session）

**替代方案**：
- A. 每个 file event 起一个新 task 并发 parse —— Mutex 竞争 + 可能的乱序 add，复杂度不合算

### 6. Tauri 侧事件命名

**选择**：`emit("notification-added", DetectedError)`（payload 是单条 error）。现有 `notification-update`（空 payload）继续用于 "mark as read" 后的 badge 刷新。

**原因**：
- `notification-added` 带 payload 让前端能**增量**更新 UI（不必全量 reload 100 条）
- 与 `notification-update` 语义分离：added 是新条目、update 是已有条目状态变
- 前端 `NotificationsView` 可以在收到 added 时 prepend 到列表顶

### 7. `subscribe_files()` 广播是否够用

**是**。`broadcast::Receiver` 是 MPMC，多订阅者各拿各的 deep copy。notifier 是一个订阅者，未来若加其他订阅者（例如 SSE endpoint）不互相影响。

## Risks / Trade-offs

- [**历史 notification id 与新算法不兼容 risk**] 已存在 `notifications.json` 里的 uuid id 不会被重新哈希，与新 hash id 可能指向同一 (session,line,trigger) → `add_notification` 对这些"旧 id + 新 id"会同时存在。缓解：启动时不清洗历史数据；通过 prune(100) 自然淘汰；用户手动清空也可重置
- [**大 session re-parse 性能 risk**] 超大 session 每次 append 都全量 parse → M2 若成为瓶颈，加 `last_processed_line` 游标优化。当前 acceptance：< 5000 行 session < 100ms parse
- [**Mutex 长持 risk**] `NotificationManager::add_notification` 内部 `save().await` 期间持锁 → 与 Tauri commands 的 `get_notifications` / `mark_as_read` 会短暂等待。可接受：save 是几 KB 文件的 write_all，亚毫秒级
- [**broadcast lag risk**] 若前端 listen 未及时取走消息，channel 会丢旧消息（容量 256）→ 对通知场景影响小（下次 file change 会再次触发全量扫描，漏的通知会补回）
- [**测试集成难度**] 集成测试要启真实 `FileWatcher` 监听 tmp 目录 → macOS symlink `/var` → `/private/var` 已在 `FileWatcher::with_paths` canonicalize 过；fixture 里直接写 JSONL 即可
- [**跨 session 错误误报**] 不同 session 写同一个 error message 会产生不同 id（因为 session_id 不同），不会互相覆盖——符合预期

## Migration Plan

1. **改 `create_detected_error` 为确定性 id**：单点修改，`cdt-config` 单测 + `cdt-api` 既有 notifications 测试重跑
2. **`NotificationManager::add_notification` 加 dedup**：不破坏既有 "add then get" 测试（因为第一次 add 就是新条目）；新增 "add twice same id" 测试
3. **新建 `cdt-api::notifier`**：`NotificationPipeline::start(...)` 独立单测（用 mock trigger + in-memory messages）
4. **`LocalDataApi::new_with_watcher`**：新构造器 + 旧构造器并存
5. **集成测试 `notifier_pipeline.rs`**：tmp 目录、真实 FileWatcher、写 JSONL 行、订阅 `subscribe_detected_errors()`，断言收到 error
6. **Tauri 接入**：`setup` 里 spawn；前端 listen `notification-added`
7. **回归手工验证**：`cargo tauri dev` 配置一个 `is_error` trigger，打开某个有错误的 session，观察 badge 是否从 0 变 N

**回滚**：`LocalDataApi::new` 旧构造器保留即可回滚——Tauri 侧把 `new_with_watcher` 换回 `new`，管线不起；其他改动（确定性 id、dedup）不影响功能正确性。

## Open Questions

- 是否需要启动时对 `~/.claude/projects/` 做一次全量扫描（而不是等 file change）？原版 TS 有"启动扫最近 N 天 session"的初始化逻辑 → 本次先不做，避免启动卡；用户打开应用后第一次 file append 就会触发管线。未来若用户反馈"历史错误看不到"再补。
- `DetectedError.message` 可能达 500 字符（`truncate_message` 上限）；纳入 hash 有点浪费 CPU 但 SHA-256 几 KB 数据亚微秒，可接受
- 是否把 `is_meta` 消息过滤放进 `detect_errors`？—— 不做，`detect_errors` 保持纯函数语义；notifier 在调用前可以按需过滤，但 `is_meta=true` 的消息一般不含 tool_result，天然不触发，省略过滤无副作用
