## Context

Phase 2 (`subagent-messages-lazy-load`) 把 subagent 嵌套 chunks 全文从首屏 IPC 裁掉后，46a25772 case payload 7702 KB → 3070 KB。phase 3 perf bench 升级（commit 0c8a7a6）发现新瓶颈：**user message 内联截图 base64**。这是用户高频操作（截屏 + cmd+v 粘贴到 Claude 对话）的副产品，存在两个独立问题：

1. **IPC 跨进程传输成本**：单张截图 base64 约 600 KB，按 Tauri webview 实测 13 KB/ms 吞吐 → ~46 ms/张。7826d1b8 case 7 张 = ~322 ms 纯 IPC。
2. **JS 堆字符串占用**：base64 字符串解码 + 长驻 store 缓存吃 V8 堆。同一 session 反复滚动每张 image 至少占 600 KB × N。

`crates/cdt-core/src/message.rs:91` 早就有注释承认这点，但一直没实施。

当前 `ImageSource` 结构：
```rust
pub struct ImageSource {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub media_type: String,  // e.g., "image/png"
    #[serde(default)]
    pub data: String,        // base64 字符串
}
```

Tauri 2 提供 `asset://` 协议：webview 可以直接 `<img src="asset://localhost/...">` 加载文件系统文件，零 JS 解码、零 IPC、浏览器原生缓存。需要 `tauri.conf.json` 的 `app.security.assetProtocol` 与 `capabilities/default.json` 的 `core:asset:default` 权限配合。

## Goals / Non-Goals

**Goals:**

- 把 base64 image data 从 `get_session_detail` 默认 IPC payload 中裁掉（与 phase 2 同模式：derived flag + OMIT 常量回滚开关）。
- 提供按需懒拉的新 IPC `get_image_asset(rootSessionId, sessionId, blockId) -> String`，返回前端可直接用作 `<img src>` 的 `asset://` URL。
- 同一 image base64 内容跨调用、跨 session 自动去重（SHA256 内容寻址）。
- 前端 ImageBlock 组件只在视口内才发起 IPC 拉取（IntersectionObserver），与 lazy markdown 同节奏。
- 保持向后兼容：老后端（无 `dataOmitted` 字段）/ 回滚开关 `false` 时前端走原 `data:` URI 路径。

**Non-Goals:**

- 不主动管理 cache 目录的 GC / 容量上限——依赖 OS 自身 cache 清理策略（macOS `~/Library/Caches/`、Linux XDG cache、Windows `%LOCALAPPDATA%`），用户手动清理也可。
- 不做 image 压缩 / 转码（PNG → WebP）—— OS 缓存命中后浏览器解码已经足够快。
- 不做 HTTP API 路径同步改造（HTTP path 当前无活跃用户，phase 2 已经分叉处理）。
- 不改 assistant message 内的 image（当前 schema 中 ContentBlock::Image 主要出现在 user message 的 ToolResult 或粘贴块；如未来 assistant 也产 image 块，行为天然继承）。
- 不预先 warmup cache（不在 parse / list 时就落盘——只在懒拉时按需写）。

## Decisions

### 决策 1：选 asset:// 协议 + 落盘 cache 文件，不走 base64 IPC 透传

**选择**：image base64 落盘到 OS cache 目录，IPC 返回 `asset://` URL；前端 `<img src="asset://...">` 让浏览器原生加载。

**替代方案 A：OMIT + 按需 IPC 返回 base64 字符串**
照 phase 2 模式，但懒拉时仍跨 IPC 传 base64。
- 优点：实现最简单，无 cache 目录 / asset 协议配置。
- 缺点：单张 600 KB 仍有 ~46 ms IPC 延迟（首次点开图就感知卡顿）；JS 堆吃 600 KB 字符串；跨调用无法去重（同一图反复粘贴每次都跨 IPC）。

**替代方案 B：parse 时强制落盘 + 只存 file path**
parse session JSONL 时就把 image base64 落盘，`ImageSource` 直接存 file path。
- 优点：彻底——内存中再无 base64。
- 缺点：parse 是热路径，磁盘 IO 拖慢首屏；cache lifecycle 强耦合到 parse（session 删除时清不清？跨平台？）；可观测性下降（debug 时拿不到原始 bytes）。

