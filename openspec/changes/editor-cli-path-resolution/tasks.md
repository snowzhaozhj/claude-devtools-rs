## 1. PATH 解析模块

- [x] 1.1 新建 `crates/cdt-api/src/ipc/path_resolve.rs`：`resolve_program` / `resolve_in` / `merge_paths` / `augmented_path`(OnceCell) / `login_shell_path`(unix, sentinel + 2s timeout) / `well_known_dirs`
- [x] 1.2 `crates/cdt-api/src/ipc/mod.rs` 注册 `mod path_resolve`
- [x] 1.3 `which` 加入 `[workspace.dependencies]`，cdt-api 用 `which = { workspace = true }`
- [x] 1.4 well-known home 目录用 `cdt_discover::home_dir()`（Windows 兼容硬约束）

## 2. 接入 external_app

- [x] 2.1 `build_editor_command` async 化，4 个编辑器 CLI（code/cursor/zed/subl）spawn 前 `resolve_program`
- [x] 2.2 `build_terminal_command` async 化，Linux emulator 分支 `resolve_program`；macOS/Windows 分支不动
- [x] 2.3 `goto_command` / `path_with_loc_command` 首参 `&str` → `OsString`
- [x] 2.4 `open_in_editor` / `open_in_terminal` 调用处加 `.await`

## 3. 测试

- [x] 3.1 `merge_paths` 保序去重单测
- [x] 3.2 `resolve_in` 命中（temp 可执行文件）+ 未命中（回退 bare name）单测
- [x] 3.3 `login_shell_path` 烟雾测试（CI 无 SHELL 容错）
- [x] 3.4 `build_*` 测试改 `#[tokio::test]`，Linux/editor 程序断言改 `ends_with`
- [x] 3.5 `cargo clippy -p cdt-api --all-targets -- -D warnings` + `cargo test -p cdt-api` 全绿

## 4. 验证

- [x] 4.1 真机验证：GUI 精简 PATH 下 `which zed` 复现 not found；login-shell 解析 + well-known 目录均能命中 `/usr/local/bin/zed`
- [ ] 4.2 桌面端 smoke：release 构建 + 从 Finder 启动 → 右键「在编辑器打开」实际开 zed（需 release 构建，由 dev 跑）

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex + pr-review-toolkit 二审通过（如发现 bug：修 → push → 回到 N.2 重跑）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
