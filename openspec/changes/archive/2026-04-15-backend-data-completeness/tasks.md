## 1. Slash 命令提取（cdt-analyze + cdt-core）

- [x] 1.1 在 `cdt-core` 中定义 `SlashCommand` 结构体（name, message, args, message_uuid, timestamp），添加 serde camelCase 序列化
- [x] 1.2 在 `cdt-analyze/src/chunk/` 中实现 `extract_slash_info(content: &str) -> Option<SlashCommand>` 函数，从 `<command-name>/xxx</command-name>` XML 标签提取 slash 信息
- [x] 1.3 修改 `AIChunk` 添加 `slash_commands: Vec<SlashCommand>` 字段（默认空数组）
- [x] 1.4 修改 `build_chunks` 逻辑：遇到 isMeta 消息时尝试提取 slash，将提取结果附加到紧随其后的 AIChunk
- [x] 1.5 前端 `buildAiGroupSummary` 加 slash 计数：读取 `chunk.slashCommands`，在 summary 中显示 "N slash"
- [x] 1.6 前端 `api.ts` 的 `AIChunk` interface 添加 `slashCommands: SlashCommand[]` 类型定义
- [x] 1.7 `cargo test -p cdt-analyze` 验证 slash 提取和 chunk 集成
- [x] 1.8 `cargo clippy -p cdt-analyze -p cdt-core -- -D warnings` 通过

## 2. Subagent 解析集成（cdt-api）

- [x] 2.1 在 `cdt-api/src/ipc/local.rs` 中实现 subagent 候选扫描：从同 project session 列表中构建 `SubagentCandidate` 列表
- [x] 2.2 在 `get_session_detail` 中调用 `resolve_subagents`，将解析结果填充到对应 `AIChunk.subagents`
- [x] 2.3 处理边界情况：无候选时 subagents 为空数组；扫描失败时 warn 日志但不报错
- [x] 2.4 `cargo test -p cdt-api` 验证 subagent 集成
- [x] 2.5 `cargo clippy -p cdt-api -- -D warnings` 通过

## 3. Search 对接（cdt-api + src-tauri）

- [x] 3.1 修改 `LocalDataApi` 构造函数，注入 `SessionSearcher`（或在内部构造）
- [x] 3.2 实现 `search()` 方法：委托 `SessionSearcher.search_sessions()` 执行真实搜索
- [x] 3.3 处理空 query 返回空结果、无效 project_id 返回错误
- [x] 3.4 在 `src-tauri/src/lib.rs` 新增 `search_sessions` Tauri command，透传到 `LocalDataApi.search()`
- [x] 3.5 `cargo test -p cdt-api` 验证 search 集成
- [x] 3.6 `cargo clippy -p cdt-api -- -D warnings` 通过

## 4. 集成验证

- [x] 4.1 `cargo build --workspace` 全量编译通过
- [x] 4.2 `cargo test --workspace` 全量测试通过
- [x] 4.3 `cargo clippy --workspace --all-targets -- -D warnings` 通过
- [x] 4.4 `npm run check --prefix ui` 前端类型检查通过
- [x] 4.5 `cargo tauri dev` 启动验证：编译成功，应用正常启动
