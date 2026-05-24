# Rust 结构反模式（仓特定）

读这份文件前先过 `code-smells-catalog.md`。本文聚焦 **结构维度** 反模式：错误处理选型 / 边界可见性 / serde 标注 / 测试陷阱 / 模块组织。

**不在本 catalog 范围**（其它 skill / reviewer 的本职）：
- 性能向反模式（async 阻塞 / runtime / cache byte cap / hot loop / clone）
- clippy 抓得到的（unused var / needless clone / single match）
- 跨平台兼容（Windows path / home dir）
- 配置链 / build / release / feature flag 维护

## 1. 错误处理

| category | 反模式 | 期望 |
|---|---|---|
| `rust-unwrap-boundary` | 公共 API / IPC handler / 配置加载里裸 `.unwrap()` / `.expect()`，错误无上下文 | 用 `Result` + `thiserror` 自定义错误，调用方决定 panic |
| `rust-unwrap-internal` | 内部不变量 panic 用 `assert!` / `unreachable!` 而不是 `unwrap`，让意图清晰 | 看是否真是不变量 panic；是 → 改 `assert!` 加注释；不是 → 改 Result |
| `rust-anyhow-public` | 公共 crate API 暴露 `anyhow::Result`（吞掉错误类型分层）| 公共 API 用 `thiserror` 定义具体 error；`anyhow` 仅 binary / 集成层 |
| `rust-error-stringify` | `.map_err(|e| format!("{}", e))` 把结构化错误字符串化 | 保留原始错误链，用 `#[from]` 转换 |

## 2. 边界 / 可见性

| category | 反模式 | 期望 |
|---|---|---|
| `rust-overpub` | 内部 helper 标 `pub`（实际只跨模块用） | 改 `pub(crate)` / `pub(super)` |
| `rust-trait-leak` | 公共 trait 暴露内部类型 / 内部 trait 错放 `pub mod` | trait 内部化 |
| `rust-cross-crate-private` | 跨 crate 引用 `pub` 项却走 `#[cfg(test)]` 后门 / 复制实现 | 显式 export 或拆出公共 sub-crate |

`rust-overpub` 缩 `pub` → `pub(crate)` 是 BREAKING change（即使下游没真用）→ 命中 §2 boundary guard #1（公共 API 改动），不是纯结构改。

## 3. 类型 / serde

| category | 反模式 | 期望 |
|---|---|---|
| `rust-serde-snake-camel` | 公共 IPC 类型用 snake_case 输出 | `#[serde(rename_all = "camelCase")]` |
| `rust-stringly-typed` | 用 `String` / `enum-as-string` 表达只能取几个值的字段 | `enum` + `#[serde(rename_all = "camelCase")]` |
| `rust-newtype-missing` | 业务 ID（`SessionId` / `ProjectId`）用裸 `String` | newtype `pub struct SessionId(String);` 防止参数顺序错位 |
| `rust-option-bool` | `Option<bool>` 三态意图不明 | 命名 enum：`Visibility::{Public, Private, Inherit}` |

## 4. 测试结构

| category | 反模式 | 期望 |
|---|---|---|
| `rust-test-tmpdir-leak` | 测试用 `std::env::temp_dir()` 不清理 | `TempDir`（`tempfile` crate） |
| `rust-test-fsevents-canon` | macOS 下 `TempDir` 因 `/tmp` symlink → `/private/tmp` 让 watcher 路径不匹配 | `TempDir.path().canonicalize()` |
| `rust-test-clippy-skip` | `#[allow(clippy::xxx)]` 散在 test 里掩盖共性问题 | 集中 `#[cfg_attr(test, allow(...))]` 或修源代码 |

## 5. 模块组织

| category | 反模式 | 期望 |
|---|---|---|
| `rust-mod-bloat` | `mod.rs` 同时含子模块声明 + 大量实现代码 | 拆出 `<mod-name>/types.rs` / `impl.rs` / `error.rs` |
| `rust-circular-dep` | `cdt-X` 依赖 `cdt-Y` 又反向依赖 | 提取共享类型到 `cdt-core` |
| `rust-orphan-impl` | `impl Foo for ExternalType` 在错的 crate（违反 orphan rule） | 包装 newtype 或在拥有 trait 的 crate 实现 |
