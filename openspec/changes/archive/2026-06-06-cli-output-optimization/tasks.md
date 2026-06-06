## 1. 共享 view 层提取

- [x] 1.1 新建 `crates/cdt-cli/src/view.rs`，从 `mcp/mod.rs` 提取：`ContentMode`、`ChunkView`（原 ChunkEnvelope）、`ToolExecView`（原 ToolExecEnvelope）、`ResponseView`（原 ResponseEnvelope）、`ContentField`、`build_chunk_view()`、`build_tool_exec_view()`、`summarize_input()`、`message_content_text()`、`tool_output_text()`、`tool_output_to_value()`、`truncate_str()`；在 `main.rs` 和 `lib.rs` 声明 `mod view` / `pub mod view`
- [x] 1.2 修改 `mcp/mod.rs`：删除提取出去的定义，改为 `use crate::view::*`；重命名 MCP-only 类型（`SessionDetailResponse` → `SessionDetailMcpResponse`、`ErrorEntry` → `McpErrorEntry`）
- [x] 1.3 验证 MCP 行为不变：`cargo test -p cdt-cli --test mcp_integration` 全部通过

## 2. grep 顺序统一 + flag 清理

- [x] 2.1 重构 `cmd_sessions_detail` 的 grep 应用顺序为 MCP 语义：先 grep/context 再 range/tail（当前是先 range/tail 再 grep）
- [x] 2.2 `--full` 改名 `--all`（`--full` 作 clap alias），更新 help 文本为"返回全部 chunk，禁用默认 tail=20"
- [x] 2.3 `--range` 与 `--tail` 加 clap `conflicts_with` 互斥校验
- [x] 2.4 更新 `sessions_detail_with_range_flag_accepted` 测试（不再用 `--range` + `--tail` 组合作为正例）

## 3. --content omit|full flag

- [x] 3.1 在 `sessions detail` 命令加 `--content <omit|full>` clap flag（`Option<String>`），非法值报错
- [x] 3.2 实现 JSON 输出路径：指定 `--content` 时使用 `build_chunk_view()` 构建 `ChunkView` 并序列化；不指定时保持原有 raw `SessionDetail` 输出
- [x] 3.3 实现 JSONL 输出路径：指定 `--content` 时每行一个 `ChunkView`（紧凑 JSON）
- [x] 3.4 实现 grep + `--content omit` 交互：grep 命中 chunk auto-expand 为 full，context chunk 保持 omit（依赖 2.1 的 grep 顺序重构）
- [x] 3.5 加 `--content` 的 contract test（omit 模式字段存在性 + full 模式内容完整性 + 非法值拒绝）

## 4. 格式契约修正

- [x] 4.1 修 `sessions summary`/`sessions cost`/`stats` 的 `Jsonl` 分支：输出紧凑单行 JSON（`to_string` 替代 `to_string_pretty`）
- [x] 4.2 四处空结果 `exit(2)` 改为 `exit(0)`（sessions list、sessions errors、search、stats）；JSON 模式输出空值 + exit 0
- [x] 4.3 统一 truncate：新增 `view::truncate_display()` 基于 `unicode-width` 计算 display width；替换 `main.rs::truncate()` 的调用点；`Cargo.toml` 加 `unicode-width` 依赖
- [x] 4.4 更新 `search_without_results_exits_2` 测试名和断言

## 5. --json fields 字段选择

- [x] 5.1 加全局 `--json <fields>` clap flag（`Option<Option<String>>`：无参数列出字段、有参数做过滤）
- [x] 5.2 实现字段投影：序列化为 `serde_json::Value` 后做投影——数组输出时对每个元素的顶层 key 过滤，单对象时对对象顶层 key 过滤；未知字段静默忽略；输出紧凑 JSON
- [x] 5.3 为每个命令定义可用字段名列表，`--json` 无参数时输出
- [x] 5.4 加 `--json` 的 integration test

## 6. --no-truncate + session-insights skill 更新

- [x] 6.1 加全局 `--no-truncate` clap flag，table 模式跳过所有 truncate 调用
- [x] 6.2 更新 `crates/cdt-cli/assets/skills/session-insights/SKILL.md`：agent 关键路径加 `--content omit` 或 `--json <fields>`

## 7. table 显示优化

- [x] 7.1 PATH 字段 `~/` 缩写（检测 home 前缀替换）
- [x] 7.2 加 `terminal-size` 依赖，实现终端宽度检测（pipe 时 fallback 120）
- [x] 7.3 各 table 实现弹性列分配（固定列 + 弹性列按剩余宽度比例分配）
- [x] 7.4 `sessions detail` table 模式 truncate 宽度从 60 → `term_width - 16`

## N. 发布
- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
