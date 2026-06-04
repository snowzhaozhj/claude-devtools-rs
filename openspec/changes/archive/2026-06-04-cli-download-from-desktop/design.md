## Context

桌面端（Tauri 2）与 CLI (`cdt`) 共享同一 Rust workspace、同一版本号、同一 GitHub Release tag。CLI 的下载/解压/替换逻辑已在 `crates/cdt-cli/src/update.rs` 实现并经过线上验证（`platform_asset_name` / `download_and_extract` / `replace_binary` / `validate_binary_magic`）。

现有 `install.sh` 安装目标为 `~/.local/bin/cdt`（通过 `CDT_INSTALL_DIR` 可覆盖）。`cdt self-update` 就地替换当前 binary。桌面端 Settings 已有 General / Display / Notifications / Connection / Keyboard / Diagnostics / About 共 7 个 section。

## Goals / Non-Goals

**Goals:**

- 桌面端用户可以在 Settings 内一键安装/更新 CLI（与 `install.sh` 安装到相同位置）
- 启动时异步检测 CLI 状态，用户打开 Settings 时立即可见结果，无等待
- 检测和安装逻辑对应用启动/运行性能零可感知影响
- 安装路径与 `install.sh` / `cdt self-update` 统一，不产生多副本冲突

**Non-Goals:**

- 不做 CLI 自动更新（桌面端更新时不连带更新 CLI）
- 不注入用户 shell PATH
- 不在 Settings 之外的地方展示 CLI 状态（无 badge / toast / 弹窗）
- 不管理非 `~/.local/bin/cdt` 路径的 CLI 安装（外部管理的只读展示）
- 不支持 Windows 一键安装（Windows Settings 只展示状态 + 下载链接）
- 不引入新的前端组件类型（复用 SettingsGroup / SettingsField / SettingsButton）

## Decisions

### D1：共享下载逻辑的提取位置

**选择**：将 `update.rs` 中的 5 个核心函数（`platform_asset_name` / `download_and_extract` / `extract_tar_gz` / `extract_zip` / `validate_binary_magic`）+ `build_client` 提取到新内部模块 `crates/cdt-cli/src/install.rs`，通过 `cdt-cli/src/lib.rs` pub re-export。`src-tauri` 依赖 `cdt-cli`。

**替代方案**：
- 新建 `cdt-install` 独立 crate → 最干净但增加 workspace crate 数量
- 直接依赖整个 `cdt-cli` 不拆模块 → `cdt-cli` 带 clap/zip/zstd/rmcp 共 36 个独有依赖全拉进 Tauri 构建
- 复制到 `src-tauri` → 违反 DRY

**理由**：提取到 `install.rs` 子模块（不含 clap/completions 逻辑），`src-tauri` feature-gate 只编译 install 路径所需的 reqwest + flate2 + tar + zip。如果依赖膨胀实测超预期，apply 阶段可升级为独立 `cdt-install` crate。

### D2：CLI 检测策略（codex 审查后修订）

**选择**：多路径探测，**不**依赖 Tauri GUI 进程的 PATH。

检测顺序：
1. 检查固定候选路径列表：`~/.local/bin/cdt`、`/usr/local/bin/cdt`、`/opt/homebrew/bin/cdt`、`~/.cargo/bin/cdt`
2. 对每个存在的路径执行 `<absolute_path> --version`（3s timeout）
3. 取第一个成功返回版本的路径作为"已安装 CLI"
4. 如果以上全不存在，尝试通过用户 login shell 获取真实 PATH：`$SHELL -lc "which cdt"` → 拿到绝对路径后执行 `--version`

**替代方案**：
- 直接 `which cdt` → GUI 进程 PATH 不含 `~/.local/bin` / `/opt/homebrew/bin`，会误判未安装（codex CRITICAL/HIGH 指出）
- 只检查 `~/.local/bin/cdt` → 漏掉外部安装用户

**理由**：macOS Finder/Dock 启动的 app PATH 通常只有 `/usr/bin:/bin:/usr/sbin:/sbin`。直接探测固定路径列表避免了这个根本问题，且比 spawn login shell 更快更可靠。login shell fallback 作为兜底覆盖非常规安装路径。

### D3：安装目标路径

**选择**：`~/.local/bin/cdt`（macOS/Linux），Windows 不做一键安装。

**理由**：与 `install.sh` 默认路径统一。`~/.local/bin/` 不存在时创建。如果 `which cdt` 找到的路径不是 `~/.local/bin/cdt`（外部管理），Settings 只展示状态不提供更新按钮，避免覆盖包管理器文件。

### D4：安装后验证（codex 审查后修订）

**选择**：安装流程分两步验证：
1. **替换前验证**：binary 写入临时文件后，用临时文件绝对路径 spawn `<tmp_path> --version` 验证可执行且版本正确。验证失败 → 删除临时文件 + 报错，原有 binary 不受影响。
2. **替换后确认**：atomic rename 成功后，用目标绝对路径 `~/.local/bin/cdt --version` 确认。

