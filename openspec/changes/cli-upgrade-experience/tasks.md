## 1. 架构头校验（install.rs）

- [x] 1.1 在 `crates/cdt-cli/src/install.rs` 新增 `validate_binary_arch(data: &[u8]) -> Result<()>`，读 Mach-O cputype / ELF e_machine / PE Machine / Fat Mach-O arch list，验证包含当前运行架构
- [x] 1.2 `validate_binary_magic()` 错误消息移除 hex bytes，改为 "downloaded file is not a valid executable for this platform"
- [x] 1.3 为 `validate_binary_arch` 添加单测：各平台正确 header 通过、错误架构拒绝、truncated data 拒绝

## 2. 桌面端安装去掉执行验证（lib.rs）

- [x] 2.1 `src-tauri/src/lib.rs::install_cli()` 移除 spawn 临时二进制 `--version` 的验证步骤，替换为 `cdt_cli::install::validate_binary_arch()`
- [x] 2.2 `install_cli()` 对 `validate_binary_magic` / `validate_binary_arch` 错误做中文包装（不含 hex bytes）

## 3. self-update 路径检测重写（update.rs）

- [x] 3.1 移除 `check_install_path()` 的 `managed_indicators` 硬编码路径黑名单
- [x] 3.2 保留写权限探测逻辑，失败时给出 `sudo` 或 managed path 引导
- [x] 3.3 新增 `managed_install_path() -> Option<PathBuf>` 检测 `~/.local/bin/cdt` 是否存在
- [x] 3.4 当 current_exe 不在 managed path 且 managed path 存在时，输出 warn + 引导（非阻塞，仍允许继续更新）
- [x] 3.5 为新路径检测逻辑添加单测

## 4. 错误信息统一（update.rs + install.rs）

- [x] 4.1 `update.rs::check_install_path()` bail 消息移除 `raw.githubusercontent.com` URL
- [x] 4.2 `update.rs::friendly_error()` Forbidden 分支移除 "private repo" 提示
- [x] 4.3 验证所有 `bail!` / `Err(...)` 路径不含 raw URL（添加测试覆盖新消息）

## 5. spec delta 验证

- [x] 5.1 `cargo test -p cdt-cli` 全量通过
- [x] 5.2 `cargo clippy --workspace --all-targets -- -D warnings` 通过
- [x] 5.3 `openspec validate cli-upgrade-experience --strict` 通过

## 6. 发布

- [ ] 6.1 push 分支 + 开 PR
- [ ] 6.2 wait-ci 全绿
- [ ] 6.3 codex + pr-review-toolkit 二审通过
- [ ] 6.4 archive change
