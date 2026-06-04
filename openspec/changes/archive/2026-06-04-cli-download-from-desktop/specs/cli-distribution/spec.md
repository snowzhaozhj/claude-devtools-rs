## Purpose

桌面端提供 CLI binary 的检测、安装和更新能力，让用户无需离开应用即可获取与桌面端同版本的 `cdt` CLI 工具。

## ADDED Requirements

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

用户 SHALL 能通过 Settings 页面一键安装 CLI binary 到 `~/.local/bin/cdt`。

#### Scenario: 首次安装成功

- **WHEN** 用户点击"安装 CLI"按钮
- **THEN** 系统 SHALL 创建 `~/.local/bin/` 目录（如不存在）
- **AND** 从 GitHub Release 下载当前桌面端版本对应的平台 CLI asset（总超时 60s）
- **AND** 校验 HTTP 状态码为 200 且 content-length > 0
- **AND** 解压到临时文件并验证 binary magic bytes
- **AND** 设置临时文件可执行权限（Unix: 0o755）
- **AND** macOS 上清除临时文件 quarantine 属性
- **AND** 用临时文件绝对路径 spawn `<tmp_path> --version` 验证可执行且版本正确
- **AND** 验证通过后通过 atomic rename 写入目标路径 `~/.local/bin/cdt`
- **AND** 刷新内存中的 CLI 状态

#### Scenario: 安装目录无写权限

- **WHEN** 用户点击"安装 CLI"按钮
- **AND** `~/.local/bin/` 无当前用户写权限
- **THEN** 系统 SHALL 显示明确的权限错误提示
- **AND** NOT 留下临时文件

#### Scenario: 网络下载失败

- **WHEN** 下载过程中网络中断或服务器返回非 200 状态
- **THEN** 系统 SHALL 清理临时文件
- **AND** 显示错误信息 + 重试按钮
- **AND** NOT 修改已有的 CLI binary（如存在）

#### Scenario: 安装前验证失败

- **WHEN** 临时文件写入成功但 `<tmp_path> --version` 执行失败（如架构不匹配）
- **THEN** 系统 SHALL 删除临时文件
- **AND** SHALL NOT 修改目标路径的已有 binary
- **AND** 显示验证失败错误

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
