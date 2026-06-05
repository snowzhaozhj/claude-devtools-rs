## Why

`LocalDataApi::get_session_detail` 在返回数据前统一调用 `apply_all_payload_omissions`，把 tool output / response content / image data 全部清空。这是**展示关注点**被放在了**数据层**，导致 MCP grep、CLI `--full` 等需要完整数据的消费者拿到的都是 omitted 数据，无法正确工作（PR #467 codex 二审 CRITICAL #1）。

## What Changes

- 将 `apply_all_payload_omissions` 调用从 `LocalDataApi::get_session_detail` 内部移除
- `LocalDataApi` 始终返回完整（未裁剪）的 `SessionDetail`
- Tauri IPC command handler（`src-tauri/src/lib.rs`）在获得完整数据后自行调用 `apply_all_payload_omissions` 再返回给前端（行为不变）
- HTTP route handler 保持不调用 omission（现有行为不变，已不裁剪）
- MCP / CLI 消费者天然拿到完整数据，grep 可正常匹配 tool output 和 response content

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `ipc-data-api`：omission 的执行位置从 `get_session_detail` 内部移到 Tauri IPC command handler 层；`DataApi` trait 契约变更为"返回完整数据，omission 由消费者自行决定"

## Impact

- `crates/cdt-api/src/ipc/local.rs`：移除 `get_session_detail` 中的 `apply_all_payload_omissions` 调用；将该函数改为 `pub` 供外部调用
- `src-tauri/src/lib.rs`：`get_session_detail` command handler 中加 `apply_all_payload_omissions` 调用
- MCP grep（PR #467）的 CRITICAL 限制自动修复
- 内存影响：`LocalDataApi` 层短暂持有完整 payload 直到返回，但 Tauri handler 立即裁剪后序列化、完整数据随即释放——实际 RSS 增量可忽略
