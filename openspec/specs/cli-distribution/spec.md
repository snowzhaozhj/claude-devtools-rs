# cli-distribution Specification

## Purpose
TBD - created by archiving change cli-download-from-desktop. Update Purpose after archive.
## Requirements
### Requirement: 启动时异步检测 CLI 状态

桌面端启动时 SHALL 异步检测系统中 `cdt` CLI 的安装状态和版本，不阻塞 UI 渲染。检测 SHALL NOT 依赖 GUI 进程的 PATH 环境变量，而是探测固定候选路径列表 + login shell fallback。检测结果缓存在内存中供 Settings 页面即时消费。

#### Scenario: CLI 通过固定路径探测发现

- **WHEN** 应用启动
- **AND** 候选路径列表（`~/.local/bin/cdt`、`/usr/local/bin/cdt`、`/opt/homebrew/bin/cdt`、`~/.cargo/bin/cdt`）中任一文件存在
- **THEN** 系统 SHALL 用该文件的绝对路径 spawn `<path> --version` 获取版本号
- **AND** 版本号通过 semver 解析后与桌面端版本比较
- **AND** 结果包含 path 和 version 两个字段

#### Scenario: CLI 未安装

- **WHEN** 应用启动
- **AND** 所有候选路径均不存在
- **AND** login shell fallback `$SHELL -lc "which cdt"` 也未找到
- **THEN** 状态标记为 `not_installed`

#### Scenario: CLI 安装在受管路径但不在用户 shell PATH 中

- **WHEN** `~/.local/bin/cdt` 文件存在
- **AND** `$SHELL -lc "which cdt"` 返回空或返回不同路径
- **THEN** 状态标记为 `installed_not_in_path`
- **AND** 用 `~/.local/bin/cdt --version` 绝对路径获取版本号

#### Scenario: cdt --version 执行超时

- **WHEN** `cdt --version` 在 3 秒内未返回
- **THEN** 系统 SHALL kill 子进程
- **AND** 状态标记为已安装、版本未知

#### Scenario: cdt --version 输出无法解析

- **WHEN** `cdt --version` 返回但输出格式无法提取 semver 版本
- **THEN** 状态标记为已安装、版本未知（不视为错误）

### Requirement: 从桌面端安装 CLI

用户 SHALL 能通过 Settings 页面一键安装 CLI binary 到 `~/.local/bin/cdt`。安装过程 SHALL NOT 执行未最终安装的临时二进制文件。

#### Scenario: 首次安装成功

- **WHEN** 用户点击"安装 CLI"按钮
- **THEN** 系统 SHALL 创建 `~/.local/bin/` 目录（如不存在）
- **AND** 从 GitHub Release 下载当前桌面端版本对应的平台 CLI asset（总超时 60s）
- **AND** 校验 HTTP 状态码为 200 且 content-length > 0
- **AND** 解压到临时文件并验证 binary magic bytes
- **AND** 验证 binary 的 CPU 架构与当前运行平台一致（Mach-O cputype / ELF e_machine / PE Machine）
- **AND** 设置临时文件可执行权限（Unix: 0o755）
- **AND** macOS 上清除临时文件 quarantine 属性
- **AND** 通过 atomic rename 写入目标路径 `~/.local/bin/cdt`
- **AND** 刷新内存中的 CLI 状态

#### Scenario: 安装前验证失败（架构不匹配）

- **WHEN** 临时文件写入成功但 CPU 架构与当前平台不匹配
- **THEN** 系统 SHALL 删除临时文件
- **AND** SHALL NOT 修改目标路径的已有 binary
- **AND** 显示用户友好的架构不匹配错误（不含 hex bytes）

#### Scenario: 安装目录无写权限

- **WHEN** 用户点击"安装 CLI"按钮
- **AND** `~/.local/bin/` 无当前用户写权限
- **THEN** 系统 SHALL 显示明确的权限错误提示
- **AND** NOT 留下临时文件

#### Scenario: 网络下载失败

- **WHEN** 下载过程中网络中断或服务器返回非 200 状态
- **THEN** 系统 SHALL 清理临时文件
- **AND** 显示用户友好的错误信息（不含 raw URL 或内部协议细节）
- **AND** NOT 修改已有的 CLI binary（如存在）

