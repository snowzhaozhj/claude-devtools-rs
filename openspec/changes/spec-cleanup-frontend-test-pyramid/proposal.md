## Why

`frontend-test-pyramid` 主 spec 有 48 处 spec-purity 反模式命中（p1=1 / p2=26 / p4=3 / p6=18），是全仓第三高密度。内容包含大量源码路径、测试文件名、Rust 类型签名、CLI 命令等实现细节，违反 SPEC_GUIDE "spec = 用户感知 + 系统外部承诺"原则。作为 issue #303 9-PR 计划批次 A 清理。

## What Changes

- 重写全部 7 个 Requirement body + Scenario，移除源码路径 / 测试文件路径 / Rust 类型签名 / 具体 CLI 命令 / 库框架具名引用，改为用户/系统可观察行为描述
- 实现细节移入 design.md 作为参考实现指引
- 同步刷新 `scripts/spec-purity-baseline.txt` 中 `frontend-test-pyramid` 计数

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `frontend-test-pyramid`：全部 7 个 Requirement 做 MODIFIED 重写——行为语义不变，只移除实现细节词法命中

## Impact

- 仅影响 `openspec/specs/frontend-test-pyramid/spec.md` 文档内容
- 不改代码 / 测试 / 配置
- `scripts/spec-purity-baseline.txt` 计数下降
