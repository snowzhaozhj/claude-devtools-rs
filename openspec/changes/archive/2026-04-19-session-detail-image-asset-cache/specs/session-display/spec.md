# session-display Spec Delta

## ADDED Requirements

### Requirement: Inline image lazy load via asset protocol

User message 内的 `ContentBlock::Image` 块 MUST 通过视口懒加载渲染：首屏不携带 base64 字符串（后端 `get_session_detail` 默认裁剪，`source.dataOmitted=true`），ImageBlock 组件 SHALL 用 `IntersectionObserver`（与 lazy markdown 同节奏，`rootMargin: 200px`）监听自身 DOM 节点，进入视口才调用 `getImageAsset(rootSessionId, sessionId, blockId)` 拉取 Tauri `asset://` URL，赋值到 `<img src>` 由浏览器原生加载。

行为约束：
- 加载完成前 SHALL 显示占位（如固定高度 + media type 文案的 placeholder div），避免布局抖动。
- `dataOmitted=false`（回滚开关或老后端）时 SHALL 直接用 `data:<media_type>;base64,<source.data>` URI 路径，SHALL NOT 调 `getImageAsset`。
- 同一 ImageBlock 重复进出视口 SHALL 复用首次拉取的 URL（前端组件级缓存或 Svelte `$state` 留存）。
- `getImageAsset` 失败（IPC 异常 / 后端返回 fallback `data:` URI）时 SHALL 直接把返回值赋给 `<img src>`——浏览器渲染失败时显示 broken-image 图标即可，不需额外重试 UI。
- `blockId` 由前端从 chunk 内 ContentBlock 数组拼接：`<chunkUuid>:<blockIndex>`（chunkUuid 取所属 UserChunk / AIChunk response 的 uuid；blockIndex 是 image 在 `MessageContent::Blocks` 中的位置）。

#### Scenario: 首屏不加载视口外的 image

- **WHEN** SessionDetail 首屏渲染，含 5 个 ImageBlock，其中只有最上面 1 个在视口内
- **THEN** 仅视口内那 1 个 ImageBlock SHALL 调用 `getImageAsset`
- **AND** 其余 4 个 SHALL 显示占位 div，`<img>` 元素的 `src` SHALL 为空或未设置

#### Scenario: 滚动进入视口时按需加载

- **WHEN** 用户向下滚动使一个原本不在视口的 ImageBlock 进入视口
- **THEN** 该 ImageBlock SHALL 调用一次 `getImageAsset`，拿到 URL 后赋给 `<img src>`，浏览器加载并显示图片
- **AND** SHALL NOT 再次调用 `getImageAsset`（即使再次进出视口）

#### Scenario: 老后端 / 回滚开关 fallback 到 data URI

- **WHEN** ImageBlock 的 `source.dataOmitted` 为 `false` 或字段缺失，且 `source.data` 非空
- **THEN** ImageBlock SHALL 直接用 `data:<media_type>;base64,<source.data>` 作为 `<img src>`
- **AND** SHALL NOT 调用 `getImageAsset`

#### Scenario: 加载失败显示 broken-image 占位

- **WHEN** `getImageAsset` 返回的 URL `<img>` 加载失败（404 / asset 协议拒绝 / 数据损坏）
- **THEN** 浏览器原生 broken-image 图标 SHALL 显示，UI 不报错也不崩溃
- **AND** 用户 SHALL 能继续浏览 session 其他内容
