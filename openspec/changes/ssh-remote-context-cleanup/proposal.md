## Why

`ssh-remote-context` 主 spec 有 78 处 spec-purity 反模式命中（p1=34 内部模块/类/函数名 / p2=4 源文件路径 / p3=3 PR/issue 引用 / p4=19 数字诊断 / p5=1 实现开关 const / p6=17 库框架具名），是全仓密度最高的 spec。内容大量出现 `russh::client::connect` / `Arc<Mutex<SftpSession>>` / `crates/cdt-ssh/src/provider.rs` / `tracing::warn!(target: "cdt_api::perf", ...)` / `tokio JoinSet` / `broadcast::Sender<SshStatusChange>` / "50 sessions × 50ms = 2.5s wall" 等实现细节，违反 SPEC_GUIDE "spec = 用户感知 + 系统外部承诺"原则。

作为 issue #303 9-PR 计划批次 A 后半（前半 frontend-test-pyramid 已 archive，工艺直接复用）。

## What Changes

- 重写 14 个 Requirement body + 92 Scenario，移除 Rust 类型签名 / crate 路径 / 库与框架具名引用（tokio / tauri / broadcast / tracing / russh / serde） / 实测耗时数字 / PR / issue 引用 / 内部 const 名，改为用户/系统可观察行为描述
- 实现细节移入 design.md 作为参考实现指引（不属本 PR 落地范围；现有实现已稳定，此处仅做文档迁移）
- 同步刷新 `scripts/spec-purity-baseline.txt` 中 `spec/ssh-remote-context` 计数

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `ssh-remote-context`：14 个 Requirement 全做 MODIFIED 重写——行为语义不变（SHALL / MUST 句的语义对等），仅移除实现细节词法命中

## Impact

- 仅影响 `openspec/specs/ssh-remote-context/spec.md` 文档内容
- 不改代码 / 测试 / 配置
- `scripts/spec-purity-baseline.txt` 计数下降
