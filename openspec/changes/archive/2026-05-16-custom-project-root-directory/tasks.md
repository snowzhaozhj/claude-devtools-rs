## 1. `cdt-discover` 路径与扫描

- [x] 1.1 在 `path_decoder` 增加从 `general.claudeRootPath` 派生 Claude root、projects root、todos root 的 helper，保留默认 fallback 行为
- [x] 1.2 调整 `ProjectScanner` / project refresh 调用链，确保自定义 Claude root 时只扫描 `<root>/projects`
- [x] 1.3 调整 `SessionSearcher`，避免内部直接调用默认 projects root，全局/项目搜索使用注入的当前 projects root
- [x] 1.4 为默认 root、自定义 root、清空恢复默认、search 使用自定义 root 增加 Rust 测试

## 2. `cdt-config` 配置与 CLAUDE.md 读取

- [x] 2.1 调整 `claude_root_path` 校验：非绝对路径拒绝并保持旧值，`null` 或空白字符串归一为 `None`
- [x] 2.2 确保 `ConfigManager::update_general` 持久化 `claudeRootPath`，并补 IPC/config round-trip 测试
- [x] 2.3 调整 `read_all_claude_md_files` / auto-memory 路径计算，使 `user`、`user-rules`、`auto-memory` 作用域使用当前 Claude root
- [x] 2.4 为自定义 Claude root 的 CLAUDE.md / auto-memory 读取增加测试

## 3. `cdt-watch` 与 Tauri 运行时重配

- [x] 3.1 确保 `FileWatcher` 支持显式 projects/todos root，并在启动时使用当前 Claude root
- [x] 3.2 调整 `src-tauri` 启动逻辑：读取配置后用当前 Claude root 构建 scanner/API/searcher/watcher/notifier 上下文
- [x] 3.3 调整 `update_config("general", { claudeRootPath })` 后的运行时重配逻辑，使后续 IPC 和 watcher 使用新 root
- [x] 3.4 为 root 更新后的 project list/search/watcher 行为增加后端集成测试或 IPC contract 覆盖

## 4. Settings UI

- [x] 4.1 同步 `ui/src/lib/api.ts` 的 `GeneralConfig` 类型与 fixtures/mockIPC，暴露 `claudeRootPath: string | null`
- [x] 4.2 在 Settings General section 增加 Claude root 输入/恢复默认控件，按 Settings 乐观更新模式保存与回滚
- [x] 4.3 增加 UI 单测或 Playwright 覆盖：展示默认、自定义保存、清空恢复默认、相对路径失败回滚
- [x] 4.4 手动用 `just dev` 或 mock 浏览器验证 Settings 交互黄金路径与失败路径

## 5. 验证

- [x] 5.1 运行 `cargo fmt --all`
- [x] 5.2 运行 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 5.3 运行相关 Rust 测试（至少 `cdt-discover`、`cdt-config`、`cdt-api`）
- [x] 5.4 运行 `npm run check --prefix ui` 与相关 UI 测试
- [x] 5.5 运行 `openspec validate custom-project-root-directory --strict`

## N. 发布

- [x] N.1 push 分支 + 开 PR
- [x] N.2 wait-ci 全绿
- [x] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
