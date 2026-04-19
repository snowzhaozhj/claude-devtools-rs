## Why

`subagent-messages-lazy-load` 已落地后实测仍然有 case 卡顿。`crates/cdt-api/tests/perf_get_session_detail.rs` 升级版（commit 0c8a7a6）按 chunk 类型 + content block 类型两层细分后定位真凶：**user message 内联截图的 base64 data 是新瓶颈**——

| session | RAW | IPC OMIT (phase 2) | image blocks | image bytes | image 占 RAW |
|---------|-----|--------------------|-------------|-------------|-------------|
| 4cdfdf06 | 3472 KB | 1768 KB | 2 | 1253 KB | 36% |
| 7826d1b8 | 5161 KB | **4840 KB** | 7 | **4220 KB** | **82%** |
| 46a25772 | 7702 KB | 3070 KB | 0 | 0 | 0% |

7826d1b8 case 平均 603 KB/张内联截图，phase 2 的 `OMIT_SUBAGENT_MESSAGES` 完全没覆盖（image 在 user chunk 不在 subagent.messages）。`crates/cdt-core/src/message.rs:91` 早写过注释"base64 数据会完整保留在 `data` 字段里，下游若关心内存占用，应尽早丢弃或替换为引用"——这条警告一直没人遵守。Tauri webview 的优势恰在于 `asset://` 文件协议浏览器原生解码（零 JS 开销），不利用就是浪费。

## What Changes

- **MODIFIED**：`cdt-core::ImageSource` 加 `#[serde(default)] data_omitted: bool` 字段（与 `subagent-messages-lazy-load` 的 `messagesOmitted` 同模式）。`data` 字段保留语义不变；`data_omitted=true` 表示首屏被裁剪需要后续懒拉。
- **MODIFIED**：`ipc-data-api` capability 既有 `Expose project and session queries` Requirement —— `get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `ContentBlock::Image.source.data` 默认 SHALL 被替换为空字符串、`dataOmitted=true`。回滚开关 `OMIT_IMAGE_DATA: bool = true` 一行切回。
- **ADDED**：`ipc-data-api` capability 新增 `Lazy load inline image asset` Requirement —— 新 IPC `get_image_asset(rootSessionId, sessionId, blockId) -> String`，按需把 base64 落盘到 OS 标准 cache 目录（`tauri::path::app_cache_dir()/cdt-images/<sha256>.<ext>`）并返回 Tauri `asset://` URL。SHA256 内容 hash 命名 → 同一截图多次粘贴自动跨调用去重。
- **MODIFIED**：`session-display` capability 既有 `Markdown rendering for chunk content` Requirement（或新增 `Inline image lazy load` Requirement）—— 前端 ImageBlock 组件 SHALL 用 IntersectionObserver 进视口才调 `getImageAsset`，加载完成后用 `<img src={assetUrl}>` 浏览器原生加载（不再用 `data:` URI）。`dataOmitted=false`（回滚或老后端）SHALL 直接用原 `data:` URI 路径不发额外 IPC。

## Capabilities

### New Capabilities

无

### Modified Capabilities

- `ipc-data-api`：`get_session_detail` 返回值修改（image data 默认裁剪）；新增 `get_image_asset` IPC。
- `session-display`：ImageBlock 视口懒拉行为；`asset://` URL 替代 `data:` URI 渲染路径。

## Impact

- **代码**
  - `crates/cdt-core/src/message.rs`：`ImageSource` 加 `data_omitted: bool`（`#[serde(default)]` 不破坏反序列化）；roundtrip 测试同步更新。
  - `crates/cdt-api/src/ipc/local.rs::get_session_detail`：序列化前遍历 chunks 内所有 `ContentBlock::Image`，把 `source.data` 替换为空、设 `data_omitted=true`。顶部 `const OMIT_IMAGE_DATA: bool = true` 回滚开关。
  - `crates/cdt-api/src/ipc/traits.rs` + `local.rs`：新增 `get_image_asset(root_session_id, session_id, block_id) -> Result<String, ApiError>` 方法。LocalDataApi 实现：parse 目标 jsonl → 找到 block_id 对应的 ImageBlock → SHA256(base64 raw bytes) → 落盘 `<cache_dir>/cdt-images/<hash>.<ext>` → 返回 `tauri::path::AppHandle.asset_url()` 转换后的字符串。失败 fallback 返回 `data:image/png;base64,...`（保活）。
  - `src-tauri/src/lib.rs`：注册新 Tauri command `get_image_asset`。
  - `src-tauri/capabilities/default.json`：加 `asset:default` 与 `asset` protocol scope（指向 cache 子目录）。
  - `src-tauri/tauri.conf.json`：`assetProtocol.scope` 加 cache 子目录的 glob。
  - `ui/src/lib/api.ts`：新增 `getImageAsset(rootSessionId, sessionId, blockId): Promise<string>`；TS `ImageSource` 类型加 `dataOmitted` 字段。
  - `ui/src/components/ImageBlock.svelte`（新增或现有渲染逻辑改造）：`{@attach}` 挂 IntersectionObserver，进视口调 `getImageAsset` 拿 URL，`<img src={url}>`；fallback 老路径直接用 `data:` URI。
- **依赖**：新增 `sha2`（workspace 已有则复用；无则加 workspace dep）。
- **HTTP API**：当前 HTTP path 无活跃用户，按 phase 2 做法保留同步完整返回；`get_image_asset` 走 IPC-only。
- **测试**：
  - Rust 单元：`ImageSource` roundtrip 含新字段；`get_session_detail` 返回的 chunks 中所有 ImageBlock `data=""` + `dataOmitted=true`；`get_image_asset` 命中已有 cache 文件不重复落盘；同 hash 内容跨 session 共享同一文件。
  - perf bench 重跑确认收益（4cdfdf06 / 7826d1b8 image-heavy case payload 大幅下降）。
  - 前端 `npm run check --prefix ui` 通过；UI 行为人工验证（截图首屏 placeholder → 进视口加载 → 滚动复用浏览器缓存）。
- **回滚**：`OMIT_IMAGE_DATA: bool = false` 即恢复完整 base64 payload；前端 fallback 路径仍生效。
- **预期收益**：7826d1b8 case IPC 4840 KB → ~620 KB（砍 87%，est ipc 372 → 47ms）；4cdfdf06 case 1768 KB → ~515 KB（砍 71%，est ipc 136 → 40ms）；46a25772 case 不受影响（无 image）。后端 `Vec<Chunk>` 内存常驻量同步降低（base64 不再驻留）。
