# Tasks

## 1. Parser：接受 multimodal queued_command prompt（session-parsing 契约）
- [x] 1.1 `RawAttachment.prompt`：`Option<String>` → `Option<MessageContent>`（untagged 吃下 string/blocks 两态）
- [x] 1.2 `try_parse_queued_command`：`content` 直接取 prompt 的 `MessageContent`；加 `message_content_is_empty`（Text 看空串 / Blocks 看空数组）；返回类型收敛 `Result<Option<_>>` → `Option<_>`，调用点包 `Ok(...)`
- [x] 1.3 测试：`attachment_queued_command_with_multimodal_prompt_parsed_as_blocks` + `attachment_queued_command_empty_blocks_prompt_skipped`（`crates/cdt-parse/tests/parse_entry.rs`）；原 text/empty/missing 用例保持通过

## 2. 日志卫生（不涉及 spec）
- [x] 2.1 边界层：`cdt-cli::init_logging` 移到 `Cli::parse()` 之后，默认 filter `off`（所有输出格式 + mcp serve 全静默）；加 `-v`/`-vv`/`-vvv` = warn/info/debug；`RUST_LOG` 覆盖
- [x] 2.2 语义层：`pair.rs` 重复 tool_use/tool_result + `file.rs` malformed/schema 的 `warn!` → `debug!`（对齐 discovery 路径；重复已有 `duplicates_dropped` 聚合计数）
- [x] 2.3 纪律层：`crates/CLAUDE.md` 补"日志级别纪律"约定（CLI 默认静默 + 外部数据预期瑕疵记 debug 不记 warn）
- [x] 2.4 help 快照更新（新增 `--verbose` flag）；顺带修 `cli_help_snapshots.rs::cdt_bin` 把 stdin `Stdio::null()`，消除 clap help 换行因 tty 探测导致的跨环境（cargo test / nextest / CI）快照 flaky

## 3. 验证
- [x] 3.1 端到端：重建 cdt，`stats` 默认 / `--format json` / `RUST_LOG=warn` stderr 0 行；`-vvv` 仍可见 318 条 duplicate（DEBUG 级）；malformed 0 条（multimodal 行已正常解析）
- [x] 3.2 `cargo test -p cdt-parse -p cdt-analyze -p cdt-cli` 全绿
- [x] 3.3 `cargo clippy --workspace --all-targets -- -D warnings` + `cargo fmt --all`

## N. 发布
- [x] N.1 push 分支 + 开 PR（#551）
- [x] N.2 wait-ci 全绿（15/15 job pass）
- [x] N.3 codex 二审 0 问题（init_logging / parser untagged / warn→debug / normalize 四块全过；附带修 file.rs doc-rot warn→debug）；code-reviewer + silent-failure-hunter 关心的双 init / 错误吞没两点已自查通过
- [x] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
