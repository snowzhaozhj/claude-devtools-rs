# Design: cleanup-purity-session-push

## D1：清理策略

整 Requirement body 重写确保一致性。只重写命中的 Requirement body，未命中 Requirement 不动。重写时保留 SHALL/WHEN/THEN 结构和语义。

## D2：数字保留判断

- **保留**：5 分钟 stale（用户感知）、2000 entries LRU（NFR）、50 entries（NFR）、500 char（用户感知）、32 KiB（SFTP 协议约束）、并发度 8（NFR）、100 MB（用户感知）、300 ms debounce（用户感知）
- **删除**：50ms RTT / 2.5s / ~50-100ms RTT（实测观测）、local.rs:896 / session.rs:323-326（源码行号）

## D3：Requirement title 保持不变

MODIFIED delta 的 Requirement title 必须与主 spec 完全匹配。即使 title 含 `LocalDataApi` 等实现名，也保留以确保 archive 工具正确 replace。Title 中的实现名后续可通过 RENAMED 操作单独处理。
