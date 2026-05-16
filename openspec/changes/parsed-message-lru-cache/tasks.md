## 1. `cdt-api` — ParsedMessageCache 模块

- [x] 1.1 在 `crates/cdt-api/src/ipc/parsed_message_cache.rs` 新增 `ParsedMessageCache` struct（参照 `session_metadata.rs::MetadataCache` 模式：`HashMap<PathBuf, Entry> + VecDeque<PathBuf> + capacity` 三件套；Entry 含 `signature: FileSignature` 与 `messages: Arc<Vec<ParsedMessage>>`）
- [x] 1.2 实现 `lookup(&mut self, &Path) -> Option<Entry>`（命中 bump 到队首）、`insert(&mut self, PathBuf, Entry)`（已存在则覆盖 + bump；新条目达 capacity 时 pop_back 淘汰最久未访问）、`remove(&mut self, &Path)`（同步从 map + order 删除）
- [x] 1.3 定义 `PARSED_MESSAGE_CACHE_CAPACITY: usize = 50` 常量
- [x] 1.4 实现 `pub(crate) async fn extract_parsed_messages_cached(cache: &StdMutex<ParsedMessageCache>, path: &Path) -> Option<Arc<Vec<ParsedMessage>>>`：stat → lookup → signature 比对命中返回；miss 时调 `cdt_parse::parse_file(path)`，成功后 `Arc::new(messages)` 写入 cache 并返回 `Some`；parse 失败或 stat 失败返回 `None`（不写入 cache）
- [x] 1.5 在 `crates/cdt-api/src/ipc/mod.rs` 加 `pub(crate) mod parsed_message_cache;`

## 2. `cdt-api` — LocalDataApi 集成

- [x] 2.1 在 `LocalDataApi` struct 新增字段 `parsed_msg_cache: Arc<std::sync::Mutex<ParsedMessageCache>>`
- [x] 2.2 `LocalDataApi::new()` 与 `LocalDataApi::new_with_watcher()` 内部初始化新字段为 `Arc::new(StdMutex::new(ParsedMessageCache::default()))`；**不**改这两个构造器的参数签名
- [x] 2.3 在 `LocalDataApi::new_with_watcher` 内部额外 spawn 一个后台 task，订阅 `watcher.subscribe_files()` 广播；每条 `FileChangeEvent` 按 `projects_dir / extract_base_dir(project_id) / "{session_id}.jsonl"` 推算 path，从 cache 中 `remove(path)`；`broadcast::Receiver::recv` 返回 `Lagged` 时静默 continue，`Closed` 时退出 loop
- [x] 2.4 重构 `get_image_asset`：在 `parse_file(&jsonl_path)` 调用点改为先调 `extract_parsed_messages_cached(&self.parsed_msg_cache, &jsonl_path).await`；命中 `Some(arc)` 走原 `find_image_block_in_messages(&*arc, ...)` 流程；`None` 走原 fall-back（返回 `empty_data_uri()`）
- [x] 2.5 重构 `get_tool_output`：在 `parse_file(&jsonl_path)` 调用点改为先调 `extract_parsed_messages_cached(...)`；命中 `Some(arc)` 走 `cdt_analyze::build_chunks(&*arc)` + 线性扫；`None` 返回 `ToolOutput::Missing`

## 3. `cdt-api` — 单元测试

- [x] 3.1 在 `parsed_message_cache.rs` 末尾 `#[cfg(test)] mod tests` 加 LRU 行为测试：`parsed_cache_evicts_lru_when_over_capacity`、`parsed_cache_lookup_bumps_hit_to_front`
- [x] 3.2 加 `extract_parsed_messages_cached` 行为测试：`cached_hit_returns_arc_without_rereading`（第一次写入后第二次调返回同一 `Arc`，引用计数 ≥ 2）、`cached_miss_when_file_size_changes`（append 后重新 parse）、`cached_miss_when_inode_changes_via_rename`（仅 Unix，`std::fs::rename` 后重 parse）、`cached_stat_failure_returns_none_no_write`（不存在 path → None + cache 仍空）、`cached_parse_failure_returns_none_no_write`（写入非法 JSONL 内容 → parse_file 不返 Err 而是 skip 全部行返回空 Vec；本测试改为：写入空文件 → parse_file 返回空 Vec → 仍写入 cache 视为合法空结果；如需测真 parse Err 由 `parse_file` 行为决定，按实际行为断言）
- [x] 3.3 加 invalidate 集成测试：构造 `LocalDataApi` + 真 `FileWatcher`，先 cache 写入一条 → 触发文件 mtime/size 变化让 watcher emit → 等待 cache 中条目消失；测试用 `tokio::time::timeout` 防 flaky；放 `crates/cdt-api/tests/parsed_message_cache_invalidate.rs`
- [x] 3.4 `cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all`、`cargo test -p cdt-api`、`cargo test -p cdt-api --test ipc_contract`（确认 IPC 字段无破坏）

## 4. 性能验证

- [x] 4.1 扩展 `crates/cdt-api/tests/perf_get_session_detail.rs`：新增一个 `#[ignore]` 测试 `perf_get_tool_output_cache_hit_path`，构造大会话 → 第一次 `get_tool_output` 触发 cache miss + 计时 → 第二次 `get_tool_output(同 sid 不同 tool_use_id)` 命中 + 计时，断言第二次 < 第一次 / 4（粗略 4× 加速作为下限，避免环境 flake）
- [x] 4.2 PR 描述中贴出 cache hit 前后耗时数据（手动跑 `cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture` 取值）

## 5. spec 验证

- [x] 5.1 `openspec validate parsed-message-lru-cache --strict` 通过

## 6. 发布

- [ ] 6.1 push 分支 + 开 PR
- [ ] 6.2 wait-ci 全绿
- [ ] 6.3 codex 二审通过（如发现 bug：修 → push → 回到 6.2 重跑；可循环 M 次）
- [ ] 6.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
