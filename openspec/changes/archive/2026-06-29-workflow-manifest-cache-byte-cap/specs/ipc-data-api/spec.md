## ADDED Requirements

### Requirement: WorkflowManifestCache 内存双闸门 LRU 淘汰

`WorkflowManifestCache` 持有的三个内部 cache（manifest 解析结果 `entries` / 运行态合成 agents `journal_entries` / script 解析产物 `script_entries`）SHALL 各自采用 count cap + byte cap 双闸门 LRU 拓扑，淘汰策略与同 capability 既有 `MetadataCache` / parsed-message cache 一致。每个 cache **独立**配额（不共享同一 byte 预算），命中时 SHALL 把命中 key bump 到队首避免冷热混淆；写入后若 entry 数超过 count cap 或估算字节数超过 byte cap，SHALL 从 LRU 端淘汰直至两个上限均满足，但 SHALL 至少保留刚写入的 1 条（即便单条就超过 byte cap）。

字节计数 SHALL 按条目持有的堆上 `String` / `Vec` capacity 粗粒度估算（含固定 overhead 常量补足 LRU node + `PathBuf` key），写入与淘汰时增减 `current_bytes`；签名（`FileSignature`）mismatch 移除过期条目时 SHALL 同步扣减其字节计数。淘汰行为对外**透明**——被淘汰条目下次同 path 访问走 cache miss 重新 `stat` + 读盘，结果与命中一致，仅进程内存上界从无界变为有界。

#### Scenario: 同一 cache 超过 count cap 时按 LRU 淘汰

- **WHEN** 某个 cache 的 count cap 为 N，已写入 N 条不同 path 的条目后再写入第 N+1 条
- **THEN** 系统 SHALL 淘汰当前最久未访问的条目后再写入新条目，该 cache 的 entry 数始终 ≤ N
- **AND** 命中已缓存条目时 SHALL 把该 key 的 LRU 位置 bump 到队首，后续淘汰循环中该 key 不会被冷热顺序错误淘汰

#### Scenario: 同一 cache 超过 byte cap 时按 LRU 淘汰

- **WHEN** 某个 cache 的 byte cap 为 B，连续写入多条大条目使估算字节数累计超过 B
- **THEN** 系统 SHALL 从 LRU 端继续淘汰条目，直至 `current_bytes` ≤ B **或** cache 仅剩 1 条
- **AND** 即便单条条目自身估算字节数就超过 B（此时 `current_bytes` 可合法 > B），系统 SHALL 至少保留刚写入的该条（cache 内始终留一份）

#### Scenario: 签名 mismatch 移除条目时扣减字节计数

- **WHEN** 某 path 已缓存条目，但新一次访问的 `FileSignature` 与缓存条目不匹配（文件已变化）
- **THEN** 系统 SHALL 移除该过期条目并返回 cache miss
- **AND** 该条目的估算字节数 SHALL 从 `current_bytes` 中扣减，不残留虚高计数

#### Scenario: 三个 cache 各自独立配额互不挤占

- **WHEN** `entries` / `journal_entries` / `script_entries` 三个 cache 同时承载条目
- **THEN** 任一 cache 的写入与淘汰 SHALL 仅依据其自身的 count cap / byte cap / `current_bytes` 判定
- **AND** 一个 cache 达到上限触发淘汰 SHALL NOT 影响另外两个 cache 的留存条目
