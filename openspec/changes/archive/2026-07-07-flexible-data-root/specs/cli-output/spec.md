## ADDED Requirements

### Requirement: 全局数据根覆盖参数

CLI binary `cdt` SHALL 提供全局参数 `--root`（别名 `--data-dir`）用于临时覆盖数据根目录。该参数 SHALL 仅作用于当次进程调用；系统 SHALL NOT 因 `--root` 把该值写入配置文件的 `claudeRootPath` 或 `recentRoots`（避免污染桌面端与 CLI 共享的配置状态）。此不变量约束的是"`--root` 不落盘"，与 `ConfigManager::load()` 既有的独立 migration side effect 无关。

`--root` 解析出的 resolved root SHALL 应用于该次调用的**全部**数据根消费路径——包括普通子命令的 projects/sessions 取数，以及 `cdt serve` 启动的 HTTP server / 文件 watcher / SSE 推送。任一消费路径都 SHALL NOT 回退到配置文件 root。

数据根解析优先级链 SHALL 为：`--root`（当次覆盖） > 配置文件 `general.claudeRootPath`（默认继承桌面端设置） > 内置默认 root。`--root` 的取值 SHALL 支持 `~/`（Windows `~\`）前缀展开，与 GUI 侧采用同一路径解析语义（详 [[project-discovery]]）。`--root` 取值非法（相对路径 / 具名 home `~user/`）时，命令 SHALL 以非零退出码失败并输出错误说明。

#### Scenario: --root overrides data root for one invocation

- **WHEN** 用户运行 `cdt --root ~/.qoder projects list`
- **THEN** SHALL 列出 `<home>/.qoder/projects/` 下的项目
- **AND** 配置文件的 `claudeRootPath` 与 `recentRoots` SHALL NOT 因本次调用被写入

#### Scenario: --root applies to serve HTTP/watcher path

- **WHEN** 用户运行 `cdt --root ~/.qoder serve`
- **THEN** 启动的 HTTP server / 文件 watcher / SSE 推送 SHALL 全部基于 `<home>/.qoder/` 数据根
- **AND** SHALL NOT 回退到配置文件 `claudeRootPath`

#### Scenario: 无 --root 时继承配置文件

- **WHEN** 用户运行 `cdt projects list`（不带 `--root`）且配置文件 `claudeRootPath` 已设为某路径
- **THEN** SHALL 使用配置文件中的数据根

#### Scenario: --data-dir 为 --root 别名

- **WHEN** 用户运行 `cdt --data-dir ~/.qoder sessions list`
- **THEN** 行为 SHALL 等同于 `--root ~/.qoder`

#### Scenario: --root 非法路径报错

- **WHEN** 用户运行 `cdt --root relative/path projects list`
- **THEN** 命令 SHALL 以非零退出码失败并输出错误说明
