# wsl-distro-discovery Specification

## Purpose
TBD - created by archiving change wsl-distro-scan. Update Purpose after archive.
## Requirements
### Requirement: 枚举本机 WSL distro 并报告

系统 SHALL 提供 IPC command `list_wsl_distros`，返回 `WslDistroScanReport` 结构。`WslDistroScanReport` SHALL 包含两个字段：

- `candidates: Vec<WslDistroCandidate>` —— 解析 home 成功并产出有效 UNC 的 distro 候选
- `distrosWithoutHome: Vec<String>` —— 已枚举到但解 home（含 USERNAME fallback）失败的 distro 名

每个 `WslDistroCandidate` SHALL 包含：

- `distro: String`：WSL distro 名（例 `Ubuntu` / `Debian-12`）
- `homePath: String`：distro 内的 Linux 绝对 home 路径，已经 posix normalize（例 `/home/alice`）
- `claudeRootPath: String`：外部可访问的 UNC 路径，形式为 `\\wsl.localhost\<distro>\<home-with-backslashes>\.claude`
- `claudeRootExists: bool`：该 UNC 路径是否当前可访问

`list_wsl_distros` SHALL 在所有平台都注册为 IPC command。仅在 `target_os = "windows"` 上执行真实枚举；其他平台 SHALL 直接返回 `{ candidates: [], distrosWithoutHome: [] }`，且 SHALL NOT 调用 `wsl.exe`。

`candidates` SHALL 按 `distro` 名升序排列；`distrosWithoutHome` SHALL 按枚举原始顺序保留，去重后保留首次出现。

#### Scenario: Windows 平台单 distro 全部成功

- **WHEN** Windows 主机已安装 WSL，且仅安装一个 distro `Ubuntu`，distro 内 `$HOME=/home/alice`，`\\wsl.localhost\Ubuntu\home\alice\.claude` 已存在
- **THEN** `candidates` SHALL 长度为 1
- **AND** 唯一 candidate `distro = "Ubuntu"`、`homePath = "/home/alice"`、`claudeRootPath = "\\\\wsl.localhost\\Ubuntu\\home\\alice\\.claude"`、`claudeRootExists = true`
- **AND** `distrosWithoutHome` SHALL 为空 vec

#### Scenario: Windows 平台多 distro 全部成功

- **WHEN** Windows 主机已安装 WSL，安装了 `Ubuntu` 与 `Debian-12` 两个 distro，且各自的 home 路径都能成功解析
- **THEN** `candidates` SHALL 长度为 2，按 `distro` 升序排列
- **AND** `distrosWithoutHome` SHALL 为空 vec

#### Scenario: 部分 distro 解 home 失败

- **WHEN** 三个 distro `A` / `B` / `C` 中 `B` 的 home 解析（含 USERNAME fallback）失败
- **THEN** `candidates` SHALL 长度为 2，仅含 `A` 与 `C`
- **AND** `distrosWithoutHome` SHALL 包含 `["B"]`
- **AND** SHALL 记录 `B` 解析失败的 warn 日志

#### Scenario: 全部 distro 解 home 失败

- **WHEN** 枚举到 distro `[A, B]` 但两者 home 解析（含 USERNAME fallback）都失败
- **THEN** `candidates` SHALL 为空 vec
- **AND** `distrosWithoutHome` SHALL 等于 `["A", "B"]`
- **AND** SHALL 记录 warn 日志说明 home 解析全部失败

#### Scenario: 非 Windows 平台

- **WHEN** 在 macOS 或 Linux 主机上调用 `list_wsl_distros`
- **THEN** SHALL 返回 `{ candidates: [], distrosWithoutHome: [] }`
- **AND** SHALL NOT 调用 `wsl.exe`

#### Scenario: WSL 未安装

- **WHEN** Windows 主机所有 `wsl.exe` candidate 路径（`%WINDIR%\System32\wsl.exe` / `%WINDIR%\Sysnative\wsl.exe` / `wsl.exe`）spawn 都失败
- **THEN** `list_wsl_distros` SHALL 返回 `{ candidates: [], distrosWithoutHome: [] }`
- **AND** SHALL 记录一次 warn 级别日志说明 WSL 未安装或不可用

