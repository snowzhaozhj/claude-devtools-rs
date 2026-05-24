# Rust 反模式（仓特定）

读这份文件前先过 `code-smells-catalog.md`。本文聚焦 Rust + tokio + serde + workspace 边界场景，**不**重复 `.claude/rules/perf.md::反模式清单`（性能反模式由 perf skill 负责，本 skill 引用即可）。

## 1. 错误处理

| category | 反模式 | 期望 |
|---|---|---|
| `rust-unwrap-boundary` | 公共 API / IPC handler / 配置加载里裸 `.unwrap()` / `.expect()`，错误无上下文 | 用 `Result` + `thiserror` 自定义错误，调用方决定 panic |
| `rust-unwrap-internal` | 内部不变量 panic 用 `assert!` / `unreachable!` 而不是 `unwrap`，让意图清晰 | 看是否真是不变量 panic；是 → 改 `assert!` 加注释；不是 → 改 Result |
| `rust-anyhow-public` | 公共 crate API 暴露 `anyhow::Result`（吞掉错误类型分层）| 公共 API 用 `thiserror` 定义具体 error；`anyhow` 仅 binary / 集成层 |
| `rust-error-stringify` | `.map_err(|e| format!("{}", e))` 把结构化错误字符串化 | 保留原始错误链，用 `#[from]` 转换 |

引用：`crates/CLAUDE.md::Rust 边界 / 错误类型`。

## 2. 边界 / 可见性

| category | 反模式 | 期望 |
|---|---|---|
| `rust-overpub` | 内部 helper 标 `pub`（实际只跨模块用） | 改 `pub(crate)` / `pub(super)`；外部 API 改动需 spec delta |
| `rust-trait-leak` | 公共 trait 暴露内部类型 / 内部 trait 错放 `pub mod` | trait 内部化或在 spec 里明确为契约 |
| `rust-cross-crate-private` | 跨 crate 引用 `pub` 项却走 `#[cfg(test)]` 后门 / 复制实现 | 显式 export 或拆出公共 sub-crate |
| `rust-feature-flag-rot` | `Cargo.toml` features 长期未用 / feature gate 不一致（lib 启 feature 但 binary 不启） | 删除或合并；CI 需覆盖各 feature 组合 |

`rust-overpub` 命中时**走 boundary guard**：缩 `pub` → `pub(crate)` 是 BREAKING change（即使下游没真用），SHALL 走 openspec 评估。

## 3. async / runtime（指针，**不**重复 perf.md 内容）

完整 async / runtime / 调度 / 监控反模式清单见 **`.claude/rules/perf.md::反模式清单`**——本 skill **不**重抄。审计时直接从 perf.md 取规则，对应 finding 的 `category` 用：

- `rust-async-blocking`（`std::fs::*` 在 async 内 / 阻塞 tokio worker）
- `rust-spawn-no-cancel`（`tokio::spawn` 无 cancellation handle / task 泄漏；详 `crates/CLAUDE.md::后台任务 per-key cancel`）
- `rust-channel-capacity`（`mpsc` / `broadcast` 容量不合理）
- `rust-runtime-collision`（多 Runtime 并存）

性能向反模式（hot loop spawn / 未限流 / clone 大对象）走 `Skill(perf)`；本 catalog 仅复用 category 命名作为 finding 的 cross-reference key。

## 4. 类型 / serde

| category | 反模式 | 期望 |
|---|---|---|
| `rust-serde-snake-camel` | 公共 IPC 类型用 snake_case 输出 | `#[serde(rename_all = "camelCase")]`（详 `crates/CLAUDE.md`） |
| `rust-stringly-typed` | 用 `String` / `enum-as-string` 表达只能取几个值的字段 | `enum` + `#[serde(rename_all = "camelCase")]` |
| `rust-newtype-missing` | 业务 ID（`SessionId` / `ProjectId`）用裸 `String` | newtype `pub struct SessionId(String);` 防止参数顺序错位 |
| `rust-option-bool` | `Option<bool>` 三态意图不明 | 命名 enum：`Visibility::{Public, Private, Inherit}` |

## 5. 测试 / 边界陷阱

| category | 反模式 | 期望 |
|---|---|---|
| `rust-test-tmpdir-leak` | 测试用 `std::env::temp_dir()` 不清理 | `TempDir`（`tempfile` crate） |
| `rust-test-fsevents-canon` | macOS 下 `TempDir` 因 `/tmp` symlink → `/private/tmp` 让 watcher 路径不匹配 | `TempDir.path().canonicalize()`；详 `crates/CLAUDE.md::测试基础设施陷阱` |
| `rust-test-clippy-skip` | `#[allow(clippy::xxx)]` 散在 test 里掩盖共性问题 | 集中 `#[cfg_attr(test, allow(...))]` 或修源代码 |
| `rust-test-mock-real` | mock 行为与真后端不一致（typically IPC contract 测过但 mock 数据形状错） | 先跑 IPC contract test 校形状再 mock |

## 6. 模块组织

| category | 反模式 | 期望 |
|---|---|---|
| `rust-mod-bloat` | `mod.rs` 同时含子模块声明 + 大量实现代码 | 拆出 `<mod-name>/types.rs` / `impl.rs` / `error.rs` |
| `rust-circular-dep` | `cdt-X` 依赖 `cdt-Y` 又反向依赖 | 提取共享类型到 `cdt-core` |
| `rust-orphan-impl` | `impl Foo for ExternalType` 在错的 crate（违反 orphan rule） | 包装 newtype 或在拥有 trait 的 crate 实现 |

## 7. clippy 之外的语义级问题

clippy 会抓的（unused var / needless clone / single match）**不**进 audit report——本 skill 只关心 clippy 抓不到的：

- 公共 API 改动是否破坏向后兼容
- 错误类型选型是否合理（thiserror vs anyhow vs `Box<dyn Error>`）
- 后台任务取消语义是否完整
- async 边界 / Send + Sync bound 是否漂移

完整 Rust 约定见 `.claude/rules/rust.md` 与 `crates/CLAUDE.md`（如不存在则以 `crates/CLAUDE.md` 为准）。
