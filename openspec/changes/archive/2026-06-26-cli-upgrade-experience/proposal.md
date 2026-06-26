## Why

CLI 升级体验存在三个互相关联的问题：

1. **self-update 与桌面端安装冲突**：桌面端安装 CLI 到 `~/.local/bin/cdt`，但用户 PATH 中可能还有 `~/.cargo/bin/cdt`（cargo install 旧版本）。运行 `cdt self-update` 时 `check_install_path()` 硬编码检测 `/.cargo/bin/` 路径直接 bail，没有告知用户还有桌面端管理的版本可用。
2. **安装时执行临时二进制触发安全拦截**：桌面端 `install_cli()` 在 atomic rename 前 spawn `.cdt-install-{pid}.tmp --version` 验证可执行性，macOS Gatekeeper / 企业 EDR 可能拦截未签名临时文件的首次执行。业界最佳实践（mise 用 zipsign 签名、rustup 信任 CDN 不执行）趋势是用 binary header 校验替代执行验证。
3. **错误信息泄漏内部细节**：`check_install_path()` bail 消息含 `raw.githubusercontent.com` 完整 URL；`validate_binary_magic()` 失败时 hex magic bytes 直接暴露给用户；"private repo" 等误导提示；CLI 英文 vs Tauri 中文语言不一致。

## What Changes

- `check_install_path()` 从硬编码路径黑名单改为**写权限检查 + managed path 引导**：检测到无法写入时给出可操作的升级建议（包括引导到桌面端管理的 `~/.local/bin/cdt`）
- 桌面端 `install_cli()` 用 **Mach-O/ELF/PE 架构头验证**替代执行临时二进制（`cdt-cli::install` 新增 `validate_binary_arch()` 函数）
- 统一错误信息层：所有面向用户的错误不含 raw URL / hex bytes / 内部协议细节，CLI 和 Tauri 两侧风格对齐

## Capabilities

### New Capabilities

（无新增 capability）

### Modified Capabilities

- `cli-distribution`: self-update 路径检测策略变更（写权限 + managed path 引导）；安装验证方式变更（架构头校验替代执行验证）；错误信息面向用户友好化

## Impact

- `crates/cdt-cli/src/update.rs`：`check_install_path()` 重写 + `friendly_error()` 补全
- `crates/cdt-cli/src/install.rs`：新增 `validate_binary_arch()` 架构头校验函数
- `src-tauri/src/lib.rs`：`install_cli()` 去掉 spawn 临时二进制验证、改调架构头校验；`friendly_cli_install_error()` 补全
- `openspec/specs/cli-distribution/spec.md`：更新安装验证和 self-update 行为契约
