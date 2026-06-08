## Why

用户需要将 Claude Code 会话分享给团队成员（review、复盘、审计、写 postmortem），当前没有导出功能。TS 原版有基础的 Markdown/JSON/TXT 导出但存在多项限制：工具输出截断、无交互 HTML、无原生保存对话框、无选区复制、无子代理展开。

Rust 端口有天然优势：Tauri 原生 save dialog、后端流式写不阻塞 UI、highlight.js + marked 已在前端、完整 CSS 主题变量可内嵌。这是一个值得做到比 TS 原版更好的功能。

## What Changes

新增 `session-export` capability，提供三种格式的会话导出 + 选区复制能力：

### 导出格式

1. **Markdown** — 结构化标题 + 元数据表 + 代码块，适合贴 Issue/PR/Slack
2. **JSON** — 完整 SessionDetail 序列化（可配精简/完整），适合备份和机器消费
3. **HTML** — 自包含单文件（内嵌 CSS + 轻量 JS ~5KB），可在浏览器中交互浏览：折叠/展开工具详情、暗亮主题切换、搜索、目录导航跳转到指定 turn

### 导出选项（所有格式通用）

- 思考链（thinking blocks）：包含 / 排除
- 工具输出：完整 / 截断（默认 2000 字符）/ 仅工具名
- 子代理内容：展开 / 折叠为摘要

### UI 入口

- **TabBar 工具栏**：导出按钮（下载图标），点击展开格式选择下拉
- **Chunk 右键菜单**：「复制为 Markdown」复制单个/多个选中 chunk 到剪贴板
- **选区导出**：在 SessionDetail 中选中多个 turns 后，toolbar 出现「导出选中」按钮

### 文件保存

使用 Tauri 原生 save dialog（`@tauri-apps/plugin-dialog` 的 `save()`），不走 Blob hack。后端 IPC command 负责写文件。

## Capabilities

### New Capabilities

- `session-export`: 会话导出的行为契约——格式、选项、交互、错误处理

### Modified Capabilities

无。右键「复制为 Markdown」已有（归属 `session-display`），选区导出推迟到后续迭代。

## Impact

- **前端**：新增 `ui/src/lib/export/` 模块（格式化器 + HTML 模板 + 导出 UI 组件）
- **后端**：`cdt-api` 新增 1 个 IPC command `write_export_file(path, content, encoding)`
- **IPC**：新增 `write_export_file` command（单向写入，无返回 payload）
- **依赖**：无新 Rust crate；前端无新 npm 依赖（复用 marked + highlight.js）
- **性能**：大 session 导出走 get_session_detail 全量拉取（已有 content_mode='full' 路径），HTML 生成在前端异步执行不阻塞 UI
- **文件大小**：HTML 自包含文件含内嵌 CSS (~4KB gzip) + JS (~5KB gzip)，10k 消息 session 预估 ~2-5MB
