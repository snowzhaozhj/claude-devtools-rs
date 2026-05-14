## Why

会话数较多的项目打开 Sidebar 时，当前前端会先拉 50 条再重拉全量，会导致同一目录在冷启动路径被重复扫描；在 `claude-devtools-rs` 这类接近 100 个会话的项目中，用户可感知为 7~8 秒才看到列表。

本 change 目标是保持“骨架列表先渲染、metadata 后续推送补齐”的既有体验，同时降低初始列表请求的重复 I/O 和目录扫描成本。

## What Changes

- 修改前端 `listAllSessions()` 的分页策略，避免 `50 + total` 的二次全量重扫，改为稳定分页累加或单次足量骨架请求。
- 优化后端会话骨架枚举路径，减少每个 `.jsonl` 文件的串行 metadata/stat 开销，并保持排序和分页语义不变。
- 增加覆盖多会话项目的行为/性能回归测试，证明列表骨架可以更早返回且不会阻塞 metadata 流式补齐。
- 不改变 Tauri command 名称、返回字段、metadata 推送事件或 HTTP 同步完整列表语义。

## Capabilities

### New Capabilities

- 无

### Modified Capabilities

- `ipc-data-api`: 明确 `list_sessions` 在 IPC 路径 SHALL 返回可分页骨架结果，客户端 SHALL 避免为了取全量列表触发重复首包扫描。
- `project-discovery`: 明确项目会话枚举 SHALL 在保持排序/分页语义下避免不必要的逐文件串行 I/O。

## Impact

- 影响代码：`ui/src/lib/api.ts`、`ui/src/components/Sidebar.svelte` 相关调用链、`crates/cdt-discover/src/project_scanner.rs`、`crates/cdt-api/src/ipc/local.rs` 及相关测试。
- API 兼容：不新增或删除 IPC command；不改变 `SessionListResponse` 字段形态。
- 依赖：不新增第三方依赖。
- 风险：分页累加必须保持排序稳定，后端扫描优化必须保留 cursor/total 语义和 metadata 后台任务触发行为。
