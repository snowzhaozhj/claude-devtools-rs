# Proposal: cleanup-purity-discovery-telemetry-server

## Problem

PR #336 把 ipc-data-api 的 Requirement 搬到 domain cap 时字符级保留了所有实现细节引用。purity check 现报 59 hits（project-discovery 20 / application-telemetry 20 / server-mode 19），含内部 mod path、源码路径、lib/framework 名、实测 metric 等反模式。

## Solution

逐 Requirement 重写 body：删除内部 fn/type/mod 名、src 路径、框架引用，改为行为描述；保留 IPC 字段名 / Tauri command 名 / HTTP endpoint path / SSE event 名 / 错误码等外部协议。

## Scope

- project-discovery: 20 hits (4 p1 + 8 p2 + 2 p4 + 6 p6)
- application-telemetry: 20 hits (4 p1 + 4 p2 + 6 p4 + 6 p6)
- server-mode: 19 hits (2 p1 + 8 p2 + 1 p4 + 8 p6)

## Non-goals

- 不改行为契约语义
- 不触碰其它 capability spec
