## 1. 后端 IPC command

- [x] 1.1 在 `src-tauri/src/lib.rs` 新增 `export_save_session` command：后端弹 save dialog + 校验 + 写文件
  - 接收 `defaultName: String, filterExt: String, content: String`
  - 调用 `tauri_plugin_dialog` 弹原生 save dialog
  - 校验：非 symlink、扩展名白名单（.md/.json/.html）、路径非空
  - `tokio::fs::write` 写入
  - 返回 `Result<Option<String>, String>`（写入路径或 null）
- [x] 1.2 注册到 `invoke_handler!` 的 `generate_handler![]` 列表
- [x] 1.3 同步 IPC contract 计数测试（`EXPECTED_TAURI_COMMANDS` / `KNOWN_TAURI_COMMANDS`）

## 2. 前端导出核心模块

- [x] 2.1 创建 `ui/src/lib/export/types.ts`：ExportOptions + ExportFormat 类型定义
- [x] 2.2 创建 `ui/src/lib/export/markdownExporter.ts`：SessionDetail → Markdown string
  - 复用 `contextMenu/markdown.ts` 的 `userChunkToMarkdown` / `aiChunkToMarkdown` / `toolExecToMarkdown`
  - 新增 session 级元数据表 + turn 编号 + thinking 可选
- [x] 2.3 创建 `ui/src/lib/export/jsonExporter.ts`：SessionDetail → JSON string（`JSON.stringify(detail, null, 2)`）
- [x] 2.4 创建 `ui/src/lib/export/htmlExporter.ts`：SessionDetail → 自包含 HTML string
  - 内嵌 CSS（从 app.css 提取核心变量 + markdown/code 样式）
  - 内嵌 JS（折叠/展开 toggle、暗亮切换、目录导航跳转）
  - 调用 marked + highlight.js 渲染 markdown 和代码块
- [x] 2.5 创建 `ui/src/lib/export/htmlTemplate.ts`：HTML 外壳模板（head/body/script/style 骨架）
- [x] 2.6 创建 `ui/src/lib/export/index.ts`：统一入口 `exportSession(detail, options)` 分发到具体 exporter

## 3. UI 入口——SessionMetaMenu 扩展

- [x] 3.1 扩展 `SessionMetaMenu.svelte`：在 "复制 Session ID" 下方加分隔线 + 3 个导出菜单项（Markdown / JSON / HTML）
- [x] 3.2 实现导出操作流程：点击 → getSessionDetail(全量) → 调 exporter → save dialog → write_export_file IPC
- [x] 3.3 导出进行中显示 loading 状态（替换菜单项文字为"导出中..."，防止重复点击）
- [x] 3.4 导出完成/失败显示 toast 反馈（复用已有 setFeedback 机制）

## 4. HTTP 模式兼容

- [x] 4.1 HTTP 模式下（非 Tauri runtime）使用浏览器 Blob + `<a download>` 降级方案
- [x] 4.2 检测 `isTauriRuntime()`：Tauri 走 save dialog + IPC；浏览器走 Blob download

## 5. 测试

- [x] 5.1 Markdown exporter 单元测试（vitest）：验证元数据表、turn 格式、thinking 包含/排除、工具截断
- [x] 5.2 JSON exporter 单元测试：验证输出是合法 JSON、包含预期字段、projection 过滤生效
- [x] 5.3 HTML exporter 单元测试：验证输出包含 `<html>`、内嵌 CSS/JS、chunk 内容渲染正确
- [x] 5.4 HTML XSS fixture 测试：构造含 `<script>alert(1)</script>` 和 event handler 的 session 内容，验证导出 HTML 中被 escape
- [x] 5.5 IPC contract test：验证 `export_save_session` command 序列化契约（defaultName + filterExt + content 字段名）

## 6. 发布
- [ ] 6.1 push 分支 + 开 PR
- [ ] 6.2 wait-ci 全绿
- [ ] 6.3 codex + pr-review-toolkit 二审通过
- [ ] 6.4 archive change
