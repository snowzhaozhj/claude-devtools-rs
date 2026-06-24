## 1. Export 模块核心实现

- [x] 1.1 新建 `crates/cdt-cli/src/export.rs`：定义 `ExportOptions` 结构体（format / detail / include_thinking / include_subagents）+ `ExportFormat` enum（Markdown / Json）+ `ToolDetailMode` enum（Full / Summary / NameOnly）
- [x] 1.2 实现 `export_as_markdown(detail, summary, cost, options) -> String`：元数据表 + turn 分段渲染，按 semantic_steps 时序穿插工具调用
- [x] 1.3 实现 `export_as_json(detail, options) -> String`：投影处理（过滤 thinking / 截断 tool output / 去 subagent）后 `serde_json::to_string_pretty`
- [x] 1.4 实现投影函数 `project_chunk_json`：按 ExportOptions 过滤 thinking steps + content blocks / 截断或清空 tool output / 去除 subagent

## 2. CLI 子命令接入

- [x] 2.1 在 `main.rs` 新增 `Export` variant 到 `Command` enum：session_id / export_format / output / detail / no_thinking / no_subagents / range / tail / grep / grep_context / filter / all
- [x] 2.2 实现 `cmd_export` 异步函数：build_local_data_api → resolve session → get_session_detail → build_summary + compute_cost → 过滤 chunks → export → stdout 或写文件
- [x] 2.3 在 `main()` match 分支新增 `Command::Export` 处理

## 3. 测试

- [x] 3.1 `export.rs` 单元测试：Markdown 输出包含元数据表 + turn 结构 + 工具三级标题
- [x] 3.2 `export.rs` 单元测试：JSON 投影——`--no-thinking` 过滤 thinking / `--detail name-only` 清空 tool input+output
- [x] 3.3 `export.rs` 单元测试：`--detail summary` 截断超长 tool output 到 2000 字符 + `... (truncated)` 后缀

## 4. 验证

- [x] 4.1 `cargo clippy --workspace --all-targets -- -D warnings` 通过
- [x] 4.2 `cargo fmt --all` 通过
- [x] 4.3 `cargo test -p cdt-cli` 通过（含新增测试）
- [x] 4.4 真实数据验证：`cargo run -p cdt-cli -- export latest` 输出可读 Markdown
- [x] 4.5 真实数据验证：`cargo run -p cdt-cli -- export latest --export-format json --no-thinking --detail name-only -o /tmp/test-export.json` 写文件成功

## 5. 发布

- [ ] 5.1 push 分支 + 开 PR
- [ ] 5.2 wait-ci 全绿
- [ ] 5.3 codex + pr-review-toolkit 二审通过
- [ ] 5.4 archive change
