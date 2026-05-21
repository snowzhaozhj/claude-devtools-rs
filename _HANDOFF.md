# Handoff — simplify-repository-as-project

> 该文件**不要 commit**（临时 handoff），完成发版后删除。
> 上次 session 主线断点：5 reviewer 全部跑完，4 NO-GO / 1 GO，**未 commit 未 push**。

## Worktree + 分支

- worktree: `/Users/zhaohejie/RustroverProjects/Project/claude-devtools-rs/.claude/worktrees/simplify-repo-as-project`
- branch: `worktree-simplify-repo-as-project`
- 起点：origin/main commit `6aeb25f`

进入：`cd /Users/zhaohejie/RustroverProjects/Project/claude-devtools-rs/.claude/worktrees/simplify-repo-as-project`

或者下次 session 直接 `Skill: claude-code` 后用 `EnterWorktree(path: ...)`。

## 已完成范围

| 阶段 | 状态 |
|---|---|
| Preflight | ✅ |
| Propose (proposal + design + 3 spec delta + tasks) | ✅ `openspec validate --strict` 通过 |
| codex pre-propose 方向二审（agentId `a0c0f3d9f03eb6e8d`）| ✅ 方向 B 可走，4 项落地 |
| codex post-propose design 二审（agentId `a172c69fc6d3960fb`）| ✅ 5 阻塞全修 |
| **Apply Rust 侧**（task 1-5, 7）| ✅ |
| **Apply UI 侧**（task 6）| ✅ svelte-check 0/0 + vitest 351/0 |
| cargo test --workspace | ✅ 0 failed（21 crate） |
| cargo clippy --workspace --all-targets -- -D warnings | ✅ |
| cargo fmt --all | ✅ |
| **5 reviewer 并行审查** | ✅ **见下方问题清单** |
| commit | ❌ 未执行 |
| push + PR | ❌ 未执行 |
| codex PR 二审 | ❌ 未执行 |
| wait-ci 全绿 | ❌ 未执行 |
| archive | ❌ 未执行 |

## 关键文件改动（git diff --stat）

```
19 files changed, ~1216 insertions(+), ~396 deletions(-)
```

新文件：
- `openspec/changes/simplify-repository-as-project/{proposal,design,tasks}.md`
- `openspec/changes/simplify-repository-as-project/specs/{project-discovery,ipc-data-api,sidebar-navigation}/spec.md`
- `ui/src/lib/groupCursor.ts`

修改文件清单见 `git status --short`。

## 5 reviewer 反馈（必须按顺序修完才能 commit）

### A. spec-fidelity-reviewer：**NO-GO**（最重）

**核心缺口**：行为契约最重的两块（k-way merge IPC + worktree filter UI）**0 测试覆盖**。

必修：
1. **k-way merge IPC 9 个 Scenario 0 测**：补 `crates/cdt-api/tests/list_group_sessions.rs` 集成测，按 spec ipc-data-api §"Expose group session listing via k-way merge pagination" 9 个 Scenario 逐条（首页 / 续页 / next_cursor=null / 同 mtime sid 稳序 / 续页边界 off-by-one / Exhausted filter / 不全量 / pageSize=0 拒绝 / 损坏 base64 fallback）
2. **Session.cwd 暴露 4 Scenario 0 测**：`crates/cdt-discover/tests/project_scanner.rs` 加 4 测
3. **worktree filter UI 0 测**：补 `ui/src/lib/groupCursor.test.ts` + `ui/src/components/Sidebar.test.svelte.ts` 加 filter 切换 / 切 group 重置 / 自动补页用例；新 e2e `ui/tests/e2e/worktree-filter.spec.ts`
4. **行尾 cwd 全路径未删除（spec 强制 REMOVED 但代码还在）**：删 `ui/src/components/Sidebar.svelte:661/932-934/1470` 的 `cwdTailLabel` + `.session-cwd`，加 cwdRelativeToRepoRoot chip + 对应 vitest
5. **stale e2e 阻塞 CI**：`ui/tests/e2e/sidebar-grouped.spec.ts:25/38/63` 仍反向断言旧 accordion 存在——CI 必红。改写为"无 accordion + 单行"断言
6. **SSE groupId 过滤断言**：`crates/cdt-api/tests/sse_event_bridge.rs:231` 仅 destructure 未断言 `group_id`，改 `assert_eq!(group_id.as_deref(), Some("g1"))`；UI 侧补 Sidebar SSE patch 按 groupId 过滤的 Vitest
7. **生产代码强制 new_with_semaphore Scenario 违反**：`crates/cdt-cli/src/main.rs:56` + `crates/cdt-api/examples/verify_session.rs:15` 仍调 `ProjectScanner::new(`。改为 `new_with_semaphore` 或显式 cfg(test) 包；加 build-time grep 测断言
8. **SessionSummary join 缓存刷新 / fallback / list_sessions 含字段** 在 `ipc_contract.rs` 加 3 测