#### Scenario: WSL 已装但无 distro

- **WHEN** Windows 主机三段命令组合（`--list --quiet` / `-l -q` / `-l`）都成功执行但解析后 distro 列表为空
- **THEN** `list_wsl_distros` SHALL 返回 `{ candidates: [], distrosWithoutHome: [] }`
- **AND** SHALL 记录一次 info 级别日志

### Requirement: `wsl.exe` 命令 fallback 链

系统在枚举 distro 时 SHALL 依次尝试三套参数组合，**任一组合**解析出非空 distro 列表即返回；同时对每个组合按 `wsl.exe` 路径 candidate 顺序尝试。

参数组合（顺序敏感）：

1. `--list --quiet`
2. `-l -q`
3. `-l`

`wsl.exe` 路径 candidate（顺序敏感）：

1. `%WINDIR%\System32\wsl.exe`（仅当 `WINDIR` 环境变量非空）
2. `%WINDIR%\Sysnative\wsl.exe`（仅当 `WINDIR` 环境变量非空）
3. `wsl.exe`（依赖 PATH）

每次调用 SHALL 设置 4 秒 timeout。任一 (executable, args) 组合成功且解析出非空 distro 列表 SHALL 立即返回；全部组合都失败或解析为空才视作"WSL 不可用 / 无 distro"。

#### Scenario: 第一组合失败但第二组合成功

- **WHEN** `--list --quiet` 在当前 Windows 版本输出异常被解析为空，但 `-l -q` 输出正常解析出 `["Ubuntu"]`
- **THEN** 系统 SHALL 使用第二组合的结果

#### Scenario: System32 路径不可用但 wsl.exe 在 PATH

- **WHEN** `%WINDIR%\System32\wsl.exe` spawn 失败，但 `wsl.exe`（依赖 PATH）成功执行并返回正常 distro 列表
- **THEN** 系统 SHALL 使用 PATH 上的 `wsl.exe` 执行后续命令

### Requirement: 解析 `wsl.exe` 输出列表

系统在解析 `wsl.exe -l` 类命令输出后 SHALL 过滤如下行：

- 头部说明行：以下任一（小写比较，去前后空白后判定）—— 以 `windows subsystem for linux` 开头、含 `default version`、以 `the following is a list` 开头
- 前缀 `*` 加可选空白（当前默认 distro 标记）
- 后缀 `(default)`（大小写不敏感，trim 后判定）
- 空行 / 经全局 strip NUL 后仍为空的行

去重 SHALL 按小写 distro 名比较，仅保留首次出现。

#### Scenario: `-l` 含说明行

- **WHEN** `wsl.exe -l` 输出（解码后）为：
  ```
  Windows Subsystem for Linux Distributions:
  Ubuntu (Default)
  Debian-12
  ```
- **THEN** 解析结果 SHALL 等于 `["Ubuntu", "Debian-12"]`

#### Scenario: 默认 distro 标记前缀

- **WHEN** 输出某行为 `* Ubuntu`
- **THEN** 解析后该行 distro 名 SHALL 为 `"Ubuntu"`

#### Scenario: 重复 distro 名仅保留首次

- **WHEN** 输出含两行均为 `Ubuntu`（罕见但可能由 fallback 拼接产生）
- **THEN** 解析结果 SHALL 仅含一个 `"Ubuntu"`

### Requirement: 解码 `wsl.exe` stdout

系统 SHALL 把 `wsl.exe` 命令的 stdout / stderr 字节流按以下算法解码为 String，适用于 `--list` 类与 `-d X -- sh -lc 'printf ...'` 类两类命令的输出：

1. **BOM 检测**：若前 2 字节为 `0xFF 0xFE` SHALL 视为 UTF-16 LE，跳过 BOM 后按 UTF-16 LE 解码
2. **Heuristic 检测**：否则取前 ≤ 512 字节按 2 字节配对统计，若**奇数 index 处 NUL 字节比例 ≥ 30%** SHALL 视为 UTF-16 LE 并按其解码（不跳过 BOM 头）
3. **UTF-8 fallback**：否则按 UTF-8 lossy 解码
4. **全局 strip NUL**：解码后字符串 SHALL 替换所有 `\0` 为空字符串（**不止行末**）
5. **行切分**：按 `\r\n` / `\r` / `\n` 任一切分；trim 每行 whitespace；过滤空行

