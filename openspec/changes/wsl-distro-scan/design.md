## Context

claude-devtools-rs 当前没有 Windows 平台的 WSL 集成；Windows 用户要把 `claudeRootPath` 设为 `\\wsl.localhost\Ubuntu\home\alice\.claude` 这种 UNC 路径**只能手填**。TS 原版（`../claude-devtools/src/main/ipc/config.ts`）已实现完整工作流：调 `wsl.exe -l -q` 列 distro → `wsl -d <name> -- sh -lc 'printf %s "$HOME"'` 解 home → 拼 UNC → Settings UI 一键应用。

本端口已在 v0.5+ 完成 Windows 平台稳定（CI matrix 含 windows-latest），是补 WSL 入口的合适窗口。

调研已确认（详见会话上下文 Explore 报告）：
- `cdt-discover` 现有 `FileSystemProvider` trait + `FsKind::{Local,Ssh}` 抽象
- TS 落地方式是**改 `claudeRootPath` 配置值**，不是引入新运行时 context
- `wsl.exe -l -q` 输出是 UTF-16 LE（含 BOM 与 NUL 字节）
- UI 当前无 source picker，Settings General section 已有 `claudeRootPath` 文本框 + 文件选择按钮（`ui/src/routes/SettingsView.svelte`）

## Goals / Non-Goals

**Goals:**
- 在 Windows 平台上一键扫描本机 WSL distro 并提供 `~/.claude` UNC 候选路径
- 与 TS 原版工作流对齐（单 distro 自动应用 / 多 distro 弹选择 / 空结果 inline 提示）
- 非 Windows 平台保持 IPC 协议单形态、行为退化为返回空（前端按钮自动隐藏）
- 不增加新 crate / 新 trait / 新运行时 context 类型，保持改动面最薄

**Non-Goals:**
- 不实现"运行时切换到 WSL 内执行命令"——Rust 端口当前从 UNC 路径直接读 session 文件即可
- 不抽象 generic remote source trait——目前只 SSH + WSL 两种 remote，过度抽象
- 不修复 TS 已知的 WSL bug（如 `\\wsl$\` 旧路径、UTF-16 fallback 缺失等）；如有相关 followup 条目随手对齐 spec
- 不引入注册表读取作为枚举 fallback（首版仅 `wsl.exe -l -q`；如真实环境暴露稳定性问题再补）

## Decisions

### D1：模块归属——`cdt-discover/src/wsl.rs` 单文件 mod

**选择**：在 `cdt-discover` crate 下新增 `wsl` 模块（`#[cfg(target_os = "windows")]` gate），不另起 `cdt-wsl` crate。

**理由**：
- WSL 探测最终落地是返回 UNC 候选路径（字符串数据），不需要实现 `FileSystemProvider`——读 UNC 路径直接走 `LocalFileSystemProvider` 即可（Windows `std::fs` 原生支持 UNC）
- 单文件 mod 内放 3 个公开函数（`list_distros` / `resolve_home` / `enumerate_candidates`）+ 解析子函数（UTF-16 解码 / 行 trim）即可，内容量不到 200 行
- `cdt-discover` 当前职责是"枚举 / 发现类工作"，与 `project-discovery` 同源；WSL distro 枚举语义贴合
- 新 crate 收益（边界更干净）远低于新 crate 成本（额外 `Cargo.toml` / workspace 注册 / 单测脚手架 / 命名歧义"是否要管 SSH"）

**替代方案**：
- A. 新 `cdt-wsl` crate —— 成本高、收益低，舍
- B. 抽 `RemoteSource` trait + `cdt-wsl` 实现 `FileSystemProvider` —— 把"配置候选路径返回"做成"运行时挂载点切换"，与 TS 落地方式不一致；当前需求不需要切换 source，舍

### D2：枚举命令——三段 fallback + wsl.exe 路径多 candidate

**选择**：依次尝试三套参数组合，**任一组合**解析出非空 distro 列表即返回；同时对每个组合按 wsl.exe 路径 candidate 顺序尝试。

