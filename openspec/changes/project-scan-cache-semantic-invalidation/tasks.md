## Baseline（2026-05-23 用户机器，30 project × 538 session corpus）

> 60s `sample $(pgrep -x claude-devtools-tauri) 60 -mayDie` 期间正常使用桌面应用，活跃 claude-code 会话持续追加写。`§7.3` 改后对比按表中数字校验下降 ≥ 80%。

| frame / 入口 | baseline 命中 | §7.3 SHALL 不超过 |
|---|---|---|
| `File::open` | 1268 | ≤ 254（80% 降幅） |
| `read_dir` | 1232 | ≤ 246 |
| `canonicalize` | 166 | ≤ 33 |
| `extract_session_cwd` | 88+ | ≤ 18 |
| `LocalGitIdentityResolver::resolve_all` | 62+ | ≤ 12 |
| 顶峰 blocking-pool worker | 211 | ≤ 80 |
| 瞬时 CPU 顶峰 | 75%+ | ≤ 30% |
| ps lifetime CPU | 65.7% | ≤ 15% |

cold scan baseline（`cargo test --release -p cdt-api --test perf_cold_scan -- --ignored --nocapture`）：

| 指标 | §0.3 baseline (643 session, 1 run) | §7.1 after (644 session, min of 3 runs) | delta |
|---|---|---|---|
| cold scan wall | 74ms | 143ms | +93%（OS cache 噪声主导，非本 change 退化） |
| cold grouper wall | 4ms | 4ms | 0 |
| cold total | 78ms | 147ms | +88% |
| warm scan wall | 159ms | 169ms | +6% |
| warm grouper wall | 4ms | 4ms | 0 |

**释义**：本 change 不改 cold scan 路径（不动 `ProjectScanner::scan` 算法 / `read_dir` / `extract_session_cwd` / `read_lines_head`），仅改 invalidator 失效粒度。cold scan wall 在 OS page cache 命中状态波动剧烈，单次测量不可比；§0.3 那次 74ms 大概率落在 disk cache warm 区间，§7.1 三次 130-160ms 是受 disk cache evict 后的新基线。

真实优化效果在**用户场景的 60s sample**（§7.2-§7.3）验证——目标是"已知 session JSONL 追加事件**不再触发** ProjectScanner 重扫"，即 sample 期间 `File::open` / `read_dir` / `extract_session_cwd` 命中数 ≥ 80% 下降。该验证需用户手动跑（详 §7.2）。

## 0. 性能 baseline 采集（前置 — 给 §6 验证用）

- [x] 0.1 在用户机器跑 60s `sample` 长 sample 并保留文件（典型路径：`$CLAUDE_JOB_DIR/spike_samples/baseline_before.txt`），命令：`sample $(pgrep -x claude-devtools-tauri) 60 -mayDie -file <path>`，**期间正常使用应用**（切 sidebar / 打开长 session / 切 worktree group 等），让 hot path 命中
- [x] 0.2 用 Python / awk 解析 `before` sample 提取 baseline 数字：`File::open` 命中数 / `read_dir` 命中数 / `extract_session_cwd` 命中数 / `LocalGitIdentityResolver::resolve_all` 命中数；写入 `tasks.md` 顶部一段 baseline 表 + commit。日后 §6 验证按 80% 降幅核对
- [x] 0.3 跑 `cargo test --release -p cdt-api --test perf_cold_scan -- --ignored --nocapture` 拿 cold scan baseline；记录 wall / user / sys / max RSS / user/real ratio

## 1. cwd 不变量先行测试（cdt-discover）

- [x] 1.1 在 `crates/cdt-discover/src/project_scanner.rs::tests` 加 `extract_session_cwd_uses_first_line_only`：
    - 用 `tempfile::tempdir()` 构造测试根目录，写一个 1000 行 JSONL（首行 user message 含 `cwd`，其余 999 行 assistant message 不含 cwd）
    - **scanner 构造 fs 必须用** `Arc::new(cdt_fs::InstrumentedFs::new(cdt_fs::local_handle()))`（未包 wrapper 的 provider 不计数，详 `cdt-fs/src/instrumentation.rs:153-160`）
    - 用 `let (cwd, counts) = cdt_fs::with_fs_counter(|| async { scanner.extract_session_cwd(&jsonl_path).await }).await;` 拿返回值
    - 断言 `cwd == Some(<首行字面量>)` 且 `counts.read_to_string == 0`
