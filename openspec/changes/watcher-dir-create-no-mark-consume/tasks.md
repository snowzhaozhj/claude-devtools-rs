## 1. cdt-watch 修复 + 单测

- [x] 1.1 改 `crates/cdt-watch/src/watcher.rs::parse_project_event` 顶层 dir-create 分支：硬编码 `project_list_changed: true`，删除 `mark_project_seen` 调用，加注释指向 spec `file-watching::Watch project directory additions` 的"dir-create followed by first jsonl"Scenario
- [x] 1.2 加单测 `parse_project_event_dir_create_does_not_consume_mark`：先 `parse_project_event(<project_dir>, false)`、再 `parse_project_event(<project_dir>/<sess>.jsonl, false)`，断言两次返回的 `project_list_changed` 都为 `true`
- [x] 1.3 加单测 `parse_project_event_dir_create_does_not_write_known_projects`：dir-create 后断言 `watcher.known_projects.lock().unwrap()` **不**包含该 project 路径
- [x] 1.4 加固既有 `parse_project_event_marks_new_top_level_project_directory`：在原断言后追加 "dir-create 后 `known_projects` 不含该 project_id"，防止未来回归

## 2. 验证 + 提交

- [x] 2.1 `cargo test -p cdt-watch -- --nocapture` 全绿
- [x] 2.2 `cargo clippy --workspace --all-targets -- -D warnings` 无 warning
- [x] 2.3 `cargo fmt --all`
- [x] 2.4 `openspec validate watcher-dir-create-no-mark-consume --strict` 通过
- [x] 2.5 `just preflight` 全绿
- [ ] 2.6 push 分支 + 开 PR + codex 二审
