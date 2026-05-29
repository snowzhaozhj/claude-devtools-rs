## 1. 合并双重文件读取（D1）

- [x] 1.1 重写 `parse_subagent_candidate`：删除第一阶段 `Value` 泛解析，改为单次 `parse_file_via_fs` + 从 `ParsedMessage` 流提取 metadata（spawn_ts / end_ts / parent_session_id / description_hint / warmup 判定）
- [x] 1.2 确保 warmup 短路逻辑正确：从 `ParsedMessage` 的前 10 条 user 消息中检测 `content == "Warmup"`

## 2. 同项目内并行化（D2）

- [x] 2.1 在 `scan_subagent_candidates_cross_project` 内层：将 `for sub_entry in sub_entries` 串行循环改为 `futures::future::join_all` + 内层 `Semaphore(4)` 并发
- [x] 2.2 确保 `per_candidate_ms` 计时和 `seen_ids` 去重逻辑在并行化后仍正确（计时放在各 task 内，去重在 collect 后）

## 3. 验证

- [x] 3.1 `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test -p cdt-api` 全绿
- [x] 3.2 用 perf bench 验证 scan_subagents_ms < 200ms 且 user/real ≤ 0.66（实测：scan 81-106ms，total 94-116ms，user/real=0.19）

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