#### Scenario: 含 BOM 的 UTF-16 LE 输出

- **WHEN** stdout 字节为 `[0xFF, 0xFE]` 后跟 `"Ubuntu\r\nDebian-12\r\n"` 的 UTF-16 LE 编码（每字符后可能含 `\0`）
- **THEN** 解码后行序列 SHALL 为 `["Ubuntu", "Debian-12"]`

#### Scenario: 无 BOM 但 heuristic 命中的 UTF-16 LE

- **WHEN** stdout 是 UTF-16 LE 编码但无 BOM，内容为 `"Ubuntu\n"`，且奇数 index NUL 字节比例 ≥ 30%
- **THEN** 解码后行序列 SHALL 为 `["Ubuntu"]`

#### Scenario: 纯 ASCII / UTF-8 输出

- **WHEN** stdout 是纯 ASCII 字节序列 `"Ubuntu\nDebian-12\n"`（NUL 比例近 0）
- **THEN** SHALL 走 UTF-8 路径
- **AND** 解码后行序列 SHALL 为 `["Ubuntu", "Debian-12"]`

#### Scenario: 仅含 BOM 无内容

- **WHEN** stdout 仅含 `[0xFF, 0xFE]` 共 2 字节
- **THEN** 解码后行序列 SHALL 为空 vec

#### Scenario: 奇数总字节数

- **WHEN** UTF-16 LE 路径下 stdout 总字节数为奇数
- **THEN** 系统 SHALL 丢弃最后 1 字节
- **AND** SHALL NOT panic

#### Scenario: 行内嵌 NUL 字节

- **WHEN** 解码后某行含 `"U\0b\0u\0n\0t\0u\0"`（每 ASCII 字符后嵌 NUL，常见于 ASCII 被误读为 UTF-16 后的二次清洗）
- **THEN** 全局 strip NUL 后该行 SHALL 为 `"Ubuntu"`

#### Scenario: 混合换行符

- **WHEN** stdout 解码后内容含 `"Ubuntu\nDebian-12\r\nKali\r"`
- **THEN** 行序列 SHALL 为 `["Ubuntu", "Debian-12", "Kali"]`

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

### Requirement: 候选 UNC 路径构造

系统 SHALL 把 distro 名与 distro 内 Linux home 路径拼接为外部可访问的 UNC 路径，形式为 `\\wsl.localhost\<distro>\<home-with-backslashes>\.claude`。Linux home 路径中的 `/` SHALL 替换为 `\`。

#### Scenario: 标准 home 路径

- **WHEN** `distro="Ubuntu"`, `homePath="/home/alice"`
- **THEN** `claudeRootPath` SHALL 等于 `"\\\\wsl.localhost\\Ubuntu\\home\\alice\\.claude"`

#### Scenario: 含连字符的 distro 名

- **WHEN** `distro="Debian-12"`, `homePath="/root"`
- **THEN** `claudeRootPath` SHALL 等于 `"\\\\wsl.localhost\\Debian-12\\root\\.claude"`

### Requirement: 候选可访问性探测

系统 SHALL 对每个 candidate UNC 路径调用 `std::fs::metadata` 探测可访问性，结果填入 `claudeRootExists` 字段。

#### Scenario: UNC 路径存在

- **WHEN** `\\wsl.localhost\Ubuntu\home\alice\.claude` 目录存在且可访问
- **THEN** 对应 candidate `claudeRootExists` SHALL 为 `true`

#### Scenario: UNC 路径不存在

- **WHEN** distro 已安装但 distro 内未运行过 Claude Code，`\\wsl.localhost\Ubuntu\home\alice\.claude` 不存在
- **THEN** 对应 candidate `claudeRootExists` SHALL 为 `false`
- **AND** 该 candidate 仍 SHALL 出现在返回列表中（不被过滤）

