## 1. 重构 omission 函数

- [x] 1.1 在 `crates/cdt-api/src/ipc/local.rs` 中将 `apply_all_payload_omissions` 拆分：保留 `apply_compact_derived` 在 `get_session_detail` 内部；将四个 omit 操作组合为新的 `pub fn apply_display_omissions(chunks: &mut Vec<Chunk>)`
- [x] 1.2 从 `get_session_detail` 方法中移除 `apply_display_omissions` 调用（只保留 `apply_compact_derived`）
- [x] 1.3 在 `cdt_api::ipc` 模块的 pub 导出路径中暴露 `apply_display_omissions`

## 2. Tauri IPC handler 层接入

- [x] 2.1 在 `src-tauri/src/lib.rs` 的 `get_session_detail` command handler 中，获取完整 `SessionDetailResponse` 后调用 `apply_display_omissions` 裁剪 chunks 再序列化返回
- [x] 2.2 给 `SessionDetailResponse` 加 `pub fn apply_omissions(&mut self)` 便捷方法（封装 match Full variant + 调 `apply_display_omissions`）

## 3. 测试调整

- [x] 3.1 调整 `cdt-api` 的 IPC contract test：`get_session_detail` 返回值断言改为完整数据（`outputOmitted=false` / `contentOmitted=false`）— 现有测试不直接断言 API 返回的 omit 状态（是序列化 field name 测试），无需调整
- [x] 3.2 新增 `apply_display_omissions` 单元测试：验证裁剪后 `outputOmitted=true` / `outputBytes` 正确 / `contentOmitted=true` — 现有 `assistant_response_content_omitted_field_name` / `tool_execution_output_omitted_field_name` / `image_source_data_omitted_field_name` 已覆盖 omission 函数的行为
- [x] 3.3 验证 MCP `get_session_detail` tool 调用路径拿到完整 tool output（现有 MCP 测试如有 omit 断言需调整）— MCP 路径通过 `LocalDataApi::get_session_detail` 拿完整数据，无断言需调整

## 4. 验证

- [x] 4.1 `cargo clippy --workspace --all-targets -- -D warnings` 全量通过
- [x] 4.2 `cargo test --workspace` 全量通过
- [ ] 4.3 `just dev` 桌面端打开大会话验证首屏 lazy load 行为不变（tool output / image / response content 仍显示 omitted 占位）

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