参数组合（与 TS 原版 `config.ts::listWslDistros` L858-870 一致）：
1. `--list --quiet`
2. `-l -q`
3. `-l`（无 quiet，输出更复杂，需过滤）

wsl.exe 路径 candidate（`getWslExecutableCandidates` L754-765 同源）：
1. `%WINDIR%\System32\wsl.exe`（如 `WINDIR` 环境变量存在）
2. `%WINDIR%\Sysnative\wsl.exe`（32-bit 进程访问 64-bit 目录的 alias）
3. `wsl.exe`（依赖 PATH）

每次调用 timeout 4 秒。

**理由**：
- 不同 Windows / WSL 版本对参数解析有差异；某些版本 `--list --quiet` 行为异常但 `-l -q` 正常，三段 fallback 能容错
- `Sysnative` 路径解决"32-bit 进程在 64-bit Windows 看不到 System32 下的 wsl.exe"经典坑（罕见但发生过）
- 所有组合都失败才视为"WSL 不可用"，返回空报告

**`-l`（无 quiet）输出过滤**：解析时 SHALL 过滤以下行：
- 头部说明行（小写 `startsWith("windows subsystem for linux")` / `includes("default version")` / `startsWith("the following is a list")`）
- 前缀 `*\s*`（当前默认 distro 标记）
- 后缀 `(Default)`（大小写不敏感，trim 后判定）
- 空行 / 仅 NUL 字节的行

去重：同一 distro 名（小写比较）SHALL 只保留首次出现。

**注册表 fallback**：仍不引入。三段命令 fallback 已覆盖绝大多数环境；注册表方案留待后续真实环境暴露需求再补。

### D3：home 解析——`wsl -d <distro> -- sh -lc 'printf %s "$HOME"'` + normalize + USERNAME fallback

**选择**：对每个 distro 执行 `wsl.exe -d <distro> -- sh -lc 'printf %s "$HOME"'`，stdout 按 D7 解码（同样要走 BOM-or-heuristic 检测 + 全局 strip NUL，因为 wsl.exe 的 stdout 编码与 distro / 版本相关），然后跑 `normalize_wsl_home_path`：

1. trim whitespace
2. 必须以 `/` 开头，否则 `None`
3. 按 posix 风格 normalize（解 `..` / 折叠多 `/`）
4. 去除末尾尾随 `/`（除非整路径就是 `/`）

**USERNAME fallback**：
- 若上述 normalize 返回 `None`（命令失败 / stdout 不合法）且 Windows 进程环境变量 `USERNAME` 非空，SHALL 用 `/home/<USERNAME>` 作 fallback 输入再跑一次 normalize；该 fallback 命中即视作 `homePath`
- 两者都失败才视该 distro home 解析失败（计入 D8 的 `distrosWithoutHome`）

**理由**：
- 与 TS 原版 (`config.ts::handleFindWslClaudeRoots` L912-921、`normalizeWslHomePath` L736-746) 完全对齐——TS 验证过 USERNAME fallback 在 distro 启动慢 / `sh` 异常时显著降低误报"无 home"
- distro 内 `$HOME` 默认与 Windows 用户名同名（WSL distro 创建时引导设的），fallback 命中率高
- `printf %s` 不附加换行；用 `sh -lc`（login shell + command）确保 `$HOME` 已展开
- 命令失败 / normalize + fallback 都失败 → 该 distro 计入 `distrosWithoutHome`，**不**整体失败（详见 D8）

**风险**：USERNAME 含空格 / 特殊字符（如域账户 `DOMAIN\User`）时 `/home/<raw>` 可能非合法 Linux 用户名 → normalize 会通过（路径合法）但 distro 内对应目录不存在 → `claudeRootExists = false`。这是 TS 同款行为，能 surface 到用户而非 silent fail。

### D4：UNC 路径形式——`\\wsl.localhost\<distro>\`