**关键约束**（codex CRITICAL）：
- 所有验证 SHALL 使用**绝对路径**执行，不依赖 GUI 进程 PATH
- 更新场景：backup 文件保留到验证成功后才删除；验证失败时 rollback（rename backup 回原位）

**理由**：Tauri GUI 进程 PATH 不含 `~/.local/bin`，直接 `cdt --version`（无路径）会找不到刚装的 binary 并误删。先验证临时文件避免了"替换成功但验证失败导致用户丢失旧 CLI"的风险。

### D5：macOS Gatekeeper 处理

**选择**：安装后执行 `xattr -d com.apple.quarantine <path>`。

**理由**：通过 HTTP 下载的 binary 会被 macOS 标记 quarantine 属性，首次执行时 Gatekeeper 拦截弹窗。清除该属性让 CLI 可直接运行。

### D6：Settings UI 状态机（codex 审查后修订）

**选择**：6 态——`detecting` / `not_installed` / `installed_current` / `installed_outdated` / `installed_not_in_path` / `externally_managed`。

`detecting` 态：启动后异步检测尚未完成时的初始状态。Settings CLI section 显示禁用的占位文案"正在检测..."，不显示安装按钮（避免误操作）。典型持续 < 500ms，冷启动极端场景 < 2s。

颜色遵循 DESIGN.md Named Rules：
- `installed_current`：Execution Green（已完成）
- 安装/更新进行中：Focus Blue 10×10 inline spinner（secondary live signal）
- 失败：Warning Amber（actionable，非 error）
- `not_installed` / `externally_managed`：neutral `--color-text-muted`

### D7：`installed_not_in_path` 检测

**选择**：安装完成后 + 启动检测时，如果 `~/.local/bin/cdt` 存在但 `which cdt` 找不到（或找到的不是这个路径），判定为 `installed_not_in_path`，显示 PATH 添加指令。

### D8：进度反馈

**选择**：安装按钮点击后变为 disabled + inline spinner + "安装中..." 文案。不引入进度条组件。

**理由**：6MB 文件下载通常 2-5s，不值得引入 progress bar 新组件。与现有 Settings 交互一致（如 "检查更新" 按钮的 loading 态）。

### D9：下载超时与取消（codex 审查新增）

**选择**：`install_cli` IPC command 设置 reqwest 总超时 60s（connect 10s + read 30s）。超时后返回可重试错误，前端恢复按钮可点击状态。

**理由**：企业代理/弱网下 TCP 连接建立后 body 不推进，无超时会让 spinner 无限转。60s 覆盖 6MB 在 100KB/s 弱网的极端场景。

### D10：PATH 可见性判断（codex 审查新增）

**选择**：`installed_not_in_path` 判定方式为 `$SHELL -lc "which cdt"` 确认用户 login shell 是否能找到 `~/.local/bin/cdt`（而非 GUI 进程内 `which`）。如果 login shell `which` 返回的路径与 `~/.local/bin/cdt` 不同（存在外部安装 + 被 shadow），状态标记为 `externally_managed`（外部路径优先级高于受管路径），不误导用户加 PATH。

## Risks / Trade-offs

- **[GUI PATH 不一致]**（codex CRITICAL/HIGH）macOS Finder/Dock 启动的 Tauri app PATH 不含用户自定义路径 → 缓解：D2 改为固定路径列表探测 + login shell fallback，不依赖 GUI PATH
- **[更新失败丢失 CLI]**（codex CRITICAL）atomic rename 后验证失败导致用户既没新版本也没旧版本 → 缓解：D4 改为先验证临时文件再 rename，backup 保留到验证成功
- **[PATH 优先级 shadow]** 用户同时有 `/usr/local/bin/cdt` 和 `~/.local/bin/cdt` → 缓解：D10 通过 login shell which 判断实际生效路径，shadow 时标为 externally_managed
- **[依赖膨胀]** `cdt-cli` 带 36 个独有 crate → 缓解：D1 提取独立模块 `install.rs`，apply 时评估是否需要升级为独立 crate
- **[无签名校验]**（codex HIGH）magic bytes 不验证发布资产完整性 → 接受风险：当前 CLI `self-update` 也无 checksum，两条路径一致；后续统一加 SHA256 校验作为 follow-up issue
- **[权限问题]** `~/.local/bin/` 可能被 root 创建 → 缓解：安装前 probe 写权限，失败时显示明确错误
- **[下载超时]** 企业代理/弱网 → 缓解：D9 设 60s 总超时
- **[版本格式变化]** `cdt --version` 输出格式变了 → 缓解：解析失败时状态标记为"已安装（版本未知）"
- **[Windows `where` 多结果]** → 缓解：Windows 不做一键安装（Non-Goal），检测时取第一条有效路径