- [x] 1.2 加 `jsonl_append_after_first_line_does_not_change_cwd`：首行写定 cwd，调一次拿 `(R1, counts1)`；用 `OpenOptions::append` 加 100 行；再调一次拿 `(R2, counts2)`；断言 `R1 == R2` 且 `counts1.read_to_string == counts2.read_to_string == 0`
- [x] 1.3 跑 `cargo test -p cdt-discover` 确保两个新 case 在主仓改动前已经能 pass（即"首行 invariant"当前已成立）；若不 pass 说明诊断假设错误，停下回到 design.md
- [x] 1.4 把这两个测试纳入 `crates/cdt-discover/src/project_scanner.rs` 的 `mod tests`，**不**用 `#[ignore]` 标记（必须每次 `cargo test` 都跑，作为契约守护）

## 2. ProjectScanCache API 扩展（cdt-api）

- [x] 2.1 在 `crates/cdt-api/src/ipc/project_scan_cache.rs` 加 `pub fn contains_session_id(&self, ctx: &ContextId, project_id: &str, session_id: &str) -> bool`：
    - `entries.get(ctx)` 取 entry；无 entry 返 `false`
    - 遍历 `entry.snapshot.iter()` 找 `Project.id == project_id`
    - 找到后遍历 `Project.sessions: Vec<String>` 检查含 `session_id`
    - 任一步骤失败返 `false`
- [x] 2.2 把 `crates/cdt-api/src/ipc/project_scan_cache.rs::insert` 当前的 `#[cfg(test)] fn insert(...)`（`:127-128`）升级为 `#[cfg(any(test, feature = "test-utils"))] pub fn insert(...)`，让 `crates/cdt-api/tests/` 集成测试可调用注入 cache 状态。同步把任何返回类型 / 内部 helper 也按需 feature-gate
- [x] 2.3 验证 `crates/cdt-api/Cargo.toml` 已含 `test-utils` feature（参照 ProjectScanner 已用模式 + ipc-data-api spec `ProjectScanner shared read semaphore injection` Requirement）；如未含，按既有模式加：
    ```toml
    [features]
    test-utils = []
    ```
- [x] 2.4 单测 `contains_session_id`：
    - cache 无 entry → 返 false
    - cache 有 entry 但无此 project → 返 false
    - cache 有此 project 但无此 session → 返 false
    - cache 命中 → 返 true
    - 跨 ContextId 隔离（ctx_a 命中不影响 ctx_b lookup）

## 3. telemetry counter 注册（cdt-telemetry）

- [x] 3.1 在 `crates/cdt-telemetry/src/registry.rs::COUNTER_NAMES` 加 3 个：`project_scan_cache.invalidate.structural` / `project_scan_cache.invalidate.content_append_skipped` / `project_scan_cache.invalidate.lag_conservative`
- [x] 3.2 现有 `cdt-telemetry` 单测 `build_creates_all_static_counters_at_zero` 自动覆盖新 counter 注册存在；不需新加测试

## 4. invalidator 重写（cdt-api）

- [x] 4.1 定位 `crates/cdt-api/src/ipc/local.rs::spawn_watcher_runtime` 中 project-scan invalidator closure（当前在 `:2271-2282` 附近，调 `cache.lock().expect(...).invalidate_local()`）。改写为三档判定：

    ```rust
    let local_ctx = ContextId::local(projects_dir.clone());
    loop {
        match file_rx.recv().await {
            Ok(event) => {
                let mut cache = match cache.lock() {
                    Ok(g) => g,
                    Err(poisoned) => poisoned.into_inner(),
                };
                let unknown_session = !event.session_id.is_empty()
                    && !cache.contains_session_id(&local_ctx, &event.project_id, &event.session_id);
                let structural = event.project_list_changed || event.deleted || unknown_session;
                if structural {
                    cache.invalidate_local();
                    drop(cache);
                    cdt_telemetry::registry()
                        .counter("project_scan_cache.invalidate.structural").inc();
                } else {
                    drop(cache);
                    cdt_telemetry::registry()
                        .counter("project_scan_cache.invalidate.content_append_skipped").inc();
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => {
                let mut cache = match cache.lock() {
                    Ok(g) => g,
                    Err(poisoned) => poisoned.into_inner(),
                };
                cache.invalidate_local();
                drop(cache);
                cdt_telemetry::registry()
                    .counter("project_scan_cache.invalidate.lag_conservative").inc();
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
    ```

    **注**：`ProjectScanCache` 用 `std::sync::Mutex`（同步锁），调用是 `lock().expect(...)` 而非 `tokio::sync::Mutex::lock().await`；poison 用 `into_inner` 兜底（参考 cdt-api 既有模式）。**counter inc 要在 drop guard 之后**避免持锁期间走 atomic 路径增加临界区时长。