**选择**：用 `\\wsl.localhost\<distro>\<linux-home-path>\.claude`，**不**用旧式 `\\wsl$\`。

**理由**：
- `wsl.localhost` 是 Windows 11 / Windows 10 21H2+ 推荐路径（`wsl$` 是旧 alias，仍可用但 Microsoft 建议迁移）
- TS 原版已使用 `wsl.localhost`（`config.ts::toWslUncPath` L749）
- Linux home 路径里的 `/` SHALL 转换为 `\`（Windows path separator）
- 拼接示例：`distro="Ubuntu" + home="/home/alice" → \\wsl.localhost\Ubuntu\home\alice\.claude`

**风险**：在 Windows 10 < 21H2 上 `wsl.localhost` 不可用——本端口最低支持 Windows 10 22H2（Tauri 2 要求），覆盖范围内。

### D5：平台 gate 策略——IPC 单形态，非 Windows 返回空

**选择**：`list_wsl_distros` IPC command 在所有平台都注册；非 Windows 平台返回 `Ok(vec![])`，不返回错误。

**理由**：
- 避免 IPC 协议在不同平台分叉（`#[cfg]` 注册 command 会让前端 mockIPC / vitest 在 mac 跑测试时拿不到 stub）
- 前端按钮 visibility 用单独的 `is_windows()` IPC（如 `app-chrome` capability 已暴露平台信息）或 navigator UA 判定，不依赖 `list_wsl_distros` 返回值
- 非 Windows 调用是 no-op，开销可忽略（一次空 vec 序列化）

### D6：UI 落点——Settings General section "Use WSL" 按钮 + 新增共享 Modal 组件

**选择**：紧贴 `claudeRootPath` 文本框下方加 "Use WSL" 按钮（与 "Browse" / "Reset to default" 等已有按钮同行或下方）。仅 Windows 平台显示。

**Modal 现状**：经调研 `ui/src/lib/components/` 当前**无** `Modal.svelte` / `ConfirmDialog.svelte` 等通用 dialog 组件（实测仅有 `Dropdown.svelte` + `SettingsButton/Field/Group/Toggle.svelte`）。本 change apply 阶段 SHALL **新建** 一个最小可用的通用 `Modal.svelte` 组件放在 `ui/src/lib/components/`（标题 + slot 内容 + 主按钮 + 取消按钮 + ESC / 点击遮罩关闭 + a11y 焦点陷阱），并在 distro 选择 modal 内组装 radio list；**不**为了 WSL 单点需求把 modal 组件做得过度通用，但要写到能让后续 PR 直接复用。

**单 distro**：直接 `update_config("general", { claudeRootPath: candidate.claudeRootPath })`，弹 toast 或 inline 文案确认（不开 modal）。

**多 distro**：弹 `Modal` 组件包裹 distro radio list（每行展示 `distro` 名 + UNC 路径 + `claudeRootExists=false` 时附文案"该 distro 内尚无 Claude 数据"），用户选定 + 点 "Apply" 主按钮后调 `update_config`。

**空结果 / 失败**：按钮下方 inline 文案；不弹 modal。文案区分：
- `candidates.length == 0 && distrosWithoutHome.length == 0` → "未检测到 WSL distro"
- `candidates.length == 0 && distrosWithoutHome.length > 0` → "检测到 WSL distro 但无法解析 home（<distro 名列表>）"（详见 D8 + spec settings-ui）

### D7：wsl.exe stdout 解码——BOM-or-heuristic 检测，否则 UTF-8

**选择**：手工解析 `wsl.exe` 输出（适用于 `--list` 与 `-d X -- sh -lc 'printf ...'` 两类命令的 stdout / stderr）。算法与 TS 原版 `decodeWslOutput` L785-799 + `looksLikeUtf16Le` L767-783 完全对齐：

