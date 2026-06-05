## Context

`apply_all_payload_omissions` 目前在 `LocalDataApi::get_session_detail`（`crates/cdt-api/src/ipc/local.rs:3952`）统一执行，所有消费者拿到的都是裁剪后数据。消费者需求不同：

- **Tauri IPC**：需要 omit（首屏 payload 瘦身，用户展开时 lazy load）
- **MCP server**：需要 full（grep 匹配需要完整 tool output 和 response content）
- **CLI**：需要 full（`cdt show --detail` 输出完整内容）
- **HTTP route**：已不裁剪（现有行为）

## Goals / Non-Goals

**Goals:**

- `LocalDataApi::get_session_detail` 返回完整数据，omission 由消费者层决定
- Tauri IPC 前端行为零变化（仍拿到裁剪后 payload + lazy load 机制不变）
- MCP/CLI 消费者无需额外改动即可获得完整数据
- `apply_all_payload_omissions` 保持为可复用的公共函数

**Non-Goals:**

- 不改 omission 逻辑本身（各 `OMIT_*` 开关 / `xxxOmitted` flag 语义不变）
- 不改 `compact_derived` 逻辑（它不是展示裁剪，是 derived 字段填充，保留在 data layer）
- 不改 HTTP route handler（已是不裁剪的状态）
- 不做 per-field 精细控制（如"MCP 只保留 tool output 但裁剪 image"）——后续 change 按需加

## Decisions

### D1：omission 函数拆分——compact_derived 留原处，omission 提为 pub

**候选方案：**
- A：整个 `apply_all_payload_omissions` 移到 Tauri handler（compact_derived 也移走）
- B：拆分——`apply_compact_derived` 留在 `get_session_detail` 内（它是数据补全不是裁剪），四个 omit 函数组合成新的 `pub fn apply_display_omissions` 由消费者层调

**选择 B**。`compact_derived` 填充 `phase_number` / `token_delta` 等 derived 字段，MCP/CLI 同样需要——它不是"展示裁剪"而是"数据补全"，留在 data layer 是正确分层。

### D2：可见性——`pub(crate)` vs `pub`

**选择 `pub`**。`src-tauri` 是独立 crate（不在 workspace 内），通过 `cdt-api` 的 public API 调用。必须 `pub` 才能在 `src-tauri` 使用。导出路径：`cdt_api::ipc::apply_display_omissions`。

### D3：函数签名——保持 `&mut Vec<Chunk>` 原地修改

保持现有 `&mut` 签名不变。Tauri handler 拿到 `SessionDetailResponse` 后解构取 chunks → omit → 重组回 response → 序列化。无需 clone，无额外内存开销。

### D4：SessionDetailResponse 结构——需要暴露 chunks 的可变访问

当前 `SessionDetailResponse::Full` 内部字段（`SessionDetail.chunks`）需要 Tauri handler 能 `&mut` 访问。选择给 `SessionDetailResponse` 加 `pub fn apply_omissions(&mut self)` 方法，封装"只对 Full variant 的 chunks 做 omission"的逻辑——Tauri handler 调一行即可，无需手动 match variant。

## Risks / Trade-offs

- **[内存短暂增大]** → `LocalDataApi` 层到 Tauri handler 执行 omission 之间，完整数据存在于内存中。但这是同一个 async 函数内的顺序操作，时间窗口 < 1ms，实际 RSS 增量可忽略（裁剪前后都是同一次 allocation，只是 String 内容被清空）
- **[公开 API 表面扩大]** → `apply_display_omissions` 变 pub 增加了 `cdt-api` 的公开 API。但这是有意为之——消费者层需要它。用 `#[doc(hidden)]` 或模块层级限制都不合适，因为 `src-tauri` 确实需要公开访问
- **[测试断言变化]** → 现有 `get_session_detail` 的 IPC contract test 断言 omitted 字段，需要调整为：data layer 返回完整数据 + 单独测 omission 函数
