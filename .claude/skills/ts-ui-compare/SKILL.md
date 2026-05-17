---
name: ts-ui-compare
description: 对比原版 TS renderer 组件（`../claude-devtools/src/renderer/`）与 Rust 版 Svelte 组件（`ui/src/`）的功能 / 样式 / 交互差距，输出差异报告 + 移植建议。**仅**用户显式 `/ts-ui-compare <组件名>` 时触发——模型不能自主调用，因为这是一个需要逐文件细读的开销大动作，且 CLAUDE.md 已默认 UI 改动"优先对齐原版"（feedback_align_with_original 记忆），通常不需要专门跑这个 skill 才能知道方向。
model: sonnet
disable-model-invocation: true
---

# ts-ui-compare

只在用户显式调用时跑——这是个慢、读多、写少的对比 skill，平时改 UI 直接 grep 原版即可。

## 输入

一个组件 / 功能名，例如 `Sidebar`、`BaseItem`、`ToolViewer`、`SearchBar`。
无参数时列出可对比的组件清单（下方映射表）。

## 路径约定

- Rust 版前端：`/Users/zhaohejie/RustroverProjects/Project/claude-devtools-rs/ui/src/`
- 原版 renderer：`/Users/zhaohejie/RustroverProjects/claude-devtools/src/renderer/`
- 原版 shared utils：`/Users/zhaohejie/RustroverProjects/claude-devtools/src/shared/utils/`

## 常见组件映射（持续维护——发现漂移就更新表格）

| 功能 | 原版 TS (renderer/) | Rust 版 Svelte (ui/src/) |
|------|---------------------|--------------------------|
| Sidebar | components/sidebar/ | components/Sidebar.svelte |
| Session item | components/sidebar/SessionItem.tsx | components/Sidebar.svelte（内嵌）|
| BaseItem | components/session/BaseItem.tsx | components/BaseItem.svelte |
| Tool viewers | components/session/tools/ | components/tool-viewers/ |
| Session detail | components/session/ | routes/SessionDetail.svelte |
| Search | components/search/ | components/SearchBar.svelte + CommandPalette.svelte |
| Settings | components/settings/ | routes/SettingsView.svelte |
| Notifications | components/notifications/ | routes/NotificationsView.svelte |
| Context panel | components/context/ | components/ContextPanel.svelte + DirectoryTree.svelte |
| Tab system | components/layout/TabBar.tsx | components/TabBar.svelte |
| Dashboard | components/dashboard/（若有）| routes/DashboardView.svelte |
| Content sanitizer | shared/utils/contentSanitizer.ts | lib/toolHelpers.ts |
| Markdown render | renderer/utils/markdown*.ts | lib/render.ts |
| Diff viewer | components/diff/ | components/DiffViewer.svelte |
| Subagent card | components/subagent/ | components/SubagentCard.svelte |

若表中没有精确匹配，用 Glob 和 Grep 按关键词搜索两侧。若搜出来的 Rust 版路径与上表不一致，**在报告里指出并建议更新本 skill 的映射表**——port 后 UI 已重构多轮，表会漂移。

## 工作步骤

1. **定位两侧文件**：根据映射表或搜索找到原版和 Rust 版的对应文件。若 Rust 版不存在，明确标注"未实现"。

2. **读取并对比**：
   - 原版：读取组件结构（props、state、子组件、事件处理、样式）
   - Rust 版：读取对应 Svelte 组件
   - 对比功能差异、样式差异、交互差异

3. **输出报告**（≤ 600 字）：

```
# ts-ui-compare: <组件名>

**原版**：<文件路径列表>
**Rust 版**：<文件路径>（或"未实现"）

## 功能对比
| 功能 | 原版 | Rust 版 | 状态 |
|------|------|---------|------|
| ... | ... | ... | OK / 缺失 / 部分 |

## 样式差异
- <具体差异，带行号引用>

## 移植建议
- <优先级排序的具体建议>
- <可直接移植的代码片段或逻辑>
```

## 硬性约束

- 只读：不改任何文件
- 每个对比项必须基于实际读取的文件，不要凭记忆
- 引用文件时带行号区间
- 若用户没给组件名，列出映射表让用户选
- 发现映射表漂移：在报告末尾用"⚠️ 映射表过期"标注，让用户决定是否更新本 SKILL.md