**理由**：方案 C（即本决策）折中——保留 phase 2 的"OMIT + 懒拉"骨架（实现成熟），但懒拉返回的是 file URL 而不是 base64，吃透 Tauri 文件协议优势。SHA256 内容寻址解决跨调用去重问题。Parse 路径不变，cache 只在懒拉时按需创建。

### 决策 2：cache 目录用 `tauri::path::AppCacheDir` 不用 `temp_dir`

**选择**：`app_cache_dir().join("cdt-images/")`，OS 标准 cache 位置。
- macOS: `~/Library/Caches/<bundle-id>/cdt-images/`
- Linux: `~/.cache/<bundle-id>/cdt-images/`
- Windows: `%LOCALAPPDATA%/<bundle-id>/Cache/cdt-images/`

**替代方案**：`std::env::temp_dir()` 或自定义 `<config_dir>/images/`。
- temp_dir：OS 重启 / 周期清理可能丢失 → 用户重启电脑后所有图都要重新落盘。
- config_dir：与 user 配置混在一起，备份策略奇怪（config 该备份，cache 不该）。

**理由**：cache_dir 语义明确——"可丢失、可重建"。OS 自身有 cache 清理机制（macOS `Tools/System Information` 清理时会包含；Linux/Windows 各有约定）。用户手动清也容易找到。

### 决策 3：文件命名用 SHA256 内容 hash 不用随机 UUID

**选择**：`<sha256_of_base64_bytes>.<ext>`（前 16 hex 字符即可，碰撞概率 ~2^-64）。

**替代方案**：UUID v4 / blockId / `<sessionId>_<blockIndex>`。
- UUID：每次落盘新 UUID → 同图反复粘贴每次新文件，cache 膨胀。
- blockId：blockId 不稳定（可能是 chunk 内索引），且无法跨 session 去重。
- session+index：与 UUID 同问题。

**理由**：内容寻址 = 自动去重。同一截图被用户在不同 session / 不同位置粘贴 5 次 → 文件系统 1 份，IPC 也只首次落盘那次有磁盘 IO。返回时 `if path.exists() { return url; }` 直接命中。

### 决策 4：用 blockId 定位 image，不用 base64 hash 反查

**选择**：`get_image_asset(rootSessionId, sessionId, blockId)`，前端在拿到 OMIT 后的 chunks 时记录每个 ImageBlock 的稳定 ID（用 chunk uuid + content block index 拼出）。

**替代方案**：前端把 hash 算出来传给后端 → 后端按 hash 找文件。
- 但前端拿到的 OMIT 后 `data=""`，根本算不出 hash。
- 让前端自己重新拉 base64 再算 hash → 形成循环。

**理由**：blockId 是定位锚点，后端按 (sessionId, blockId) 重新 parse 那条 message 拿原始 base64 → 落盘。前端不需要也不应该看到 base64。`blockId` 编码方案：`<chunk_uuid>:<block_index>`（chunk uuid 在 OMIT 后仍保留）。

### 决策 5：cache 不做主动清理 / 不做容量上限

**选择**：写入后不管理。OS 清理 + 用户手动清理为主。

**替代方案**：LRU 上限 / 启动时扫描清理 / session 删除时联动清理。
- LRU：需要持久化访问时间，复杂度高。
- 启动清理：扫整个目录算总大小耗时不可控。
- session 联动：当前 session 删除流程没有 hook 点，且 hash 去重后无法判断"这个 image 是否还有别的 session 引用"。

**理由**：单张图 ~600 KB，1000 张才 600 MB；用户极少留万张截图。OS cache_dir 本来就是"可丢失"语义。后续如真有用户上报 cache 过大再加 cleanup（当前是过早优化）。

### 决策 6：失败 fallback 返回 data: URI 不报错

**选择**：cache 写入失败（磁盘满 / 权限错）时，`get_image_asset` 直接返回 `data:image/png;base64,<原始 base64>`，前端按 `<img src>` 一样能用。

**理由**：可用性 > 性能优化。极端环境下（CI / 容器）不至于因为 cache 写不进去就显示不出图。

### 决策 7：blockId 编码用 `<chunkUuid>:<blockIndex>`

**选择**：前端拿 chunk.uuid（UserChunk 有此字段）+ image 在 `content.blocks` 数组里的 index 拼成 `"abc-123:2"` 传给后端；后端 parse 同 jsonl 找到对应行 + 同样 index 取 image。