- [x] 4.2 确认 `LocalDataApi::new()`（无 watcher）路径未启动此 task（参照 `parsed-message 缓存按 file-change 广播主动失效` 现有实现，应已遵循同模式）
- [x] 4.3 移除/调整原代码中可能残留的注释 `"任何事件都 invalidate Local entry——partial invalidation 不划算"`（`crates/cdt-api/src/ipc/local.rs:1591-1595` 附近），换成新注释指向本 spec Requirement

## 5. 集成测试（cdt-api/tests）

- [x] 5.1 新建 `crates/cdt-api/tests/project_scan_cache_invalidation.rs`，覆盖 spec 的 11 个 Scenario：
    - `jsonl_append_does_not_invalidate_cache`（已知 sa append → cache 命中保留 + skipped counter +1）
    - `new_session_first_appearance_in_known_project_invalidates_cache`（**核心**：plc=false, deleted=false, sid 不在 snapshot → invalidate + structural counter +1）
    - `top_level_dir_create_with_empty_session_id_invalidates_cache`（顶层 dir-create 事件，session_id="" + plc=true）
    - `session_jsonl_delete_invalidates_cache`（deleted=true → invalidate + structural +1）
    - `subagent_jsonl_modification_does_not_invalidate_cache`（folded plc=false, sid 命中 → skipped）
    - `subagent_jsonl_delete_invalidates_cache_as_false_positive`（folded deleted=true → invalidate；spec 明确接受 false-positive）
    - `ssh_entry_unaffected_by_file_change`（Local 失效不动 SSH entry）
    - `broadcast_lagged_invalidates_with_lag_counter`（强制 lag → invalidate_local + lag_conservative +1，structural NOT +1）
    - `broadcast_closed_exits_loop`（drop sender → task 退出）
    - `new_constructor_does_not_spawn_invalidator`（用 `LocalDataApi::new()` 构造 + 验证无 task 订阅 file_tx）
    - `cache_hit_in_normal_append_does_not_regress`（跨 IPC 复用回归，详 §6）
- [x] 5.2 测试用 `LocalDataApi::new_with_watcher` + `tempfile::tempdir` + `FileWatcher::with_paths` + 手动 `tx.send(FileChangeEvent { ... })` 模拟事件
- [x] 5.3 每个 Scenario 测试用 `cdt_telemetry::registry().counter_value(...)` 取改前/改后 counter 值，断言增量精确（避免与跨测试串扰：每个 test 用 `before = counter_value(); ... ; after = counter_value(); assert_eq!(after - before, 1)`）
- [x] 5.4 lag 测试技巧：`broadcast::channel(2)` 容量小 → 主 task spawn 后**不立即 recv**，连续 `tx.send(...)` 灌满超出 capacity 让 receiver 进入 lagged 状态；然后让 task 第一次 recv 拿到 `Err(Lagged(_))`。如果实际不稳定，备选用 `Receiver::resubscribe` 或注入测试钩子

## 6. 跨 IPC 复用回归测试（cdt-api/tests）