1. **BOM 检测**：若前 2 字节为 `0xFF 0xFE` → UTF-16 LE 路径
2. **Heuristic 检测**：否则取前 ≤ 512 字节，按 2 字节配对统计；若**奇数 index 处 NUL 字节比例 ≥ 30%** → 视作 UTF-16 LE
3. **UTF-16 LE 路径**：按 2 字节组装 `u16`（`chunks_exact(2)`，余 1 字节丢弃；BOM 命中时先 skip 头 2 字节），调 `String::from_utf16_lossy`
4. **否则**：按 UTF-8 解码（`String::from_utf8_lossy`）—— 覆盖 ASCII / CP1252 子集（绝大多数 ASCII distro 名直接通过）
5. **全局 strip NUL**：解码后字符串 SHALL 替换所有 `\0` 为空（**不止行末**——某些版本输出每个 ASCII 字符后嵌 NUL，仅 strip 行末会留垃圾）
6. 行切分：按 `\r\n` / `\r` / `\n` 切；trim whitespace；过滤空行

**理由**：
- 直接强制 UTF-16 LE（codex 二审前的设计）会在某些 Windows 版本 / locale 下把 ASCII / CP1252 输出误读成"每两个字符拼一个 U16 字符"，distro 名乱码
- BOM 不一定存在；单一启发式（NUL @ odd index ≥ 30%）覆盖 BOM-less UTF-16 LE 主流 case，且对纯 ASCII 输出不会误判（NUL 比例几乎为 0）
- 全局 strip NUL 是关键——即使 heuristic 把 ASCII 误判为 UTF-16，从 UTF-16 解码出的 high-byte NUL 也会被 strip 掉留下原 ASCII 序列；反之亦然，容错强
- 避免引入 `encoding_rs` / `widestring` 等依赖

**单测覆盖**：8 case
1. 含 BOM 的 UTF-16 LE 多行
2. 无 BOM 但 heuristic 命中（NUL @ odd index ≥ 30%）的 UTF-16 LE
3. 纯 ASCII（`Ubuntu\nDebian-12\n`，无 NUL，走 UTF-8 路径）
4. 仅含 BOM 无内容
5. 奇数总字节数（最后 1 字节丢弃）
6. 行末 NUL 字节
7. 行内 NUL 字节（每字符后嵌）—— 全局 strip 验证
8. 混合 `\r\n` 与 `\n`

### D8：错误模型 + 返回结构改为 `WslDistroScanReport`

**返回结构**：`list_wsl_distros` IPC command 不再返回裸 `Vec<WslDistroCandidate>`，而是返回 `WslDistroScanReport`：

```rust
pub struct WslDistroScanReport {
    pub candidates: Vec<WslDistroCandidate>,         // 解 home 成功且产出有效 UNC 的 distro
    pub distros_without_home: Vec<String>,           // 枚举到但解 home（含 USERNAME fallback）失败的 distro 名
}
```

| 情形 | wsl.exe exit | 行为 |
|---|---|---|
| 非 Windows 平台 | — | 返回 `{ candidates: [], distros_without_home: [] }`；不调 wsl.exe |
| WSL 未安装（所有 wsl.exe candidate 都 spawn 失败 / 三段命令都失败） | spawn 失败 / exit 非 0 | 返回 `{ candidates: [], distros_without_home: [] }`；warn 日志一次（标记"WSL 未安装或不可用"） |
| WSL 已装但无 distro（命令 exit 0 但 parse 后 distro 列表空） | 0 | 返回 `{ candidates: [], distros_without_home: [] }`；info 日志 |
| 全部 distro 解 home 失败（含 USERNAME fallback） | per-distro exit / stdout 异常 | 返回 `{ candidates: [], distros_without_home: ["A", "B"] }`；warn 日志（前端可显示"检测到 WSL 但无法解析 home"区分于"未检测到 WSL"） |
| 部分 distro 解 home 失败 | 混合 | 返回 `{ candidates: [...成功的], distros_without_home: [...失败的] }`；warn 日志 |
| 其他 io 错误（join_all 内部 panic 等） | — | 返回 `Err`，前端 UI 按 inline 错误提示处理 |

