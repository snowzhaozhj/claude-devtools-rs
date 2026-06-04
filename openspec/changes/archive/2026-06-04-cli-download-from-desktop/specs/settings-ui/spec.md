## MODIFIED Requirements

### Requirement: Settings Section 导航

Settings 页面 SHALL 包含 section tab 导航。SHALL 支持 General、Display、Notifications、Connection、CLI、Keyboard、Diagnostics、About 八个 section。Connection section SHALL 仅在 Tauri 桌面 host 渲染；CLI section SHALL 在所有平台渲染。

#### Scenario: 默认显示 General section

- **WHEN** Settings 页面首次打开
- **THEN** SHALL 默认显示 General section

#### Scenario: 切换到 CLI section

- **WHEN** 用户点击 CLI section tab
- **THEN** SHALL 显示 CLI 安装状态、版本信息和操作按钮

#### Scenario: standalone 模式隐藏 Connection tab

- **WHEN** Settings 页面在 HTTP standalone 模式下渲染（无 Tauri runtime）
- **THEN** Section 导航 SHALL NOT 渲染 Connection tab
- **AND** 即使 URL 含 `#connection` hash，SHALL 静默回退到 General section

## ADDED Requirements

### Requirement: CLI Section 状态展示

CLI section SHALL 展示当前 CLI 安装状态。状态来自启动时异步检测的内存缓存结果，打开 Settings 时 SHALL 立即可见（不显示加载态）。

#### Scenario: CLI 未安装

- **WHEN** CLI section 渲染
- **AND** CLI 状态为 `not_installed`
- **THEN** SHALL 显示"安装 CLI vX.Y.Z"按钮
- **AND** SHALL 显示安装目标路径 `~/.local/bin/cdt`
- **AND** 按钮使用 SettingsButton 组件

#### Scenario: CLI 已安装且版本与桌面端一致

- **WHEN** CLI section 渲染
- **AND** CLI 状态为 `installed_current`
- **THEN** SHALL 显示绿色 ✓ + 版本号 + 路径
- **AND** 版本号和路径使用 mono 字体

#### Scenario: CLI 已安装但版本落后

- **WHEN** CLI section 渲染
- **AND** CLI 版本 < 桌面端版本
- **AND** CLI 路径为 `~/.local/bin/cdt`
- **THEN** SHALL 显示当前版本 + "更新到 vX.Y.Z" 按钮

#### Scenario: CLI 安装在受管路径但不在 PATH 中

- **WHEN** CLI section 渲染
- **AND** CLI 状态为 `installed_not_in_path`
- **THEN** SHALL 显示版本 + 路径
- **AND** SHALL 显示 PATH 添加指令（`export PATH="$HOME/.local/bin:$PATH"`）
- **AND** PATH 指令使用 mono 字体 + 可复制

#### Scenario: CLI 由外部管理

- **WHEN** CLI section 渲染
- **AND** CLI 路径不是 `~/.local/bin/cdt`
- **THEN** SHALL 显示版本 + 路径 + "由外部管理" 提示
- **AND** SHALL NOT 显示安装或更新按钮

### Requirement: CLI Section 安装/更新交互

CLI section 的安装/更新按钮 SHALL 提供明确的进行中、成功、失败三态反馈。

#### Scenario: 安装进行中

- **WHEN** 用户点击安装/更新按钮
- **THEN** 按钮 SHALL 变为 disabled 状态
- **AND** SHALL 显示 inline spinner（10×10，Focus Blue）+ "安装中..." 文案
- **AND** SHALL NOT 阻塞 Settings 页面其他交互

#### Scenario: 安装成功

- **WHEN** 安装流程完成且验证通过
- **THEN** SHALL 立即刷新 CLI 状态展示为 `installed_current`
- **AND** 如果 `~/.local/bin` 不在 PATH 中，SHALL 切换到 `installed_not_in_path` 状态展示 PATH 指令

#### Scenario: 安装失败

- **WHEN** 安装流程中任一步骤失败
- **THEN** SHALL 使用 warning 色（Amber）显示错误信息
- **AND** SHALL 显示"重试"按钮
- **AND** SHALL NOT 使用 error red（下载失败是 actionable 状态，非系统错误）
