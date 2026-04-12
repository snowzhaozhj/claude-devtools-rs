## Context

`cdt-watch` crate 目前为空壳（`lib.rs` 只有一个空 `pub mod`）。TS 侧 `FileWatcher.ts` 使用 Node.js `chokidar` 库，以 100ms 去抖监听两个目录，通过 Electron IPC 将事件广播给 renderer 和 HTTP 客户端。

`followups.md` 标记 file-watching 为 ✅ 完全匹配，无 impl-bug 需要修正，Rust port 直接按现有 spec 实现即可。

## Goals / Non-Goals

**Goals:**
- 实现跨平台文件系统监听（`notify` crate）
- 实现 100ms 去抖逻辑（spec 硬约束）
- 使用 `tokio::sync::broadcast` 多订阅者分发
- 暴露公共 API：`FileWatcher::new()`、`FileWatcher::subscribe()`、`FileWatcher::start()`
- 补齐 spec 全部 5 个 Scenario 对应的测试

**Non-Goals:**
- 不实现 IPC / HTTP / SSE 传输层（属于 `cdt-api`）
- 不实现 SSH 远端文件监听（属于 `cdt-ssh`）
- 不实现配置热加载（属于 `cdt-config`）

## Decisions

### 决策 1：使用 `notify` crate 而非手动 `inotify`/`FSEvents`/`kqueue`
- `notify` v6 封装了所有平台的 native 事件，支持 recursive watch，社区维护活跃
- 备选方案：直接调用 `inotify-sys`（Linux-only）或 `fsevent-sys`（macOS-only），需要各平台分支代码，维护成本高
- 选择：`notify` + `notify::recommended_watcher()`，保持跨平台一致

### 决策 2：去抖用 `tokio::time::sleep` + per-file HashMap 而非外部去抖库
- spec 要求 100ms 去抖窗口，逻辑简单：对每个文件路径记录上次事件时间，超过 100ms 未更新则触发
- 使用 `HashMap<PathBuf, Instant>` + 独立 tokio task 轮询未触发事件
- 备选方案：`debounced` crate，但增加依赖；`notify` 内置的 `debouncer` 功能（`notify-debouncer-mini`）也可用
- 选择：使用 `notify-debouncer-mini`（`notify` 生态内，专门做 debounce，100ms 常量直接配置），避免手写 per-file 时钟逻辑

### 决策 3：多订阅者使用 `tokio::sync::broadcast`
- spec 要求"每个事件对所有活跃订阅者都投递一次"
- `broadcast::channel` 天然满足 fan-out，新订阅者可通过 `sender.subscribe()` 获取 `Receiver`
- 滞后消息（订阅者处理慢时）会在 receiver 端返回 `RecvError::Lagged`，可 log warn 后继续——符合 spec 的"continue watching"精神
- 备选方案：`tokio::sync::watch`（仅保留最新值，不适合事件流）

### 决策 4：共享事件类型放 `cdt-core`
- `FileChangeEvent` / `TodoChangeEvent` 结构体放 `cdt-core`，让 `cdt-api`、`cdt-config` 等下游 crate 无需引入 `cdt-watch` 就能使用事件类型
- 符合现有"共享类型 → `cdt-core`"约定

## Risks / Trade-offs

- **`notify` 在 macOS 下的延迟**：FSEvents API 有约 1-2 秒系统延迟；测试时需用真实文件 I/O 触发，不能假设毫秒级响应。→ 集成测试使用 `tempfile` 目录，等待事件时加 200-500ms sleep。
- **broadcast channel 容量**：若订阅者处理慢，`Lagged` 错误会丢弃旧事件。→ channel capacity 设为 256，生产场景下事件频率可控，不构成风险。
- **去抖测试的时间敏感性**：单元测试中模拟"30ms 内 5 次写入"依赖系统时钟精度。→ 使用 `tokio::time::pause()` + `advance()` 在 tokio test 中注入虚拟时间，消除 flakiness。

## Migration Plan

无迁移步骤——`cdt-watch` 是空壳 crate，直接填充实现。`cdt-cli` 和其他 crate 暂不依赖，后续 `port-session-search` 和 `port-configuration-management` 再接入。
