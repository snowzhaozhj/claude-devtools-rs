## MODIFIED Requirements

### Requirement: distro 内 home 解析与 USERNAME fallback

系统 SHALL 对每个 distro 执行 `wsl.exe -d <distro> -- sh -lc 'printf %s "$HOME"'` 解析 distro 内 `$HOME`，stdout 按"解码 `wsl.exe` stdout"规则解码后跑统一的 home path 规范化算法：

1. trim whitespace
2. 若不以 `/` 开头则视为非法 → 返回 None
3. 按 posix 风格 normalize（解析 `..` / 折叠多 `/`）
4. 去除末尾 `/`（除非整路径就是 `/`）

**USERNAME fallback**：若规范化对命令输出返回 None：

- 若 Windows 进程环境变量 `USERNAME` 非空 SHALL 用 `"/home/" + USERNAME` 作为输入再跑一次规范化，命中即视作该 distro 的 `homePath`
- 若 `USERNAME` 为空 / fallback 仍非法 SHALL 视该 distro home 解析失败（计入 `distrosWithoutHome`）

每次解 home 命令 SHALL 设置 5 秒 timeout。

#### Scenario: 命令成功且 home 合法

- **WHEN** distro `Ubuntu` 的 `wsl -d Ubuntu -- sh -lc 'printf %s "$HOME"'` exit 0、stdout 解码后为 `"/home/alice"`
- **THEN** 该 distro `homePath` SHALL 等于 `"/home/alice"`

#### Scenario: 命令输出非绝对路径，USERNAME fallback 命中

- **WHEN** stdout 解码后为 `"alice"`（不以 `/` 开头）
- **AND** Windows 进程 `USERNAME = "alice"`
- **THEN** 系统 SHALL 用 `"/home/alice"` 作 fallback 输入
- **AND** 该 distro `homePath` SHALL 等于 `"/home/alice"`

#### Scenario: 命令失败且 USERNAME 为空

- **WHEN** distro 的解 home 命令 exit 非 0
- **AND** Windows 进程 `USERNAME` 为空
- **THEN** 该 distro SHALL 计入 `distrosWithoutHome`

#### Scenario: home 路径含尾随斜杠

- **WHEN** stdout 解码后为 `"/home/alice/"`
- **THEN** normalize 后 `homePath` SHALL 等于 `"/home/alice"`

#### Scenario: 命令超时

- **WHEN** 解 home 命令在 5 秒内未完成
- **THEN** 系统 SHALL kill 该子进程
- **AND** 走 USERNAME fallback；fallback 仍失败时该 distro SHALL 计入 `distrosWithoutHome`