### B. rust-conventions-reviewer：0 blockers / 6 advisories（不阻塞但应处理）

1. **`crates/cdt-api/src/ipc/types.rs:38-49`** `WorktreeOffset` serde `tag = "kind", rename_all = "camelCase"`——全仓 enum 都是 snake_case tag。改 `rename_all = "snake_case", rename_all_fields = "camelCase"`
2. **`crates/cdt-api/src/ipc/types.rs:54-57`** `GroupCursor` 缺 `#[serde(rename_all = "camelCase")]`，`per_worktree` 序列化成 snake_case
3. **`crates/cdt-api/src/ipc/local.rs:592`** `try_join_all` 短路：group 任一 worktree scan 失败 → 整页 500；与 scanner 自身降级语义不一致。改 `join_all` + per-result warn + skip
4. **`crates/cdt-discover/src/worktree_grouper.rs:464-481`** `compute_cwd_relative_to_repo_root` 对 `project_path` 未 canonicalize；macOS `/var` vs `/private/var` symlink 场景 `strip_prefix` 失败 → 字段恒 None。helper 内对 `project_path` 也尝试 canonicalize
5. **`crates/cdt-discover/src/project_scanner.rs:257`** 注释与实现漂移："最新 mtime 的 cwd" 实际是 "按 mtime 倒序找首个非 None"
6. **`crates/cdt-core/src/project.rs:94`** `Worktree.is_repo_root: bool`（非 Option），偏好 `Option<T>` 的 crates/CLAUDE.md 指导。记录为信息

### C. tauri-config-reviewer：**GO**

4 处链路（invoke_handler / EXPECTED_TAURI_COMMANDS / KNOWN_TAURI_COMMANDS / api.ts client）已对齐，无需新 capability。无问题。

### D. ui-reviewer：**NO-GO**

必修：
1. **原生 `<select>` 违反 `ui/CLAUDE.md` 硬约束**：`ui/src/components/Sidebar.svelte:767-776` worktree filter 用了原生 `<select>`，规范要求用 `lib/components/Dropdown.svelte`（size="sm"）—— macOS WKWebView 原生弹层遮盖当前选值
2. **缺 CSS**：`.worktree-filter-bar` / `.worktree-filter-select` 在 `<style>` 无规则
3. **loadSessions 双触发**：切 group 时 line 528 直接调 `loadSessions(selectedGroupId)`，同时 line 552-556 reset `worktreeFilter` 触发 line 539-543 再调一次。同一 group 首次加载发两次 IPC

建议：`ProjectSwitcher.svelte:177/239` rgba 硬编码换 CSS 变量；line 1500 rgba fallback 冗余

### E. windows-compat-reviewer：**NO-GO**

必修（功能性 Windows 回归）：
1. **`crates/cdt-discover/src/worktree_grouper.rs:133-135`** `tokio::fs::canonicalize` 在 Windows 返 `\\?\C:\...` UNC 前缀，`compute_cwd_relative_to_repo_root` 的 `strip_prefix` 永远失败 → Windows 上 cwd_relative_to_repo_root 整体为 None
   - **修法**：用 `dunce::canonicalize`（跨平台剥 UNC，需 `cargo add dunce` 到 cdt-discover）+ `spawn_blocking` 包成 async
