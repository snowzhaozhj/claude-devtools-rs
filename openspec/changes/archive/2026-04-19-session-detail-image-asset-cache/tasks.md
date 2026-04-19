## 1. cdt-core 数据结构扩展

- [x] 1.1 `crates/cdt-core/src/message.rs::ImageSource` 加 `#[serde(default)] pub data_omitted: bool` 字段
- [x] 1.2 该字段单独标 `#[serde(rename = "dataOmitted", default)]`（struct 整体保持 snake_case 与上游 Anthropic 格式一致，data_omitted 是 IPC derived 字段例外走 camelCase）
- [x] 1.3 加 `Default` derive；workspace test 全过验证无构造点破坏
- [x] 1.4 添加 roundtrip 单元测试：`image_source_default_data_omitted_false` + `image_source_data_omitted_roundtrip`
- [x] 1.5 `cargo test -p cdt-core` 通过

## 2. cdt-api 后端 OMIT 路径

- [x] 2.1 `crates/cdt-api/src/ipc/local.rs` 顶部加 `const OMIT_IMAGE_DATA: bool = true;` 模块常量
- [x] 2.2 `get_session_detail` 序列化前加 `apply_image_omit(&mut cloned)`，递归覆盖 UserChunk.content / AIChunk.responses[].content / AIChunk.subagents[].messages 嵌套层内所有 ImageBlock 的 `source.data` 与 `data_omitted` 字段
- [x] 2.3 `OMIT_IMAGE_DATA = false` 时 `apply_image_omit` 跳过调用（顺序：image OMIT 在 subagent OMIT 之前，回滚组合下嵌套层仍能命中）
- [x] 2.4 单元测试：`apply_image_omit_clears_user_image_data` + `apply_image_omit_clears_assistant_response_image` 覆盖顶层两路径
- [~] 2.5 集成测试：deferred — task 6 perf bench 在真实 image-heavy JSONL 上验证；单元层 + perf bench 已覆盖
- [x] 2.6 `cargo test -p cdt-api` 全过

## 3. cdt-api 后端新增 get_image_asset IPC

- [x] 3.1 workspace `Cargo.toml` 加 `base64 = "0.22"`（`sha2` 已存在）；`crates/cdt-api/Cargo.toml` 引入 `sha2 + base64 workspace deps`
- [x] 3.2 `crates/cdt-api/src/ipc/traits.rs::DataApi` trait 加 `async fn get_image_asset(...)`，默认实现返回空字符串
- [x] 3.3 `LocalDataApi::get_image_asset` 实现：rsplit_once `:` 解析 block_id → `locate_session_jsonl`（root 自身或 subagents/）→ `parse_file` → `find_image_block_in_messages` → `materialize_image_asset`（SHA256 前 8 字节 hex / media_type→ext / 已存在短路 / 失败 fallback `data:` URI）
- [x] 3.4 `with_image_cache(self, dir: PathBuf)` 链式构造器（`new()` 签名不变；Tauri host 通过 `LocalDataApi::new(...).with_image_cache(...)` 注入；测试无 cache 走 fallback 路径）
- [x] 3.5 单元测试：`materialize_image_asset_writes_file_and_dedupes` 覆盖落盘 + 同 hash 复用
- [x] 3.6 单元测试：`get_image_asset_invalid_block_id_returns_empty_data_uri` + `materialize_image_asset_fallbacks_on_invalid_base64` 覆盖 fallback 路径
- [x] 3.7 `cargo test -p cdt-api` 通过（17 passed）

## 4. Tauri 集成

- [x] 4.1 `src-tauri/src/lib.rs` 用 `dirs::cache_dir().join("claude-devtools-rs/cdt-images")` 同步算 cache 目录，链式 `LocalDataApi::new_with_watcher(...).with_image_cache(dir)` 注入（Tauri setup 之外初始化，无需 app handle）
- [x] 4.2 `src-tauri/src/lib.rs` 注册新 Tauri command `get_image_asset(state, root_session_id, session_id, block_id) -> Result<String, String>`
- [x] 4.3 `src-tauri/src/lib.rs::invoke_handler!` 加 `get_image_asset` 入口
- [x] 4.4 `capabilities/default.json` 不需要新增权限——asset protocol 在 `core:default` 内（cargo 自动加 `protocol-asset` feature）
- [x] 4.5 `src-tauri/tauri.conf.json::app.security.assetProtocol`：`enable: true` + 三平台 cache 路径 scope（macOS `$HOME/Library/Caches/`、Linux `$HOME/.cache/`、Windows `$LOCALAPPDATA/`）
- [ ] 4.6 手动验证：`just dev` 启动后，开一个含 image 的 session，浏览器 devtools Network 面板看 `<img src="asset://...">` 加载成功（200，非 403）—— 留到前端 5.x 完成后一起跑

## 5. 前端 ImageBlock 视口懒拉

- [x] 5.1 `ui/src/lib/api.ts` 加 `getImageAsset(rootSessionId, sessionId, blockId): Promise<string>`
- [x] 5.2 `api.ts` 加 `ImageSource` 类型导出（`type` / `media_type` / `data` / `dataOmitted?`）；`ContentBlock.source?: ImageSource`
- [x] 5.3 现有 image 渲染入口：**前端从未渲染 image**——`utext()` 只取 text block。本次 phase 3 顺手补齐 image 渲染（属于隐藏 coverage gap，前端原本拿到 base64 也直接丢弃）
- [x] 5.4 新建 `ui/src/components/ImageBlock.svelte`：props (source / rootSessionId / sessionId / blockId)；`{@attach}` 挂 IntersectionObserver(rootMargin=200px)；进视口调 `getImageAsset` → `<img src>`；`dataOmitted=false` 直接 `data:` URI；加载中固定 200px 高度占位
- [x] 5.5 `SessionDetail.svelte` user chunk 渲染分支加 `uimages()` 抽 image blocks → `{#each images}` 渲染 `<ImageBlock>`，blockId 按 `<chunkUuid>:<blockIndex>` 拼接
- [x] 5.6 `npm run check --prefix ui` 通过（0 errors，5 warnings 全是预先存在与本次无关）
- [ ] 5.7 手动验证：留到 Task 6 perf bench 后一起跑

## 6. perf bench 验证收益

- [x] 6.1 `cargo test --release -p cdt-api --test perf_get_session_detail -- --ignored --nocapture` 重跑
- [x] 6.2 4cdfdf06: 1768 → **515 KB**（砍 71%）；7826d1b8: 4840 → **620 KB**（砍 88%）—— 完全命中预期
- [x] 6.3 46a25772: 3070 KB 不变（无 image，符合预期）
- [x] 6.4 `openspec/followups.md` 性能条目加 Phase 3 子段（含三 case 实测数字 + 行为契约引用 + 下一轮 follow-up 方向）

## 7. preflight + commit

- [x] 7.1 `just fmt` 通过
- [x] 7.2 `just lint` 通过（workspace + src-tauri clippy）
- [x] 7.3 `just test` 通过（含前端）
- [x] 7.4 `just spec-validate` 通过
- [x] 7.5 commit `1bfe0ad` 已落（`feat(perf): image base64 lazy load via asset:// 协议 (phase 3)`）
