## 1. cdt-discover：新增 path_compare 模块

- [x] 1.1 新建 `crates/cdt-discover/src/path_compare.rs`，导出 `paths_equal(&Path, &Path) -> bool` / `path_starts_with(&Path, &Path) -> bool` / `normalize_path_for_compare(&Path) -> Cow<'_, Path>` / `normalize_path_string_for_compare(&str) -> Cow<'_, str>` / `path_strip_prefix(&Path, &Path) -> Option<&Path>` 五个公开函数
- [x] 1.2 Windows 分支用 `#[cfg(target_os = "windows")]` 走 ASCII lowercase；非 Windows 分支返回 `Cow::Borrowed`
- [x] 1.3 在 `crates/cdt-discover/src/lib.rs` 导出 `pub mod path_compare;` + 公开 helper
- [x] 1.4 单测：12 个跨平台分支 case，`paths_equal` / `path_starts_with` / `path_strip_prefix` / `normalize_path_string_for_compare` 在 Windows / Unix 双平台双向断言

## 2. cdt-discover：接入 ProjectPathResolver / ProjectScanner / SubprojectRegistry

- [x] 2.1 `project_path_resolver.rs::cache_get/cache_set/invalidate` 在写入与查询前都过 `normalize_path_string_for_compare(project_id)` 拿规范化 key
- [x] 2.2 `project_scanner.rs` 的 cwd `BTreeMap` key 改用 `normalize_path_string_for_compare(&cwd).into_owned()`，bucket value 内仍存原 cwd 用于展示
- [x] 2.3 `subproject_registry.rs::compose_id` 的 SHA-256 输入改为 `normalize_path_string_for_compare(&cwd).as_bytes()`
- [x] 2.4 单测：`project_path_resolver::cache_handles_case_per_platform`（Windows 命中、Unix miss）；`subproject_registry::compose_id_case_handling_matches_platform`（Windows 同 hash、Unix 异 hash）

## 3. cdt-watch：接入 FileWatcher

- [x] 3.1 `watcher.rs::route_event` 把 `path.starts_with(&self.projects_dir)` / `starts_with(&self.todos_dir)` 改用 `cdt_discover::path_starts_with`；`parse_project_event` 把 `path.strip_prefix(&self.projects_dir)` 改用 `cdt_discover::path_strip_prefix`
- [x] 3.2 `watcher.rs::known_projects` 写入与初始扫描都过 `cdt_discover::normalize_path_for_compare`，HashSet 元素在 Windows 上以规范化形式存储
- [x] 3.3 `crates/cdt-watch/Cargo.toml` 加 `cdt-discover = { workspace = true }` 依赖
- [x] 3.4 既有 26 个 watcher 单测全过；端到端 `tests/file_watching.rs` 6 case 保持 `--ignored` 标记不变

## 4. cdt-config：接入 mention 路径校验

- [x] 4.1 `mention.rs::is_path_within_allowed` 把两处 `normalized.starts_with(&claude_dir)` / `starts_with(root)` 改用 `cdt_discover::path_starts_with`
- [x] 4.2 既有 mention 单测全过（沿用 tempdir 路径同源场景）

## 5. 全量验证

- [x] 5.1 `cargo clippy --workspace --all-targets -- -D warnings`（在 `just preflight` 内）
- [x] 5.2 `cargo fmt --all`（在 `just preflight` 内）
- [x] 5.3 `cargo test --workspace`：cdt-discover / cdt-watch / cdt-config / cdt-api 全过
- [x] 5.4 `openspec validate --all --strict`：25 items 通过
- [x] 5.5 `pnpm --dir ui run check`（svelte-check 0 errors）+ `pnpm --dir ui run test:unit`（263 passed）
- [x] 5.6 更新 `openspec/followups.md::Windows 平台::[coverage-gap] Windows 路径大小写不敏感比较` 标 ✅ 已修正并指向本 change slug

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
