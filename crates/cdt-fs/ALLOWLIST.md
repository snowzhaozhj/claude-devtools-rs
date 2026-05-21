# H1 Allowlist —— `tokio::fs::*` 直调豁免清单

`xtask check-fs-direct-calls` 与 `crates/cdt-api/tests/build_time_invariants.rs`
SHALL 在运行时 parse 本表作为 allowlist 输入。表外路径命中 `tokio::fs::*`
即报 H1 violation。任何 allowlist 增删 SHALL 改本表，**不**改 xtask 源码 /
测试源码（single source of truth）。

行为契约 + H1-H6 完整定义见 `openspec/specs/fs-abstraction/spec.md`。

## Allowlist

| crate/path | reason |
|---|---|
| `crates/cdt-fs/**` | fs 抽象层 crate 本身（含 `LocalFileSystemProvider` 实现 + instrumentation 单测 + open_read overhead bench） |
| `crates/cdt-cli/src/main.rs` | binary entrypoint，初始化日志 / 配置加载读 file 是 boot phase |
| `crates/cdt-watch/src/**` | `notify` 库本身基于 inotify / FSEvents，非 fs read/write 抽象的范畴 |
| `**/tests/**` | 测试 setup 直读 fixture / 写 `TempDir`（覆盖 workspace 内任意 `tests/` 目录） |
| `crates/cdt-ssh/src/provider.rs` | `SshFileSystemProvider` 实现层，与 `LocalFileSystemProvider` 同等地位 |
| `crates/xtask/**` | dev tooling 自身 |
