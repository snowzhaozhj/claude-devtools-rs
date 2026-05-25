## MODIFIED Requirements

### Requirement: Expose a pure synchronous API driven by chunk output

系统 SHALL 以纯同步 API 形式提供 context tracking：消费内存中的 chunk 序列以及外部注入的 per-file token 字典；SHALL NOT 在 context 计算过程中执行文件 I/O、网络调用或其它副作用。该 API SHALL 可在非 async 代码路径调用，SHALL NOT 依赖 `tokio` 等 runtime。

#### Scenario: Library consumer calls the API from a sync context

- **WHEN** 非 async 线程调用 context tracking 主入口，传入借用的 chunk slice 与已填好 token 字典的处理参数
- **THEN** 该入口 SHALL 在不 spawn task、不 await future、不访问文件系统的前提下返回 `SessionContextResult`

#### Scenario: Missing token data falls back to zero without error

- **WHEN** 注入的 `claude_md_token_data` / `mentioned_file_token_data` / `directory_token_data` map 不包含某 chunk 引用的 key
- **THEN** 对应 injection SHALL 仍被产出（`estimated_tokens = 0`），且函数 SHALL NOT 返回错误、panic 或写出高于 `debug` 级别的日志

#### Scenario: Empty chunk slice yields empty result

- **WHEN** 传入的 chunk slice 为空
- **THEN** 返回的 `SessionContextResult` SHALL 含空 `stats_map`、空 `phase_info.phases`、且 `phase_info.compaction_count == 0`
