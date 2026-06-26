## Context

CLI 二进制分发涉及三个安装来源（桌面端 `~/.local/bin/cdt`、cargo install `~/.cargo/bin/cdt`、install.sh `~/.local/bin/cdt`）和两个升级入口（`cdt self-update`、桌面端 Settings "更新 CLI"按钮）。当前 `check_install_path()` 用硬编码路径黑名单（`/Cellar/`、`/.cargo/bin/` 等）阻止 self-update，且桌面端安装时 spawn 临时二进制验证可执行性。

业界调研（mise / rustup / deno / gh）的最佳实践趋势：
- 包管理器检测：**写权限检查**（rustup / deno）优于硬编码路径（脆弱且无法覆盖新包管理器）
- 二进制验证：**签名 / 架构头校验**（mise 用 zipsign）优于执行验证（安全工具拦截）
- 错误信息：**可操作的引导**（deno 的 root-ownership 消息）优于技术细节堆砌

## Goals / Non-Goals

**Goals:**
- 桌面端安装的 CLI 能通过 `cdt self-update` 正常升级（`~/.local/bin/cdt` 路径）
- 从其他路径（如 `~/.cargo/bin/cdt`）运行时，给出**可操作的引导**而非裸 bail
- 消除安装过程中执行临时二进制的安全风险
- 所有面向用户的错误信息不含 raw URL / hex bytes / 内部协议细节

**Non-Goals:**
- 不实现加密签名验证（zipsign / minisign）——需要构建管线改动，留给后续
- 不改变安装目标路径（仍为 `~/.local/bin/cdt`）
- 不处理 Windows 特定路径问题（当前 Windows 无 `/.cargo/bin/` 检测逻辑）
- 不添加自动清理旧安装的功能（仅引导）

## Decisions

### D1：self-update 路径检测改为写权限检查 + managed path 引导

**候选方案**：
- A. 保留路径黑名单但放宽 `/.cargo/bin/` → 仍脆弱，新包管理器路径漏拦
- B. mise 式 marker 文件 → 需要桌面端安装时写 marker，且 cargo install 不会写
- C. **写权限检查 + managed path 感知**（选定）→ 跟 rustup/deno 一致，不依赖路径模式

**实现**：
1. 移除 `managed_indicators` 硬编码路径黑名单
2. 保留写权限探测（`fs::write` 探针文件），失败时提示 `sudo` 或引导到 managed path
3. 新增 `detect_managed_install()` 检查 `~/.local/bin/cdt` 是否存在且可写
4. 当 `current_exe()` 不在 managed path 且 managed path 存在时：warn + 引导用户切换到 managed path 的升级方式（`~/.local/bin/cdt self-update` 或桌面端更新）
5. 当 `current_exe()` 在 managed path 时：正常 self-update

**风险**：用户 cargo install 的版本不再被拦截 → 允许原地升级（但 `cargo install` 后续 `cargo update` 会覆盖回来）。接受此风险，因为用户主动执行 `self-update` 表达了明确意图。

### D2：安装验证从执行临时二进制改为架构头校验

**候选方案**：
- A. 保留执行验证 + 更好的 quarantine 处理 → 治标不治本，EDR 仍可拦截
- B. 纯 magic bytes（现有 `validate_binary_magic`）→ 已有，但不验证架构匹配
- C. **架构头校验**（选定）→ 读 Mach-O CPU type / ELF machine / PE machine，验证与当前运行架构一致

**实现**：
在 `cdt-cli::install` 新增 `validate_binary_arch(data: &[u8]) -> Result<()>`：
- Mach-O：读 `cputype` 字段（offset 4-7），匹配 `CPU_TYPE_ARM64`（0x0100000C）或 `CPU_TYPE_X86_64`（0x01000007）
- ELF：读 `e_machine` 字段（offset 18-19），匹配 `EM_AARCH64`（183）或 `EM_X86_64`（62）
- PE：读 `Machine` 字段（PE header offset + 4-5），匹配 `IMAGE_FILE_MACHINE_AMD64`（0x8664）
- Universal binary (fat Mach-O)：magic `0xCAFEBABE` / `0xBEBAFECA`，遍历 arch list 验证包含当前架构

桌面端 `install_cli()` 调用链：`validate_binary_magic()` → `validate_binary_arch()` → atomic rename。不再 spawn 临时二进制。

**风险**：失去"binary 真的能跑"的运行时验证。接受：magic + 架构匹配已覆盖 99% 的失败场景（错误平台下载），剩余极端情况（truncated binary / dynamic linker 缺依赖）用户首次运行时自然会发现。

### D3：统一错误信息层

**原则**：
- 所有面向用户的错误消息经过 friendly 层过滤
- 不含 raw URL（GitHub / raw.githubusercontent.com）
- 不含 hex bytes、内部协议细节、误导性提示（如 "private repo"）
- CLI 统一英文（用户群体是开发者，英文消息更通用且与 --help 一致）
- Tauri 统一中文（桌面端面向中文用户，与 UI 一致）

**具体改动点**：
1. `update.rs::check_install_path()` bail 消息：移除 `raw.githubusercontent.com` URL，改为引导文案
2. `install.rs::validate_binary_magic()` 错误消息：不暴露 hex bytes，改为 "downloaded file is not a valid executable for this platform"
3. `update.rs::friendly_error()` 的 `Forbidden` 分支：移除 "private repo" 提示，改为 "Access denied. Check your network or proxy settings, or set GH_TOKEN"
4. `lib.rs::friendly_cli_install_error()` 对 `validate_binary_magic` / `validate_binary_arch` 错误的包装

## Risks / Trade-offs

1. **放宽 self-update 路径检测**：cargo install 的 binary 现在可以被 self-update 覆盖。trade-off 可接受——用户主动调 `self-update` = 明确意图；且提供了 managed path 引导作为推荐方案。
2. **去掉执行验证**：极端场景（binary 完整但 dynamic linker 缺依赖）不会在安装时捕获。trade-off 可接受——这类问题极罕见且用户首次运行即发现。
3. **CLI 统一英文**：当前 `friendly_error()` 已经是英文，保持不变。可能有中文用户觉得不够友好，但 CLI 工具英文错误信息是行业惯例。
