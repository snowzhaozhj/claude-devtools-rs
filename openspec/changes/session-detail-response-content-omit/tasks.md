## 1. cdt-core 数据结构扩展

- [x] 1.1 `crates/cdt-core/src/chunk.rs::AssistantResponse` 加 `#[serde(rename = "contentOmitted", default)] pub content_omitted: bool` 字段
- [x] 1.2 `assistant_response_roundtrip` 测试加 `content_omitted` 字段；新增 `assistant_response_default_content_omitted_false` + `assistant_response_content_omitted_roundtrip`
- [x] 1.3 同步更新 `cdt-analyze` / `cdt-api` 内所有 `AssistantResponse { ... }` 字面构造点（chunk/builder.rs L120 / tool_linking/resolver.rs L699 + L751 / context_tracking.rs L45 / cdt-api/src/ipc/local.rs L1544 等）补 `content_omitted: false`
- [x] 1.4 `cargo test -p cdt-core` 通过；`cargo build --workspace` 不破其他 crate 构造点

## 2. cdt-api 后端 OMIT 路径

- [x] 2.1 `crates/cdt-api/src/ipc/local.rs` 顶部加 `const OMIT_RESPONSE_CONTENT: bool = true;` 模块常量（紧贴 OMIT_IMAGE_DATA 上下，加注释引用 change slug）
- [x] 2.2 新增 `apply_response_content_omit(chunks: &mut [Chunk])` 函数：递归覆盖顶层 AIChunk + subagent.messages 嵌套层，把 `responses[i].content` 替换为 `MessageContent::Text(String::new())` + `content_omitted = true`
- [x] 2.3 `get_session_detail` 序列化前调用顺序：image OMIT → response.content OMIT → subagent OMIT（每个开关独立判定）
- [x] 2.4 单元测试 `apply_response_content_omit_clears_assistant_response_content` 覆盖顶层 AIChunk 路径
- [x] 2.5 单元测试 `apply_response_content_omit_clears_nested_subagent_response_content` 覆盖嵌套 subagent.messages 路径
- [x] 2.6 `cargo test -p cdt-api` 全过（19 lib tests + integration 全通过）

## 3. preflight + perf bench 验证

- [x] 3.1 `just fmt` 通过
- [x] 3.2 `just lint` 通过（workspace + src-tauri clippy）
- [x] 3.3 `just test` 通过（含前端，svelte-check 0 errors / 5 pre-existing warnings）
- [x] 3.4 `just spec-validate` 通过（22 passed）
- [x] 3.5 `cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture` 重跑：实测三 case payload：4cdfdf06 515→**248 KB** (-52%) / 7826d1b8 620→**334 KB** (-46%) / 46a25772 3070→**1829 KB** (-40%)，命中预期（46a25772 ~1800 KB target）

## 4. 收尾

- [ ] 4.1 `openspec/followups.md` 性能条目加 Phase 4 落地子段（含三 case 实测数字 + 行为契约引用 + 下下轮 follow-up 方向 tool_exec 懒加载）
- [ ] 4.2 commit：`feat(perf): response.content OMIT (phase 4)`，body 引用本 change slug 与预期收益数字
- [ ] 4.3 `openspec archive session-detail-response-content-omit -y` 把 delta sync 回主 spec