### Requirement: 从桌面端更新 CLI

已安装 CLI 版本低于桌面端版本时，用户 SHALL 能一键更新。

#### Scenario: 更新受管路径的 CLI

- **WHEN** CLI 安装在 `~/.local/bin/cdt`
- **AND** 版本低于桌面端版本
- **AND** 用户点击"更新"按钮
- **THEN** 系统 SHALL 执行与安装相同的下载-替换-验证流程

#### Scenario: 外部管理的 CLI 不提供更新

- **WHEN** CLI 安装路径不是 `~/.local/bin/cdt`（如 `/opt/homebrew/bin/cdt`、`/usr/local/bin/cdt`）
- **THEN** Settings SHALL 展示当前版本和路径
- **AND** SHALL NOT 提供更新按钮
- **AND** 显示"由外部管理"提示

### Requirement: CLI 版本高于桌面端时不降级

系统 SHALL NOT 在 CLI 版本高于桌面端版本时提示降级或重新安装。

#### Scenario: CLI 比桌面端新

- **WHEN** 检测到 CLI 版本 > 桌面端版本
- **THEN** Settings SHALL 显示"已安装"状态（绿色）
- **AND** SHALL NOT 提示降级或重新安装

### Requirement: CLI self-update 智能路径检测

`cdt self-update` SHALL 基于写权限检测和 managed path 感知来决定升级策略，而非硬编码路径黑名单。

#### Scenario: 从 managed path 运行 self-update

- **WHEN** 用户运行 `cdt self-update`
- **AND** 当前可执行文件位于 `~/.local/bin/cdt`
- **AND** 该目录有写权限
- **THEN** 系统 SHALL 正常下载并替换当前 binary

#### Scenario: 从非 managed path 运行且 managed path 存在

- **WHEN** 用户运行 `cdt self-update`
- **AND** 当前可执行文件不在 `~/.local/bin/cdt`
- **AND** `~/.local/bin/cdt` 文件存在
- **THEN** 系统 SHALL 显示提示：检测到桌面端管理的版本在 managed path，建议通过桌面端更新或直接运行 managed path 的 self-update
- **AND** 仍然允许用户继续更新当前路径的 binary（非阻塞 warn）

#### Scenario: 从非 managed path 运行且无写权限

- **WHEN** 用户运行 `cdt self-update`
- **AND** 当前可执行文件所在目录无写权限
- **THEN** 系统 SHALL 显示权限不足错误
- **AND** 建议使用 `sudo cdt self-update` 或通过桌面端安装到 managed path

#### Scenario: 从非 managed path 运行且有写权限

- **WHEN** 用户运行 `cdt self-update`
- **AND** 当前可执行文件不在 `~/.local/bin/cdt`
- **AND** 该目录有写权限
- **AND** `~/.local/bin/cdt` 文件不存在
- **THEN** 系统 SHALL 正常下载并替换当前 binary

### Requirement: 错误信息面向用户友好化

所有面向用户的错误信息 SHALL NOT 包含 raw URL、hex bytes、内部协议细节或误导性提示。

#### Scenario: 下载失败错误不含 URL

- **WHEN** CLI self-update 或桌面端安装下载失败
- **THEN** 错误信息 SHALL NOT 包含 `github.com`、`raw.githubusercontent.com` 等域名
- **AND** SHALL 包含可操作的建议（检查网络 / 设置 token / 稍后重试）

#### Scenario: 二进制校验失败错误不含 hex

- **WHEN** 下载的文件未通过 binary magic 或架构校验
- **THEN** 错误信息 SHALL NOT 包含 hex magic bytes（如 `ca fe ba be`）
- **AND** SHALL 描述为平台不匹配或文件损坏

#### Scenario: 访问拒绝错误不含误导信息

- **WHEN** GitHub API 返回 403 Forbidden
- **THEN** 错误信息 SHALL NOT 提及 "private repo"
- **AND** SHALL 建议检查网络/代理设置或设置 GH_TOKEN

