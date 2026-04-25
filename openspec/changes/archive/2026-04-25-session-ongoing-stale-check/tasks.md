# tasks: session-ongoing-stale-check

## 1. 后端实现

- [x] 1.1 `cdt-api::ipc::session_metadata` 加 `STALE_SESSION_THRESHOLD: Duration = 5 min`、`is_session_stale(file_modified, now)` 纯函数与 `is_file_stale(path)` async wrapper
- [x] 1.2 `extract_session_metadata` 在 `messages_ongoing == true` 时再叠加 `!is_file_stale(path)`；`messages_ongoing == false` 时直接 `is_ongoing = false`
- [x] 1.3 `LocalDataApi::get_session_detail` 同步路径同样补 stale check（用 `jsonl_path`）

## 2. 测试

- [x] 2.1 `cdt-api::ipc::session_metadata::tests` 5 个纯函数 case：fresh / 4m59s / 5m exact / 7d / 时钟回拨（future mtime）
- [x] 2.2 现有 `session_metadata_stream.rs` 集成测试不破坏（fixture 文件 mtime=now，自动通过 stale check）

## 3. spec delta

- [x] 3.1 `openspec/changes/session-ongoing-stale-check/specs/ipc-data-api/spec.md` MODIFIED `Expose project and session queries`（加 stale 阈值子句 + 一个 Scenario）

## 4. validate / archive

- [x] 4.1 `openspec validate session-ongoing-stale-check --strict` 通过
- [ ] 4.2 PR 合并后 `openspec archive session-ongoing-stale-check -y`（用户操作）