**关键不变量**：
1. **单 distro 解 home 失败不导致整体失败** —— 失败的 distro 进 `distros_without_home`，成功的进 `candidates`
2. **前端能区分"无 WSL"和"有 WSL 但全失败"** —— 这是 codex design 二审拦下的语义缺口；UI 提示文案因此必须分支
3. **`WslDistroScanReport` 字段的 IPC 契约** —— 序列化为 `{ candidates: [...], distrosWithoutHome: [...] }`（camelCase）；contract test 同步覆盖

### D9：UNC 可访问性探测——`claudeRootExists` 字段

**选择**：候选 candidate 携带 `claudeRootExists: bool` 字段，由后端用 `std::fs::metadata(&unc_path).is_ok()` 探测。

**理由**：
- WSL distro 已安装但用户从未跑过 Claude Code → `~/.claude` 目录不存在 → UI 应该展示"distro 候选"但提示"该 distro 内尚无 Claude 数据"，让用户**知情后**决定是否仍要切到该 distro（首次启动时 Claude Code 会自动创建该目录）
- 不在 `list_wsl_distros` 内部过滤掉不存在的——把决策权留给前端 / 用户
- 探测开销低（`metadata` 一次 I/O）

## Risks / Trade-offs

- **`wsl.exe -l -q` 输出格式跨 WSL 版本可能漂移** → mitigation：解析容错（trim / 过滤空 / NUL 处理），异常输出降级返回空 + warn 日志，不 panic
- **`wsl -d X -- sh -lc ...` 命令在 distro 首次启动时可能很慢**（distro init 触发） → mitigation：每个 distro 单独 `tokio::process::Command` + 全局 timeout（建议 5s / distro），超时跳过该 distro 不阻塞整体
- **多 distro 串行解 home 在 N 大时延迟累计** → mitigation：用 `futures::future::join_all` 并发跑所有 distro 的 home 解析（每个 wsl.exe 进程独立），上限按 D2 / D3 D8 错误模型容忍部分失败；并发上限 6（参考 `.claude/rules/perf.md` "Semaphore 限到 ≤ CPU 核数 / 4"）
- **UNC 路径在 Defender / 第三方杀软下访问可能慢** → 该 risk 由 Windows 平台本身承担，本 change 只读 metadata 一次，不构成额外热路径
- **UTF-16 LE 解析依赖 `wsl.exe` 语言环境** → mitigation：BOM-first 检测；非 ASCII distro 名（罕见）`String::from_utf16_lossy` 容错；如果未来发现 BOM-less UTF-16 输出，再考虑 `encoding_rs` 兜底
- **非 Windows 平台 stub 实现的"协议单形态" vs "代码冗余" 取舍** → 选协议单形态，stub 只是 5 行代码；单测同样跑空 vec 校验

## Migration Plan

无 schema / 配置 / 数据迁移。新功能纯增量。

回滚策略：revert PR 即可，无需数据清理（`claudeRootPath` 是配置层字段，用户即使切到 WSL UNC 路径也可以手动改回）。

## Open Questions

- **Q1**：是否需要展示 distro 内 `~/.claude/projects/<encoded>/` 是否非空作为辅助信号（"该 distro 内有 N 个 project"）？—— 倾向**不做**，避免 list_wsl_distros 退化为重 I/O 命令；用户切过去后由 sidebar 自然反映。
- **Q2**：UI distro 选择 modal 是 radio list 还是 button list？—— 倾向 radio list + "Apply" 主按钮，与 Settings 现有交互风格一致。apply 阶段实测后定。
- **Q3**：是否需要把 distro 选择记录到配置以便下次直接 reuse？—— **不做**。`claudeRootPath` 已经是配置字段，下次启动直接用上次选定的 UNC 路径，不需要单独记忆 distro 选择。
- **Q4**：超时 5s / distro 是否合适？—— TS 原版没有显式 timeout（依赖 Node `child_process` 默认）。本端口设 5s 起步；如发现真实环境 distro init 普遍超时，调到 10s。
