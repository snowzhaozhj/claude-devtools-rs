## 1. 后端：提取共享下载逻辑

- [x] 1.1 在 `cdt-cli/src/` 新建 `install.rs` 模块，将 `update.rs` 中的 `platform_asset_name`、`download_and_extract`、`extract_tar_gz`、`extract_zip`、`validate_binary_magic`、`build_client` 提取为 pub 函数（`update.rs` 内部调用改为引用 `install.rs`）
- [x] 1.2 在 `cdt-cli/src/lib.rs` pub re-export `install` 模块
- [x] 1.3 `src-tauri/Cargo.toml` 添加 `cdt-cli = { path = "../crates/cdt-cli" }` 依赖
- [x] 1.4 验证：`cargo check -p cdt-cli` 和 `cargo check --manifest-path src-tauri/Cargo.toml` 通过，评估新增依赖数量

## 2. 后端：IPC command 实现

- [x] 2.1 实现 `get_cli_status` Tauri command：固定路径列表探测（`~/.local/bin/cdt`、`/usr/local/bin/cdt`、`/opt/homebrew/bin/cdt`、`~/.cargo/bin/cdt`）→ 绝对路径 `<path> --version`（3s timeout）→ login shell fallback → 返回 `CliStatus { status: String, version: Option<String>, path: Option<String>, managed: bool }`
- [x] 2.2 实现 `install_cli` Tauri command：下载（60s 总超时）→ 解压到临时文件 → chmod → xattr（macOS）→ 临时文件绝对路径验证 `<tmp> --version` → 验证通过后 atomic rename → 返回结果；验证失败删临时文件不动原 binary
- [x] 2.3 在 `src-tauri/src/lib.rs` 的 `invoke_handler!` 注册两个 command
- [x] 2.4 在 `src-tauri/capabilities/default.json` 添加两个 command 的 permission（N/A: Tauri 2 invoke_handler commands 自动可用）
- [x] 2.5 编写 IPC contract test 验证返回值 camelCase 字段名

## 3. 后端：启动时异步检测

- [x] 3.1 在 Tauri app setup 阶段 `tokio::spawn` 异步调用 `detect_cli_status`，结果存入 `CliStatusCache(tokio::sync::Mutex<Option<CliStatus>>)` 作为 app state
- [x] 3.2 `get_cli_status` IPC command 读取缓存状态（如果已检测完毕）或现场执行（Settings 打开时缓存未就绪的 fallback）

## 4. 前端：Settings CLI Section

- [x] 4.1 在 `SettingsView.svelte` 的 `sections` 数组中添加 `{ id: "cli", label: "CLI", description: "命令行工具", icon: TERMINAL_SVG }`，位于 `keyboard` 之后 `diagnostics` 之前
- [x] 4.2 添加 `TERMINAL_SVG` 图标常量到 `ui/src/lib/icons.ts`（复用已有 TERMINAL）
- [x] 4.3 扩展 `SectionId` 类型包含 `"cli"`
- [x] 4.4 实现 CLI section 渲染逻辑：5 种状态对应不同 UI（使用 SettingsGroup + SettingsField + SettingsButton）
- [x] 4.5 实现安装/更新按钮交互：disabled + spinner + 文案切换 + 成功后刷新状态 + 失败显示错误
- [x] 4.6 PATH 指令显示：mono 字体 + 复制按钮（复用 Browser Access section 的 copyFeedback 模式）

## 5. 前端：启动时检测集成

- [x] 5.1 在 `App.svelte` 或 `SettingsView` onMount 中调用 `get_cli_status` IPC（异步，不阻塞）
- [x] 5.2 mockIPC 添加 `get_cli_status` 和 `install_cli` mock handler（`?mock=1` 浏览器调试可用）

## 6. 测试

- [x] 6.1 Rust 单测：IPC contract test 覆盖字段名 + command 列表一致性（detect 逻辑强依赖 Tauri runtime，由集成测试覆盖）
- [x] 6.2 前端 vitest：mockIPC handler 覆盖 + 68 test files 全绿
- [x] 6.3 `pnpm --dir ui run check` 通过

## 7. 发布

- [ ] 7.1 push 分支 + 开 PR
- [ ] 7.2 wait-ci 全绿
- [ ] 7.3 codex 二审通过
- [ ] 7.4 archive change
