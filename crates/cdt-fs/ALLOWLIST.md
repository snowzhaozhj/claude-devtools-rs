# H1 Allowlist —— `tokio::fs::*` 直调豁免清单

`xtask check-fs-direct-calls` 与 `crates/cdt-api/tests/build_time_invariants.rs`
SHALL 在运行时 parse 本表作为 allowlist 输入。表外路径命中 `tokio::fs::*`
即报 H1 violation。任何 allowlist 增删 SHALL 改本表，**不**改 xtask 源码 /
测试源码（single source of truth）。

行为契约 + H1-H6 完整定义见 `openspec/specs/fs-abstraction/spec.md`。

## 豁免准则

新加 ALLOWLIST 行 SHALL 满足以下任一条件且在 PR description 引用对应 design 决策：

1. **路径在 design.md 已分类为 Local-only 业务**（typical: 用户配置 / 系统通知历史 / Local-only disk cache）—— 引用 change `unify-fs-direct-calls` design D7
2. **SSH 路径有显式 graceful skip / 该路径永远不接 SSH context**（HTTP routes / notifier / mention SSH 契约）—— 引用 design D4 / D7
3. **测试 fixture / 测试 setup 写文件**（覆盖 `**/tests/**`）

每条 entry 的 `reason` 列 SHALL 简要说明豁免依据，xtask 启动时校验 reason 非空（详 design D4）。

## Allowlist

| crate/path | reason |
|---|---|
| `crates/cdt-fs/**` | fs 抽象层 crate 本身（含 `LocalFileSystemProvider` 实现 + instrumentation 单测 + open_read overhead bench） |
| `crates/cdt-cli/src/main.rs` | binary entrypoint，初始化日志 / 配置加载读 file 是 boot phase |
| `crates/cdt-watch/src/**` | `notify` 库本身基于 inotify / FSEvents，非 fs read/write 抽象的范畴 |
| `**/tests/**` | 测试 setup 直读 fixture / 写 `TempDir`（覆盖 workspace 内任意 `tests/` 目录） |
| `crates/cdt-ssh/src/provider.rs` | `SshFileSystemProvider` 实现层，与 `LocalFileSystemProvider` 同等地位 |
| `crates/xtask/**` | dev tooling 自身 |
| `crates/cdt-config/**` | 用户配置 / 通知历史 / 内存文件 / @mention 文件读取——永远 Local context（用户机本地配置），不参与 SSH cache；mention.rs SSH context 下 SHALL 走 graceful skip 返结构化错误而非读 Local 路径串扰（详 change unify-fs-direct-calls design D7） |
| `crates/cdt-api/src/notifier.rs` | poll_session metadata 检测仅对 Local Tauri sessions 生效（SSH session 走前端心跳，notifier 不接 SSH context） |
| `crates/cdt-api/src/http/routes.rs` | HTTP file serve / image data-URI——HTTP context 当前不接 SSH（remote backend 通过 IPC 路径接入），future 若开 server-mode SSH 再扩 |
| `crates/cdt-discover/src/wsl.rs` | WSL distro 探测——永远本机 Local，跨发行版 Windows 路径检测，与 SSH context 无关（design D7 同型 Local-only 业务） |
| `crates/cdt-discover/src/worktree_grouper.rs` | Local-only Git plumbing 读 .git/HEAD / commondir 等文件——SSH 路径走 NoopGitIdentityResolver（local.rs:3068 policy fork），永远不调此 module |
| `crates/cdt-ssh/src/config_parser.rs` | ssh config 文件解析——boot / connect phase Local 读取 ~/.ssh/config，与 active SSH context 的 fs trait 无关 |
