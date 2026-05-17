## 1. `cdt-api` 后端 title 提取逻辑

- [x] 1.1 在 `crates/cdt-api/src/ipc/session_metadata.rs` 顶部加 `pub const TITLE_MAX_CHARS: usize = 500;`，并把 3 处 `truncate_str(_, 200)` 改为引用该常量
- [x] 1.2 修改 `extract_session_metadata_with_ongoing` 行 203-206 的 `is_command_content` 分支：调 `extract_command_display` 后判断"args 部分非空"——非空 SHALL 直接 `truncate_str(..., TITLE_MAX_CHARS)` 赋 `title`；空 args / 无 args SHALL 继续走 `command_fallback`
- [x] 1.3 拆 `extract_command_display` 返回值：现版本返回 `Option<String>`（已含 args 拼接），重构为返回 `(slash_name, args_or_empty)` 元组或新增 helper 区分"是否有 args"，便于 1.2 判断
- [x] 1.4 在 `extract_session_metadata_with_ongoing` 同位置加 `[Request interrupted by user` 起首消息跳过分支（位置在 `is_command_output` 同段，早于 `is_command_content` 与 teammate 检查）
- [x] 1.5 在 `sanitize_for_title` 函数尾部、`trim().to_string()` 之前加 `Read the output file to retrieve the result: \S+` 正则移除；regex 用 `once_cell::sync::Lazy<Regex>` 进程级编译
- [x] 1.6 检查 `crates/cdt-api/Cargo.toml` 是否已含 `regex` 与 `once_cell` 依赖；缺则加（`regex` 已在 workspace.dependencies 中可能已有）

## 2. `cdt-api` 单测

- [x] 2.1 加 `slash_with_non_empty_args_used_as_title` 单测：fixture 第一条 user 为 `<command-name>/impeccable</command-name><command-args>生成设计规范</command-args>`，第二条为 `提一下PR`，断言 title 为 `/impeccable 生成设计规范`
- [x] 2.2 加 `slash_with_empty_args_falls_back_to_next_message` 单测：第一条 `<command-name>/clear</command-name><command-args></command-args>`，第二条 `今天的工作`，断言 title 为 `今天的工作`
- [x] 2.3 加 `slash_without_args_tag_uses_fallback_when_no_other_message` 单测：第一条 `<command-name>/help</command-name>`，无其他 user 消息，断言 title 为 `/help`
- [x] 2.4 加 `interrupted_message_is_skipped` 单测：第一条 `[Request interrupted by user during tooling cycle]`，第二条 `继续`，断言 title 为 `继续`
- [x] 2.5 加 `read_output_file_instruction_stripped` 单测：第一条 `<task-notification>x</task-notification> Read the output file to retrieve the result: /tmp/x.txt`，断言 title 不含 `Read the output file` 与 `/tmp/x.txt`
- [x] 2.6 加 `read_output_file_multi_match_all_stripped` 单测：含两段 `Read the output file...` 全部被移除
- [x] 2.7 加 `slash_with_long_args_truncated_at_max_chars` 单测：第一条 slash args 700 字符，断言 `title.chars().count() <= 500`
- [x] 2.8 加 `plain_text_long_title_truncated_at_max_chars` 单测：第一条 700 字符纯文本，断言 `chars().count() <= 500`
- [x] 2.9 加 `slash_with_self_closing_command_args_treated_as_no_args` 单测：第一条 `<command-name>/foo</command-name><command-args/>`（自闭合无 inner），断言走 fallback（与"无 args"行为一致），有第二条 user 时 title 为第二条
- [x] 2.10 加 `sanitized_to_only_whitespace_falls_back` 单测：第一条 user content 全是 `<system-reminder>...</system-reminder>` 等噪声 sanitize 后只剩空白，断言 title 走 fallback（命令 fallback 或 None），第二条 user 消息正常作为 title
- [x] 2.11 加 `title_once_set_does_not_get_overridden` 单测：第一条 user 为有效文本 `T1`、第二条 user 为有效文本 `T2`、第三条 slash with args，断言 title = `T1`，验证 `title.is_none()` early-exit gate 正向

