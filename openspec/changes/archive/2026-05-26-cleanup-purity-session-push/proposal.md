# Proposal: cleanup-purity-session-push

## Why

PR #336 字符级搬运导致 session-parsing (46 hits) 和 push-events (33 hits) 中残留大量实现细节引用（mod path / src path / lib 引用 / impl const / 实测 metric），违反 SPEC_GUIDE 规范。

## What Changes

清理两个 spec 中所有 purity 命中的 Requirement body：
- 内部 fn/type/mod 路径 → 行为描述
- 源码路径 → 删除
- 库/框架引用 → 行为描述
- 回滚开关 const 名 → 用数字描述行为
- 实测 metric → 保留用户感知阈值，删纯实测数据

行为契约语义不变。IPC 字段名 / Tauri command 名 / SSE event 名保留。
