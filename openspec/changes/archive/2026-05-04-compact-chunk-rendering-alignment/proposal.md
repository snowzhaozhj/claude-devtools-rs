## Why

会话详情页的 Compact 与 System 气泡渲染和 Electron 原版（`../claude-devtools`）差距明显——Compact 当前只是一行 `Compact` 标签 + 裸文本，原版是带 token delta（`{pre} → {post} ({delta} freed)`）+ Phase 徽章 + 折叠交互的 amber 风格 boundary；System 当前是裸 `<pre>`，原版是左对齐气泡。差距背后既有视觉/交互的距离，也有 IPC 数据契约的缺口：后端 `ContextPhaseInfo::compaction_token_deltas` 与 `ai_group_phase_map` 数据已存在但**未透出到 SessionDetail IPC payload**，前端无从拿到 token delta / phase number。本 change 一并补 IPC 字段透出 + 视觉对齐 + 顺手修一个 Sidebar 行视觉折断 bug。

## What Changes

- **CompactChunk 透出关联 token delta + phase number**（行为契约改动）
  - `cdt-core::CompactChunk` 新增 `tokenDelta: Option<CompactionTokenDelta>` + `phaseNumber: Option<u32>`，两个字段都 `#[serde(default, skip_serializing_if = "Option::is_none")]`
  - `cdt-analyze::chunk::builder` 算法层产出时这俩字段填 `None`，由 `cdt-api::session_detail` 在组装 IPC payload 时基于 `ContextPhaseInfo` 派生填入
  - 序列化形态 SHALL 是 camelCase（`tokenDelta` / `phaseNumber`）
  - IPC contract test 覆盖新字段
- Compact chunk UI 重做（视觉/交互）
  - `ui/src/routes/SessionDetail.svelte` 的 compact 分支按 `../claude-devtools/src/renderer/components/chat/CompactBoundary.tsx` 重做：折叠 button 头（ChevronRight + Layers + "Compacted" + token delta + Phase 徽章 + 时间）+ 默认折叠 + 展开时 markdown 渲染 summaryText（max-h-96 滚动 + 左侧 2px accent border）
- System chunk UI 气泡对齐（视觉）
  - 按 `../claude-devtools/src/renderer/components/chat/SystemChatGroup.tsx` 加 `rounded-2xl rounded-bl-sm` 气泡容器，背景 `var(--chat-system-bg)`，pre 文字 `var(--chat-system-text)`
- Sidebar `.session-meta` flex 行视觉折断修复（视觉）
  - `.session-msg-count` / `.session-time` 加 `flex-shrink: 0` + `white-space: nowrap`；`.session-branch` 加 `min-width: 0` + `flex-shrink: 1`，分支名在剩余空间 ellipsis

## Capabilities

### New Capabilities
（无）

### Modified Capabilities
- `chunk-building`: ADDED 1 Requirement——`CompactChunk` 携带可选 `tokenDelta` / `phaseNumber` 派生字段（builder 算法层产 `None`，组装层后置填充）
- `ipc-data-api`: ADDED 1 Requirement——`SessionDetail.chunks` 中的 `CompactChunk` 在 IPC 组装层基于 `ContextPhaseInfo` 派生填充 `tokenDelta` / `phaseNumber`，序列化 camelCase。**用 ADDED 不用 MODIFIED**：仅新增字段透出，不改既有 `Expose project and session queries` Requirement 的语义（避 archive 顺序坑——CLAUDE.md "archive 顺序坑（多 change 同 Requirement）"）

## Impact

- **影响代码**
  - `crates/cdt-core/src/chunk.rs::CompactChunk`（加 2 字段）
  - `crates/cdt-analyze/src/chunk/builder.rs`（构造 CompactChunk 时填 `None`，多处构造点要补字段）
  - `crates/cdt-api/src/session_detail.rs`（或等价组装入口，新增派生逻辑：找 compact uuid 之后第一个 AIChunk → 查 `ai_group_phase_map`；查 `compaction_token_deltas[uuid]`）
  - `crates/cdt-api/tests/ipc_contract.rs`（IPC contract test 加 case）
  - `ui/src/lib/api.ts::CompactChunk`（interface 同步加字段）
  - `ui/src/lib/__fixtures__/multi-project-rich.ts::compactChunk`（fixture 加示例 tokenDelta + phaseNumber）
  - `ui/src/routes/SessionDetail.svelte`（compact 分支完全重做 + system 分支加气泡 CSS）
  - `ui/src/components/Sidebar.svelte`（`.session-meta` 子元素 CSS fix）

- **不影响**
  - `cdt-analyze::chunk::builder` 算法逻辑（不改 chunk 流构造规则、不改 is_meta / slash / interruption / teammate-message 任何已有语义）
  - 主 spec `chunk-building` 既有 Requirement 的算法描述（仅 ADDED，不 MODIFIED 已有）
  - AIChunk / UserChunk 的渲染样式
  - PR #38 已经决定的 Sidebar git 分支 per-session 显示位置（不反转回 SidebarHeader）

- **依赖**：无新增 crate / npm 依赖；复用现有 `lazyMarkdown` / `attachMarkdown` 渲染管线，复用 lucide 风格 SVG（在 `ui/src/lib/icons.ts` 加 `LAYERS` / `CHEVRON_RIGHT` 路径，对齐已有 SVG 常量风格）。