## 2b. `cdt-api` 缓存兼容性单测

- [x] 2b.1 加 `cache_hit_returns_legacy_title_without_recomputing` 单测：手动构造 `MetadataCacheEntry { title: Some("旧规则 title".into()), signature, ... }` 写入缓存，然后调 `extract_session_metadata_cached`——签名匹配时 SHALL 返回 `Some("旧规则 title")` 不重扫；不可断言"重扫调用次数"（uncached fn 无 hook），用"返回值与缓存写入值一致"间接验证
- [x] 2b.2 加 `cache_miss_after_signature_change_uses_new_algorithm` 单测：先写一个老 fixture 让 cache 写入旧 title；append 新行让 size 变化触发 cache miss；重新扫描时新算法生效（如截图 case 类型——slash with args 作 title）

## 3. `cdt-api` 契约 / 集成测试

- [x] 3.1 `crates/cdt-api/tests/ipc_contract.rs` 中加 `list_sessions_returns_title_up_to_500_chars` 测试：构造 700 字符 title fixture 走 `LocalDataApi::list_sessions` → `SessionSummary.title.chars().count() <= 500`
- [x] 3.2 `crates/cdt-api/tests/ipc_contract.rs` 中加 `list_sessions_title_skips_request_interrupted` 测试：fixture 第一条 user 是 interrupted 标记，断言 title 不含该字面量

## 4. UI 前端 Tab label 截断改纯 CSS

- [x] 4.1 `ui/src/lib/tabStore.svelte.ts` 第 90-92 行：删除 `shortLabel` 函数（或改为透传 `(label: string) => label`），更新所有调用点（`openTab` / `openOrReplaceTab` 等）传入 full label
- [x] 4.2 `ui/src/components/TabBar.svelte` 的 `.tab-label` CSS：确认 / 加 `max-width: 200px` + `overflow: hidden` + `text-overflow: ellipsis` + `white-space: nowrap`
- [x] 4.3 TabBar 容器 `title={tab.label}` 已存在；确认 store 改动后 `tab.label` 是 full title（hover tooltip 自动获得全文）
- [x] 4.4 grep 全局 `tab.label.slice` / `tab.label.substring` / `shortLabel(` 等模式，确认无残留不可逆截断
- [ ] 4.5 `ui/src/lib/__fixtures__/multi-project-rich.ts`（或对应 fixture）补一条 title 长度 > 100 字符的 session，便于浏览器调试模式肉眼验证 CSS ellipsis + hover tooltip 行为

## 5. UI 前端测试 / 验证

- [x] 5.1 `pnpm --dir ui run test:unit` 跑 vitest 单测，确认 tabStore 单测（如有）涵盖"label 不被截断"
- [ ] 5.2 `just dev` 启动桌面应用，肉眼验证：(a) 截图 case `sessionId=cecc12ae-...` 标题显示 `/impeccable ...`；(b) hover 长 tab 标题显示完整内容；(c) 拉宽 sidebar 长 title 显示更多

## 6. 验证与质量

- [x] 6.1 `cargo fmt --all`
- [x] 6.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 6.3 `cargo test -p cdt-api`（含新增 8 个单测 + 2 个契约测试）
- [x] 6.4 `pnpm --dir ui run check`
- [x] 6.5 `just preflight` 全绿（包含 spec validate strict）
- [ ] 6.6 `bash scripts/run-perf-bench.sh`：对比 baseline 验 `perf_cold_scan` / `perf_get_session_detail` 四维（wall / user / RSS / user/real）无回归

## 7. 性能 / 兼容验证

- [x] 7.1 确认 `MetadataCache` 未强制 invalidate；旧缓存条目仍按 `FileSignature` 命中（design.md D6）
- [ ] 7.2 PR 描述 "Perf impact" 段贴 6.6 输出对比

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
