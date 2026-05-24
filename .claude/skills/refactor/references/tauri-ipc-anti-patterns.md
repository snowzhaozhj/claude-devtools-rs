# Tauri / IPC 反模式（仓特定）

本仓 src-tauri 是 Tauri 2 + tauri-plugin-* + 自定义 IPC handlers。详细约束见 `src-tauri/CLAUDE.md`。

## 1. IPC 字段契约（指针为主，不重抄真相源）

IPC payload 瘦身模式 / IPC 字段改动 checklist 真相源在 **`src-tauri/CLAUDE.md`** + **`.claude/rules/perf.md::反模式清单`**——本 skill **不**重抄。审计时取规则，category 命名作 cross-reference：

- `tauri-ipc-payload-large`（单次 IPC payload > 1 MB；瘦身模式详 `src-tauri/CLAUDE.md::IPC payload 瘦身模式`）
- `tauri-ipc-schema-drift`（Rust 端字段改了 / 前端 mockIPC + contract test 没同步；硬约束 + 解法详 `src-tauri/CLAUDE.md::IPC 字段改动 checklist`）
- `tauri-ipc-snake-leak`（默认 snake_case 漏到前端；要求详 `crates/CLAUDE.md` serde camelCase）
- `tauri-ipc-error-stringify`（`Result<T, String>` 吞掉错误分层；解法详 `crates/CLAUDE.md` 错误类型）

`tauri-ipc-schema-drift` / `tauri-ipc-payload-large` 命中即 boundary guard 升级 → openspec。

## 2. 配置链一致性

| category | 反模式 | 期望 |
|---|---|---|
| `tauri-conf-feature-mismatch` | `Cargo.toml` features 与 `tauri.conf.json` 启用模块不一致 | 4 处真相源对齐：`tauri.conf.json` + `capabilities/default.json` + `Cargo.toml features` + `src/lib.rs::invoke_handler!` |
| `tauri-capability-missing` | 新加 IPC command 但 `capabilities/default.json` 没 allow | tauri-config-reviewer subagent 审一道 |
| `tauri-handler-not-registered` | 写了 `#[tauri::command]` 但 `invoke_handler!` 没注册 | 同上 |
| `tauri-release-feature-skip` | release feature 引入新 plugin 但 dev 没启 → 调试看不见但发版炸 | 评估是否合并到 default features |

## 3. 通知 / 托盘 / 平台 API

| category | 反模式 | 期望 |
|---|---|---|
| `tauri-notification-no-throttle` | 通知触发无去重 / 无节流，频繁打扰 | 节流策略 + 去重 dedup key（详 `cdt-config::notification-triggers` spec） |
| `tauri-tray-icon-platform` | 托盘图标 / 角标只在 macOS 测，Windows / Linux fallback 缺失 | windows-compat-reviewer subagent 审 |
| `tauri-platform-direct-cmd` | 平台 API 用 `Command::new("xxx")` 直接 spawn 而不是 plugin | 走 `tauri-plugin-shell` 等官方 plugin（权限 + 跨平台） |

## 4. updater / 发版

| category | 反模式 | 期望 |
|---|---|---|
| `tauri-updater-no-pubkey` | `tauri.conf.json` updater pubkey 缺失或与 `TAURI_PRIVATE_KEY` 不配对 | release-runbook skill 校验 |
| `tauri-version-skew` | `Cargo.toml`(workspace) / `src-tauri/Cargo.toml` / `tauri.conf.json` 三处版本号不同步 | `just release-check` 跑校验 |

## 5. 平台兼容（Windows）

| category | 反模式 | 期望 |
|---|---|---|
| `windows-path-separator` | 硬编码 `/` 分隔符 | `std::path::MAIN_SEPARATOR` / `Path::join` |
| `windows-home-dir-naive` | 直接用 `dirs::home_dir()` 不处理 Windows roaming / fallback | 用本仓 `cdt-core::paths` 抽象 |
| `windows-encode-path-test` | 测试里把 `encode_path(windows_path)` 当真磁盘目录名 | 真假路径分离；用 fixture 生成 |
| `windows-is-absolute` | `Path::is_absolute()` 在 Windows 行为差异（`\foo` vs `C:\foo`）| 显式判断 drive prefix |

`windows-*` 全套对应 `windows-compat-reviewer` subagent，audit 命中后 SHALL 调它二审。

## 6. 仅 Tauri 验证才能抓的问题

audit 时如果命中以下信号 SHALL 提示 "需要 `just dev` 桌面端 smoke + `qa-engineer` teammate 验证"：

- 通知 / 托盘 / setBadgeCount 改动
- IPC payload 字段被前端用于 sidebar / list 渲染
- updater / installer 配置改动
- Capability / permission 边界改动

mockIPC fixture ≠ 真后端数据；vitest 跑过不代表桌面端跑过。详见 `e2e-http-verify` skill。
