## 1. cdt-api 后端 IPC

- [x] 1.1 新增 memory IPC 数据类型（`ProjectMemory` / `MemoryLayer` / `MemoryFileContent`），字段按 camelCase 序列化
- [x] 1.2 实现 `LocalDataApi::get_project_memory(project_id)`，发现 `memory/` 目录、解析 `MEMORY.md`、区分 index/entry/orphan layers
- [x] 1.3 实现 `LocalDataApi::read_memory_file(project_id, file)`，限制只读同目录 `.md` 文件并拒绝目录穿越
- [x] 1.4 为 memory discovery、index parser、safe read 添加 Rust 测试
- [x] 1.5 在 `DataApi` trait / Tauri command / `EXPECTED_TAURI_COMMANDS` 中暴露 `get_project_memory` 与 `read_memory_file`

## 2. 前端 API 与状态

- [x] 2.1 在 `ui/src/lib/api.ts` 增加 memory 类型与 `getProjectMemory` / `readMemoryFile` 调用封装
- [x] 2.2 在 `ui/src/lib/tauriMock.ts` 与 fixture 中加入 memory mock command 和样例数据
- [x] 2.3 扩展 `tabStore.svelte.ts`，加入 `memory` tab type 与 project-scoped singleton 打开逻辑
- [x] 2.4 同步 `TabBar` / `PaneView` / tab 清理逻辑，确保 `memory` tab 能正确渲染和关闭

## 3. Memory UI

- [x] 3.1 在 Sidebar 当前项目区域加载 memory summary，并在有 layers 时显示 `Memory (N)` 入口
- [x] 3.2 新增 `MemoryView.svelte`，实现左侧 layers 列表、右侧 Markdown 预览、loading/error/empty 状态
- [x] 3.3 实现顶部 Copy 按钮与文件下拉切换，复制当前 Markdown 原文并展示反馈
- [x] 3.4 复用现有 `renderMarkdown` / 主题 CSS，确保 Markdown 与代码块在浅/深色主题下可读

## 4. 验证与 OpenSpec

- [x] 4.1 添加/更新 Rust IPC contract 测试，断言 command 清单与 camelCase memory JSON
- [x] 4.2 添加 UI 单测或 Playwright user story，覆盖 Sidebar Memory 入口、打开 tab、切换文件、复制按钮状态
- [x] 4.3 运行 `cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、相关 Rust tests、`npm run check --prefix ui`、相关 UI tests
- [x] 4.4 运行 `openspec validate memory-viewer --strict`，并在实现完成后勾选本文件任务
