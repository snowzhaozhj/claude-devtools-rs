## ADDED Requirements

### Requirement: General section "Use WSL" 按钮

General section SHALL 在 `claudeRootPath` 输入控件下方提供 "Use WSL" 按钮。该按钮 SHALL 仅在 Windows 平台显示。点击按钮 SHALL 触发 `list_wsl_distros` IPC 调用，IPC 返回结构为 `WslDistroScanReport { candidates, distrosWithoutHome }`，UI SHALL 按以下分支处理：

- `candidates.length == 1`：SHALL 自动调用 `update_config("general", { claudeRootPath: candidates[0].claudeRootPath })`，并通过 toast 或 inline 文案提示已切换
- `candidates.length >= 2`：SHALL 显示 distro 选择 modal（apply 阶段新建的共享 `Modal.svelte` 组件），用户选择后 SHALL 调用 `update_config` 应用所选 candidate
- `candidates.length == 0 && distrosWithoutHome.length == 0`：SHALL 在按钮下方显示 inline 文案"未检测到 WSL distro"
- `candidates.length == 0 && distrosWithoutHome.length > 0`：SHALL 在按钮下方显示 inline 文案"检测到 WSL distro 但无法解析 home"，并附 `distrosWithoutHome` 的 distro 名列表（用于排查）
- IPC 调用失败：SHALL 显示 inline 错误提示

distro 选择 modal SHALL 在每个候选项展示 `distro` 名、`claudeRootPath` UNC 路径，并在 `claudeRootExists = false` 时附加视觉提示（例"该 distro 内尚无 Claude 数据"）但**不**禁用选择。

#### Scenario: 非 Windows 平台不显示按钮

- **WHEN** Settings 页面在 macOS 或 Linux 上渲染
- **THEN** General section SHALL NOT 渲染 "Use WSL" 按钮

#### Scenario: Windows 平台单 distro 自动应用

- **WHEN** Windows 用户点击 "Use WSL" 按钮
- **AND** `list_wsl_distros` 返回单个 candidate `{ distro: "Ubuntu", claudeRootPath: "\\\\wsl.localhost\\Ubuntu\\home\\alice\\.claude", ... }`
- **THEN** 系统 SHALL 调用 `update_config("general", { claudeRootPath: "\\\\wsl.localhost\\Ubuntu\\home\\alice\\.claude" })`
- **AND** SHALL 在 UI 显示成功提示
- **AND** SHALL NOT 弹出 modal

#### Scenario: Windows 平台多 distro 弹选择

- **WHEN** Windows 用户点击 "Use WSL" 按钮
- **AND** `list_wsl_distros` 返回 `["Ubuntu", "Debian-12"]` 两个 candidate
- **THEN** SHALL 弹出 distro 选择 modal
- **AND** modal SHALL 列出两个 candidate 的 distro 名与 `claudeRootPath`
- **AND** 用户选定 `Debian-12` 并确认后 SHALL 调用 `update_config("general", { claudeRootPath: "\\\\wsl.localhost\\Debian-12\\..." })`

#### Scenario: distro 内尚无 Claude 数据

- **WHEN** modal 渲染某 candidate 时 `claudeRootExists = false`
- **THEN** SHALL 在该候选行显示视觉提示文案（例"该 distro 内尚无 Claude 数据"）
- **AND** SHALL 仍允许用户选择该候选

#### Scenario: WSL 未检测到

- **WHEN** Windows 用户点击 "Use WSL" 按钮
- **AND** `list_wsl_distros` 返回 `{ candidates: [], distrosWithoutHome: [] }`
- **THEN** SHALL 在按钮下方显示 inline 文案"未检测到 WSL distro"
- **AND** SHALL NOT 弹出 modal
- **AND** SHALL NOT 调用 `update_config`

#### Scenario: 检测到 distro 但全部 home 解析失败

- **WHEN** Windows 用户点击 "Use WSL" 按钮
- **AND** `list_wsl_distros` 返回 `{ candidates: [], distrosWithoutHome: ["Ubuntu", "Debian-12"] }`
- **THEN** SHALL 在按钮下方显示 inline 文案"检测到 WSL distro 但无法解析 home"
- **AND** 文案 SHALL 包含 `Ubuntu` 与 `Debian-12` 的 distro 名供用户排查
- **AND** SHALL NOT 弹出 modal
- **AND** SHALL NOT 调用 `update_config`

#### Scenario: IPC 调用失败

- **WHEN** Windows 用户点击 "Use WSL" 按钮
- **AND** `list_wsl_distros` IPC 调用返回错误
- **THEN** SHALL 在按钮下方显示 inline 错误提示
- **AND** SHALL NOT 弹出 modal
- **AND** SHALL NOT 调用 `update_config`
