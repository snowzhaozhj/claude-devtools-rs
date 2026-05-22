## MODIFIED Requirements

### Requirement: `xtask check-fs-direct-calls` 自动化 H1

系统 SHALL 提供 `xtask check-fs-direct-calls` 命令（或等价 cargo / shell 脚本），扫描业务 crate 内 `tokio::fs::*` 直调反模式。脚本 SHALL：

1. 扫描路径：`crates/cdt-api/src/**/*.rs`、`crates/cdt-config/src/**/*.rs`、业务路径其他 crate
2. allowlist：从 `crates/cdt-fs/ALLOWLIST.md` 的 `## Allowlist` markdown table 解析；本 change 后 allowlist SHALL 含 provider 实现文件（`crates/cdt-fs/**`、`crates/cdt-ssh/src/provider.rs`）、`crates/cdt-cli/**`、`crates/cdt-watch/**`、`crates/xtask/**`、`**/tests/**`、以及 design.md D4 / D7 钉死的 Local-only 业务路径（`crates/cdt-config/**`、`crates/cdt-api/src/notifier.rs`、`crates/cdt-api/src/http/routes.rs`、`crates/cdt-api/src/ipc/image_disk_cache.rs`）
3. 匹配模式：`tokio::fs::metadata` / `tokio::fs::read` / `tokio::fs::File::open` / `tokio::fs::read_to_string` / `tokio::fs::read_dir` / `tokio::fs::write` / `tokio::fs::create_dir(_all)?` / `tokio::fs::remove(_file|_dir(_all)?)?` 等 13 个 forbidden patterns
4. 退出码：non-allowlist 命中时 SHALL 默认 exit 1 (`ExitCode::FAILURE`)，CI 拒；`--warn-only` flag 仅作本地诊断 opt-in（exit 0 + warning 输出），CI 不 SHALL 加此 flag

#### Scenario: xtask 命令存在且可调用

- **WHEN** 在仓库根跑 `cargo xtask check-fs-direct-calls` 或等价命令
- **THEN** 命令 SHALL 存在并产出 grep 结果到 stdout
- **AND** 退出码反映检查结果

#### Scenario: allowlist 路径不报警

- **WHEN** xtask 扫描时遇到 `crates/cdt-fs/src/local.rs` 内 `tokio::fs::metadata` 调用
- **THEN** SHALL NOT 报警（因为这是 LocalFileSystemProvider 内部实现，被 allowlist）

#### Scenario: 默认 fail-on-match（CI enforce）

- **WHEN** CI 跑 `cargo xtask check-fs-direct-calls`（**不**带 `--warn-only` flag）且业务路径出现一处非 allowlist 的 `tokio::fs::*` 直调
- **THEN** xtask 进程 SHALL 以 `ExitCode::FAILURE` 退出，CI step fail
- **AND** stdout SHALL 含 `error: <relpath> (H1 violation) -- '<pattern>' at <relpath>:<line_no>` 格式的错误行 + 末尾 `xtask: check-fs-direct-calls found N violation(s); allowlist source = crates/cdt-fs/ALLOWLIST.md`

#### Scenario: `--warn-only` 仅供本地诊断

- **WHEN** 开发者本地跑 `cargo xtask check-fs-direct-calls --warn-only`
- **THEN** xtask SHALL 以 `ExitCode::SUCCESS` 退出 + stdout 列出 `warning:` 前缀的违反清单 + 末尾打印 `xtask: --warn-only is on，exit 0`
- **AND** 这条路径仅作开发者迁移期诊断手段，CI workflow `.github/workflows/*.yml` SHALL NOT 在 `cargo xtask check-fs-direct-calls` invocation 上传 `--warn-only`

#### Scenario: ALLOWLIST.md 顶部固化豁免准则

- **WHEN** 阅读 `crates/cdt-fs/ALLOWLIST.md`
- **THEN** 文件 SHALL 在 `## Allowlist` table 之前的段落明示豁免准则：
  - 路径在 design.md 已分类为 Local-only 业务路径（typical: 用户配置 / 系统通知历史 / Local-only disk cache）
  - 或 SSH 路径有显式 graceful skip / 该路径永远不接 SSH context（HTTP routes / notifier）
  - 或测试 fixture / 测试 setup 写文件（覆盖 `**/tests/**`）
- **AND** 任何新加 ALLOWLIST 行的 PR SHALL 在 PR description 引用对应 design 决策的锚点（典型 D7 cdt-config 全 Local / D4 image disk cache 抽 module）