2. **`crates/cdt-discover/src/worktree_grouper.rs:475-480`** `relative.to_string_lossy()` Windows 上产 `crates\subdir` 反斜杠 → IPC payload 跨平台分叉。加 `.replace('\\', "/")`
3. **`crates/cdt-discover/src/worktree_grouper.rs:1097`** 测试 `Some(".claude/worktrees/feat-x")` 硬编码 `/` —— Windows CI 挂。若采纳第 2 修法则自然消解

## subagent 上下文（如需续审）

- spec-fidelity: agentId `a756f16da6ce3d24b`
- rust-conventions: agentId `a9943d52682e30e4c`
- tauri-config: agentId `ab29a40ab2cdc5f7b`
- ui-reviewer: agentId `a9ca1c04c476b5255`
- windows-compat: agentId `a0c141128aa3d2fbf`
- task 6 UI subagent: `a39b144441d335678` / `a0edb7965df2f756d`（断线 2 次）
- codex pre-propose: `a0c0f3d9f03eb6e8d`
- codex post-propose: `a172c69fc6d3960fb`

主 session 用 `SendMessage(to: "<agentId>", message: ...)` 续审。

## 下一步建议执行顺序

1. **修 windows-compat blocker**（dunce + 反斜杠归一）——加新依赖要早，让 lock 同步
2. **修 spec-fidelity 第 4/5 项**（删 Sidebar.svelte 残留 cwdTailLabel + 改写 stale e2e）——这两个会让 CI 红
3. **修 spec-fidelity 第 7 项**（cdt-cli / examples 改 new_with_semaphore）——构造点违反 spec
4. **修 ui-reviewer 第 1/2/3 项**（原生 select 替换 + 双触发去重）——UX 影响
5. **修 rust-conventions 1/2/3**（serde tag / join_all 容错）—— 契约 + 健壮性
6. **补 spec-fidelity 关键测试** 1/2/3/6/8 ——契约级测试
7. **跑全套**：`just preflight`（fmt + lint + test + spec-validate 一把梭）
8. **commit** —— commit message 模板：
   ```
   feat(project-discovery): simplify repository as project + k-way merge group sessions

   - is_repo_root + cwd_relative_to_repo_root 字段（D1/D2 scheme c）
   - list_group_sessions IPC k-way merge cursor 分页（D3）
   - ProjectScanner 共享 read_semaphore（D4）
   - ProjectSwitcher 单层 group 入口 + worktree filter 下拉 server-side cursor（D5/D6）
   - selectedGroupId 分层维护 + SSE groupId filter（D7）
   - session 行 branch + cwd hint chip（D8）

   change `simplify-repository-as-project`（含 design + 3 spec delta + 5 reviewer 反馈修复）

   🤖 Generated with Claude Code
   ```
9. **push + PR + codex PR 二审 + wait-ci 并行**（按 `.claude/rules/opsx-apply-cadence.md` N.1-N.3）
10. **archive** 原子 commit + 再次 wait-ci → 完成 N.4

## 性能验证（push 前 SHALL 跑一次）

```bash
cargo test -p cdt-api --release --test perf_cold_scan -- --ignored --nocapture
cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture
```

baseline：list_repository_groups 95ms / user-real=0.13 / RSS 59MB。回归 > 5% 即拒。

## 验证命令速查

```bash
# 在 worktree 内
cd /Users/zhaohejie/RustroverProjects/Project/claude-devtools-rs/.claude/worktrees/simplify-repo-as-project

# Rust
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

# UI
pnpm --dir ui run check
pnpm --dir ui run test:unit
just test-e2e  # 视环境

# Spec
openspec validate simplify-repository-as-project --strict

# 一把梭
just preflight
```

## 状态总结（一行）

**Rust + UI 主体改完编译/测试全绿，5 reviewer 已发现 6 个 blocker + 8 个待补测试 + 6 个 advisory；按上面执行顺序修完即可 commit + 走 N.1-N.4 发布尾段**。