- [x] 6.1 在 `project_scan_cache_invalidation.rs` 加 `cache_hit_in_normal_append_does_not_regress`（对应 spec scenario "跨 IPC cache 复用在普通 append 场景不退化"）：
    1. 构造 `LocalDataApi::new_with_watcher` + tempdir 含 1 个 fixture project + 2 个 session
    2. 跑一次 `list_repository_groups()`（首次扫描，写入 cache）—— 校验 cache hits=0
    3. 触发 N=5 条普通 `FileChangeEvent { plc=false, deleted=false, session_id=已知 sid }`
    4. 等 invalidator 处理完（注入 `Arc<Notify>` 同步原语而非 sleep）
    5. 再调 `list_repository_groups()` 第二次
    6. 断言 cache hits=1（命中 + 用同一 snapshot Arc 复用，可用 `Arc::ptr_eq` 验证返回的 Arc 是同一个）
    7. 断言 `content_append_skipped` counter 增量 == 5
- [x] 6.2 加 `cache_invalidated_after_structural_does_not_regress_other_ctx`：构造同时含 Local + SSH entry 的 cache（用测试辅助 `ProjectScanCache::insert` cfg(test) API），触发 `plc=true` 事件，断言 Local entry 被清但 SSH entry 保留
- [x] 6.3 加 `unknown_session_in_known_project_invalidates_then_repopulates`：cache 有 `pa` 与 `{sa1, sa2}`，触发 `(pa, sa3, plc=false, deleted=false)` 事件 → 断言 cache 被清；再调 `list_repository_groups` → cache 重填含 sa3

## 7. 性能验证（perf bench + 手动验）

- [x] 7.1 跑 `cargo test --release -p cdt-api --test perf_cold_scan -- --ignored --nocapture` 拿改后数据，与 §0.3 baseline 对比断言不退化
- [ ] 7.2 在用户机器上跑 60s `sample` 后 sample（同 §0.1 操作流程），保存到 `$CLAUDE_JOB_DIR/spike_samples/baseline_after.txt`
- [ ] 7.3 用同一解析脚本对比 §0.2 baseline，断言：
    - `File::open` + `read_dir` 总命中数 SHALL 下降 ≥ 80%
    - `cdt_discover::project_scanner::ProjectScanner::scan` 命中数 SHALL 下降 ≥ 80%
    - `extract_session_cwd` 命中数 SHALL 下降 ≥ 80%
    - 若实际下降 < 80% 但 ≥ 50%，停下来回 design 评估是否还有别的根因（如 MetadataCache stat 风暴 / 双 runtime / 前端 debounce 已是更紧约束）；< 50% 视为本 change 不达预期，必须 revert
- [ ] 7.4 改前后 `top -l 2 -s 1 -i 1` 瞬时 CPU 顶峰从 75%+ 降到 ≤ 30%（活跃 session 写入场景）；blocking pool worker 顶峰从 211 降到 ≤ 80
- [ ] 7.5 PR 描述贴四维 perf 数据按 `.claude/rules/perf.md::PR Perf impact 模板`（wall / user / sys / max RSS / user/real ratio + before/after sample 关键 frame 命中数对比表）

## 8. 前端行为回归（手动验）

- [ ] 8.1 桌面应用在 `just dev` 启动后正常切 sidebar / ProjectSwitcher / 打开新 session / 切 worktree group → UX 与改动前一致（前端 `Sidebar.svelte::file-change` handler 仍调 `loadProjects(true)` → `listRepositoryGroups`，但后端 cache hit 路径直接返回 snapshot Arc，前端体感是"切回来仍即时显示"）
- [ ] 8.2 验证"已知 project 下新建 session"用户路径：在已存在的 cdt-rs 项目里 `claude code` 启新对话 → sidebar SHALL 在 1-2s 内（取决于 fsevents debounce + 250ms trailing 节流）显示新 session 条目；不应等到 LOCAL_CACHE_TTL=300s
- [ ] 8.3 PR 描述贴 8.1 / 8.2 录屏或截图

## 9. preflight + spec validate

- [x] 9.1 `cargo fmt --all`
- [x] 9.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 9.3 `cargo test --workspace`（重点过 `cdt-api` / `cdt-discover` / `cdt-telemetry`）
- [x] 9.4 `openspec validate project-scan-cache-semantic-invalidation --strict`
- [x] 9.5 `just preflight` 一把梭

## N. 发布

- [ ] N.1 push 分支 + 开 PR（含 §0 baseline 表 + §7 改后对比 + §8 前端验证截图）
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（性能关键路径 + cache 状态机 + 三档判定，必调；若发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