**替代方案**：让 `ImageSource` 自带 stable id 字段。
- 缺点：要改 cdt-core 数据结构、parse 时填充、整条数据流都要带这个字段——改动太大。

**理由**：chunk uuid + block index 是 derived key，无需改数据结构；后端 lookup 也是 O(parse 这条 message)，可接受。

## Risks / Trade-offs

- **[风险] Tauri assetProtocol scope 配错 → 前端拿到 URL 但 webview 拒绝加载** → 在 capabilities + tauri.conf.json 都精确配 cache 子目录的 glob；首次实施完写测试用例（手动开 session 看 image 显示）。文档参考 https://tauri.app/v1/api/config/#assetprotocolconfig。
- **[风险] SHA256 短 hash (16 字符 = 64 bit) 碰撞** → 单用户场景 2^32 张图才有 50% 碰撞概率，可接受；如未来发现碰撞改成全 hash (64 字符) 即可（向后兼容）。
- **[风险] 后端 `get_image_asset` 需要重新 parse 整条 message 才能拿 base64**——单次开销小（一条 message 几十 KB），但若用户连续展开 50 张图 → 50 次 parse。优化方案：在 IPC LocalDataApi 内加一层 `LruCache<(sessionId, blockId), Vec<u8>>` 作为 in-memory 快照（**不在本期实施**，留 follow-up）。
- **[风险] 老后端兼容性** → 前端按 `dataOmitted` 字段分支：`true` → 走 `getImageAsset`；`false` 或缺失 → 走原 `data:` URI 路径。`ImageSource` 加 `#[serde(default)] data_omitted: bool`，老 JSON 反序列化为 `false` 自动 fallback。
- **[trade-off] cache 文件不主动清理** → 长期可能累积 GB 级（极端场景）。决策 5 已说明：用户痛点出现再加。
- **[trade-off] HTTP path 不同步改造** → HTTP 当前无用户；如未来要支持，需要 HTTP server 注册一个 `/cache/images/<hash>` 静态文件 endpoint，与 IPC 同源指向同 cache 目录。

## Migration Plan

### 部署步骤

1. **Rust 侧无破坏性 schema 变更**：`ImageSource.data_omitted` 加 `#[serde(default)]`，老 JSON 反序列化为 `false`，老缓存 / 老 session 不受影响。
2. **前端按 `dataOmitted` 分支**：`true` 调 `getImageAsset` 走新路径；`false` 走原 `data:` URI 路径——首次升级老 build 与新 build 互不打架。
3. **Tauri 配置**：`capabilities/default.json` 加 `core:asset:default`；`tauri.conf.json` `app.security.assetProtocol.scope` 加 `<cache_dir>/cdt-images/**`。配置错误时 webview 报 403，前端 fallback 直接 `<img>` 显示破图——加日志能立即发现。
4. **首启时不预创 cache 目录**——`std::fs::create_dir_all` 在写入第一个文件时自动创建，避免用户没用 image 功能就堆空目录。

### 回滚策略

- **快速回滚**：`crates/cdt-api/src/ipc/local.rs` 顶部 `const OMIT_IMAGE_DATA: bool = true` 改为 `false`，下个 build 即恢复原行为（image data 完整 IPC 传输）。前端 fallback 路径自动接管。
- **彻底回滚**：移除 `data_omitted` 字段需要新 change（向前兼容字段不该轻易拿掉）；如真有需要按 spec delta `REMOVED` 走流程。

## Open Questions

- 是否需要在 `get_image_asset` 后端加 `LruCache<(sessionId, blockId), Vec<u8>>` 提速重复展开？— **不在本期**：先实测真实使用频率，找到痛点再加。
- 前端 ImageBlock 是否需要"加载失败重试"按钮？— **不在本期**：默认 `<img>` 加载失败浏览器有 broken-image 图标，用户右键可看 URL 自查；先观察是否真有失败场景。
- cache 目录 size 上限 / GC 策略？— **不在本期**：决策 5。
- assistant message 内出现 image block 时是否走同套机制？— **本期天然支持**：`get_session_detail` OMIT 遍历所有 chunks 内 ContentBlock，不区分 user/assistant；`get_image_asset` 用 (sessionId, blockId) 定位也一致。
