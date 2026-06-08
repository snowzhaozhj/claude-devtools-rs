## Decisions

### D1: 导出渲染在前端，写文件走 IPC

**候选方案**：
- A: Rust 后端生成 HTML/Markdown（需加 template engine + syntect）
- B: 前端生成内容字符串 → IPC command 写文件（复用 marked + highlight.js）

**选择 B**。理由：
- 前端已有 `marked`、`highlight.js`（40+ 语言）、`contextMenu/markdown.ts`（chunk→md 序列化）
- 零新 Rust 依赖
- HTML 导出的视觉效果和 app 内一致（共享 `app.css` 变量）
- 大 session 前端异步生成不阻塞 UI（Web Worker 如需要可后续加）

**风险**：超大 session（>10k 消息）前端序列化可能卡顿 200-500ms。缓解：用 `requestIdleCallback` 分片或提示用户等待。

### D2: HTML 文件自包含（Single-file, 轻交互）

**候选方案**：
- A: 纯静态 HTML（无 JS）
- B: 内嵌轻量 JS（~5KB），提供折叠/搜索/主题切换/目录导航
- C: 多文件导出（HTML + CSS + JS 分离）

**选择 B**。理由：
- 分享场景下接收者需要快速定位内容（折叠 + 搜索是硬需求）
- 单文件便于发送（Slack/邮件/IM 拖拽即可）
- 5KB JS 不影响打开速度
- 无外部依赖（不加载 CDN），完全离线可用

**交互功能清单**：
1. 工具详情（input/output）默认折叠，点击展开
2. Ctrl+F / Cmd+F 浏览器原生搜索（不重写）
3. 暗色/亮色主题切换（toggle 按钮，记忆到 localStorage）
4. 左侧目录（turn 列表），点击跳转到对应位置
5. 思考链（thinking blocks）默认折叠

### D3: UI 入口——SessionMetaMenu 扩展 + 右键菜单复用

**候选方案**：
- A: 在 UnifiedTitleBar `.zone-status` 加全局导出按钮
- B: 在 SessionMetaMenu（"..." 菜单）加导出子菜单
- C: 两者都加

**选择 B**。理由：
- 导出是 session 级操作，放在 session 上下文菜单语义明确
- SessionMetaMenu 已有 "复制 Session ID" 等相似操作，导出是自然延伸
- 避免 UnifiedTitleBar 膨胀（已有 6 个按钮/状态）
- 导出选项展开为子菜单：Markdown / JSON / HTML

右键菜单「复制为 Markdown」已存在于 `contextMenu/menu-items.ts`（单 chunk 级别）。新增：选中多个 chunks 时出现「导出选中为...」选项。

### D4: IPC command 设计——`export_save_session`（codex 二审修订）

原方案 `export_write_file(path, content)` 被 codex 否决——前端传任意 path + content 是任意文件写入原语，XSS 可升级为文件覆盖。

**修订方案：后端一次性完成 dialog + write**

```
Command: export_save_session
Input:  { defaultName: String, filterExt: String, content: String }
Output: Result<Option<String>, String>  // 返回实际写入路径或 null（用户取消）
```

后端实现：
1. 调用 `tauri_plugin_dialog::FileDialogBuilder::new().set_file_name(&default_name).add_filter(...)` 弹原生 save dialog
2. 用户选择路径后 → 校验：不是 symlink、扩展名在白名单内（.md/.json/.html）、路径非空
3. `tokio::fs::write(path, content.as_bytes())`
4. 返回实际写入的 path

**安全措施**：
- 路径只来自 Rust 端 dialog 返回值，前端无法注入任意路径
- 写入前 `symlink_metadata` 检查目标不是 symlink
- 扩展名白名单：仅允许 `.md` / `.json` / `.html`
- 错误信息脱敏：不暴露完整文件系统路径到前端

**大文件策略**（>1MB content）：
- MVP 阶段接受单次传输，加进度提示
- 超过 3MB 的 session 在 UI 上提示「大会话导出可能需要几秒」
- 后续可改为分块 IPC（export_begin/append/finish）或后端流式生成

### D5: 导出选项模型（codex 二审修订）

```typescript
interface ExportOptions {
  format: "markdown" | "json" | "html";
  includeThinking: boolean;       // 默认 true
  toolOutputMode: "full" | "truncated" | "name-only";  // 默认 "full"
  toolOutputMaxLength: number;    // truncated 时截断长度，默认 2000
  includeSubagents: boolean;      // 默认 true
}
```

