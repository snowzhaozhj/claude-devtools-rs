## ADDED Requirements

### Requirement: General Section SHALL render Browser Access subsection in Tauri runtime

General section SHALL 在 Tauri runtime 下额外渲染一个 "Browser Access" 子区块，包含三部分内容：

1. **`SettingsToggle` 切换** "Enable server mode"（受 `httpServer.enabled` 驱动），副文案 SHALL 为 "Start an HTTP server to access the UI from a browser or embed in iframes"
2. **运行状态行**：当 `http_server_status` 返回 `running: true` 时 SHALL 显示绿点 + `Running on http://localhost:{port}` + 一个 "Copy URL" 按钮（点击复制 URL 到剪贴板）；`running: false` 但 `enabled: true` 时 SHALL 显示警告文案（如 "Failed to start: port may be in use"）
3. **端口输入**：可点击编辑当前端口（`SettingsTextInput` 数字输入），保存时 SHALL 调 `update_config` 把 `httpServer.port` 持久化；server 已运行时改端口 SHALL 提示用户需先关再开生效（或后端实现重启逻辑——本 change 只规约 UI 行为，重启与否在实现期决定，**但行为 SHALL 一致可预测**）

整个 "Browser Access" 子区块 SHALL **仅**在 Tauri runtime 渲染（前端通过 `window.__TAURI_INTERNALS__` 检测）；浏览器 runtime 加载时 SHALL 隐藏该子区块——浏览器中的用户已经在用 server，再展示一个开关无意义且会让用户在浏览器里关闭 server 后失联。

切换 toggle 操作 SHALL 调 `http_server_start({ port })`（开启）或 `http_server_stop()`（关闭）IPC；操作进行中（pending）SHALL 把 toggle 设为 disabled 防止重复点击，操作返回错误 SHALL 用 inline 错误提示而非 toast（保留持续可见以便用户改 port）。

#### Scenario: Tauri runtime 默认隐藏 Browser Access 状态行

- **WHEN** Settings General section 在 Tauri runtime 渲染，`httpServer.enabled = false`
- **THEN** "Browser Access" 子区块 SHALL 显示标题 + toggle off 状态 + 端口输入框 + 副文案
- **AND** SHALL **不**显示绿点 / Running URL / Copy 按钮

#### Scenario: Toggle 开启后展示运行 URL

- **WHEN** 用户在 Tauri runtime Settings 中点击 "Enable server mode" toggle，IPC 启动成功
- **THEN** UI SHALL 显示绿点 + `Running on http://localhost:3456`（或当前 port）+ "Copy URL" 按钮
- **AND** toggle SHALL 显示为开启状态

#### Scenario: Copy URL 按钮复制到剪贴板

- **WHEN** server 运行中，用户点击 "Copy URL" 按钮
- **THEN** UI SHALL 把 `http://localhost:{port}` 写入系统剪贴板
- **AND** SHALL 给一个临时视觉反馈（如按钮文案短暂变 "Copied"）

#### Scenario: 启动失败 inline 错误展示

- **WHEN** 用户开 toggle，IPC 返回端口冲突错误
- **THEN** toggle SHALL 自动回到 off 状态
- **AND** 子区块内 SHALL 出现 inline 错误文案描述冲突 + 建议改 port
- **AND** 错误文案 SHALL 保持显示直到用户改 port 或再次尝试（**不**自动消失）

#### Scenario: 浏览器 runtime 隐藏 Browser Access 子区块

- **WHEN** 用户从 Chrome 浏览器加载 Settings 页面，`window.__TAURI_INTERNALS__` 不存在
- **THEN** Settings General section SHALL **不**渲染 "Browser Access" 子区块
- **AND** 其它 General 配置项（theme / Claude root 等）SHALL 正常渲染