**所有格式统一应用相同的 projection**（codex 指出 D6 原版 JSON 不过滤的自相矛盾）：
- 先将 SessionDetail 转为中间 `ExportDocument` 结构（统一应用 thinking/tool/subagent 过滤）
- 再由各 format exporter 渲染最终输出
- 需要原始数据时，JSON 格式提供 "Raw JSON（包含全部数据）" 显式选项

MVP 阶段默认 `toolOutputMode: "full"`（所有格式统一），不截断。HTML 格式中工具详情通过 UI 折叠管理长度，不通过截断。

后续可加 Settings 持久化用户偏好。

### D6: JSON 导出——经 projection 过滤后序列化（codex 二审修订）

JSON 导出也经过 `ExportDocument` projection：
- 默认：应用 D5 的选项过滤（thinking/tool/subagent），输出 filtered JSON
- 用户可在导出时选择 "Raw JSON"：此时跳过 projection，直接 `JSON.stringify(sessionDetail, null, 2)` 输出完整的 `SessionDetail` 对象

理由：
- 统一行为避免用户困惑（选了"排除 thinking"却 JSON 里还有）
- Raw JSON 仍可选，满足备份/调试场景

### D7: Markdown 导出格式设计

复用已有 `contextMenu/markdown.ts` 的函数作为基础，新增 session 级组装：

```markdown
# Session: {title || sessionId}

| 字段 | 值 |
|------|-----|
| Project | {projectName} |
| 分支 | {gitBranch} |
| 时间 | {startTime} — {endTime} |
| 消息数 | {messageCount} |
| Token | {totalInput} in / {totalOutput} out |

---

## Turn 1 — User

{user message content}

## Turn 2 — Assistant

{assistant text}

### Tool: Bash

```bash
$ command
```

output...

---
```

每个 turn 一个二级标题，工具调用在 turn 内用三级标题。thinking 用 blockquote 包裹（`> [thinking] ...`）。

### D8: 选区导出交互

**交互流程**：
1. 用户在 SessionDetail 内框选（或 Shift+点击）多个 chunk
2. 选中状态通过 CSS highlight + 浮动 toolbar 指示
3. 浮动 toolbar 显示「复制」（剪贴板，markdown 格式）和「导出...」（触发 save dialog）
4. 未选中时 toolbar 隐藏

**MVP 简化**：第一版仅支持右键单 chunk 的「复制为 Markdown」（已有）+ SessionMetaMenu 全 session 导出。选区导出作为 Layer 2 在后续迭代实现。

### D9: HTML 导出 XSS 防护（codex 二审新增）

session 内容（用户输入、模型输出、工具输出、文件内容）MUST 视为不可信输入。

**防护措施**：
1. Markdown 内容渲染复用 `renderMarkdown()`（已含 DOMPurify sanitize）
2. 非-markdown 拼接点（标题、path、tool name、session id、TOC anchor）全部 HTML escape
3. 导出 HTML 头部加严格 CSP meta tag：`<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'">`
4. 内嵌 JS 不使用 `eval()`、`innerHTML`（使用 `textContent` 设置动态文本）
5. CSS 使用 `'unsafe-inline'` 是必须的（内嵌样式），但不允许外部加载

**测试**：添加 XSS fixture 测试——构造含 `<script>alert(1)</script>` 和 `onerror=...` 的 session 内容，验证导出 HTML 中这些被 escape。

## Architecture

```
ui/src/lib/export/
├── index.ts              # 统一入口：exportSession(detail, options) → string
├── markdownExporter.ts   # SessionDetail → Markdown string
├── jsonExporter.ts       # SessionDetail → JSON string (thin wrapper)
├── htmlExporter.ts       # SessionDetail → HTML string (self-contained)
├── htmlTemplate.ts       # HTML shell + inlined CSS + JS
└── types.ts              # ExportOptions interface

ui/src/components/
├── SessionMetaMenu.svelte  # 扩展：加导出子菜单项
└── ExportDialog.svelte     # 可选：格式选择 + 选项配置弹窗（MVP 可不用）

src-tauri/src/lib.rs        # 新增 export_write_file command
```

**数据流**：

```
SessionMetaMenu 点击 "导出为 HTML"
  │
  ▼
getSessionDetail(projectId, sessionId, null)  ← 不传 fingerprint，强制拉全量
  │
  ▼
exportSession(detail, { format: "html", ... })
  │
  ▼
htmlExporter.ts 生成自包含 HTML string
  │
  ▼
dialog.save({ title: "导出会话", defaultPath: `session-${id}.html`, filters: [...] })
  │
  ▼
invoke("export_write_file", { path, content })
  │
  ▼
Rust: tokio::fs::write(path, content.as_bytes()) → Ok(())
```
