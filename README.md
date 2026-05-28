# claude-devtools-rs

可视化 Claude Code 会话执行的桌面应用 —— Tauri 2 + Svelte 5 + Rust workspace。

[claude-devtools](../claude-devtools)（Electron 原版）的 Rust 端口，行为契约以 `openspec/specs/` 为准。

## 特性

- **会话浏览**：扫描 `~/.claude/projects/`，按项目聚合历史 session 并实时跟进正在运行的会话
- **执行轨迹**：UserChunk / AIChunk / SemanticStep 分段 + Tool 调用卡片（Read / Edit / Write / Bash / 自定义 agent）
- **Subagent 视图**：内嵌 ExecutionTrace，token / model / 异常指标一览
- **Context Panel**：CLAUDE.md、slash command、@ 文件引用等上下文注入分类统计
- **全局搜索 + 命令面板**：`Cmd+F` 当前 session，`Cmd+K` 跨 session
- **实时刷新**：FileWatcher debounce → IPC emit → 前端 in-place patch，无"加载中"闪烁
- **桌面通知 + 系统托盘**：可自定义触发器，Dock 未读 badge（macOS）
- **主题**：浅色 / 深色 / 跟随系统
- **性能**：大会话首屏经多轮 IPC payload 瘦身（lazy markdown、image `asset://` 懒加载、subagent / tool output 懒拉、response.content OMIT），千条消息 session 仍能秒开

## 安装

### 桌面应用

从 [Releases](https://github.com/snowzhaozhj/claude-devtools-rs/releases) 下载对应平台安装包：

- macOS：`.dmg`（Apple Silicon / Intel）
- Linux：`.deb` / `.AppImage`
- Windows：`.msi` / `.exe`

> 应用未经过 Apple Developer ID 签名（仅 ad-hoc 签名）/ Windows SmartScreen。
>
> **macOS 首次打开**：从 `.dmg` 拖到 `/Applications` 后，**右键 → 打开**（不是双击），点"打开"确认。若仍被拦截，"系统设置 → 隐私与安全性"里会有"仍要打开"选项。
> 如果提示 "已损坏，无法打开"（浏览器下载会带 quarantine 属性），在终端跑：
> ```bash
> sudo xattr -rd com.apple.quarantine "/Applications/Claude DevTools.app"
> ```
>
> **Windows**：SmartScreen 会拦 → "更多信息" → "仍要运行"。

### CLI (`cdt`)

CLI 工具用于终端查询 session 数据、搭配 Claude Code MCP/Skills 使用。

**一键安装**（macOS / Linux）：

```bash
curl -fsSL https://raw.githubusercontent.com/snowzhaozhj/claude-devtools-rs/main/install.sh | sh
```

**其它方式**：

| 方式 | 命令 |
|---|---|
| 手动下载 | 从 [Releases](https://github.com/snowzhaozhj/claude-devtools-rs/releases) 下载 `cdt-{platform}.tar.gz` |
| 从源码编译 | `cargo install --git https://github.com/snowzhaozhj/claude-devtools-rs cdt-cli` |

安装后运行 `cdt setup mcp --apply` 注册 MCP server，或 `cdt setup skills` 安装 session 分析 skill。

**更新**：重新运行安装脚本或 `cargo binstall cdt-cli` 即可覆盖升级到最新版。

**环境变量**：

| 变量 | 作用 | 默认值 |
|---|---|---|
| `CDT_INSTALL_DIR` | 自定义安装目录 | `~/.local/bin` |
| `CDT_VERSION` | 指定版本（如 `v0.5.12`） | 自动检测最新 |

## 从源码构建

依赖：Rust stable（`rust-toolchain.toml` 锁 1.85+）、Node.js 20+、[pnpm](https://pnpm.io/) 8+、[just](https://github.com/casey/just)。

```bash
brew install just pnpm      # 没装 just / pnpm 先装
just bootstrap              # 首次装前端依赖（走 pnpm install）
just dev                    # 启动桌面应用 dev 模式
```

> 本仓用 pnpm（不是 npm）管前端依赖。lockfile 为 `ui/pnpm-lock.yaml`。worktree 切换 / rebase 后跑 `pnpm --dir ui install` 同步依赖；lockfile 未变时近乎瞬时（hardlink 校验）。

常用 recipe（完整列表 `just` 或 `just -l`）：

| 命令 | 作用 |
| --- | --- |
| `just build` | workspace 编译 |
| `just build-tauri` | 构建桌面应用（独立 manifest） |
| `just test` | Rust + 前端全测 |
| `just lint` | clippy 严格模式（workspace + src-tauri） |
| `just fmt` | rustfmt |
| `just check-ui` | svelte-check + tsc |
| `just test-ui-unit` | 前端 vitest 单测（store / mockIPC / IPC contract 镜像） |
| `just test-ui` | vitest + svelte-check |
| `just test-e2e` | Playwright user story 测试（启 vite + chromium） |
| `just spec-validate` | OpenSpec 严格校验 |
| `just preflight` | fmt + lint + test + test-ui-unit + spec-validate 一把梭 |
| `just release-check` | 发布前检查（版本一致 + 工作树干净 + preflight） |

### 浏览器调试 UI（不开 Tauri 窗口）

```bash
pnpm --dir ui run dev
# 浏览器打开 http://localhost:5173/?mock=1&fixture=multi-project-rich
```

`?mock=1` 启用 dev-only mockIPC，所有 Tauri command 走 fixture 数据；fixture 选项见 `ui/src/lib/__fixtures__/`（`empty` / `single-project` / `multi-project-rich`）。production bundle 完全不含 mockIPC（vite DCE 验证）。

## Browser Access

桌面应用可在 Settings → General → Browser Access 中开启本机 HTTP server。开启后应用会显示 `http://localhost:<port>`，默认端口为 `3456`；在 Chrome 或其它浏览器打开该 URL 即可访问同一套 UI。关闭开关会停止 server，并保留上次端口供下次复用。

安全模型：server 只监听 `127.0.0.1`，CORS 只放行 `localhost` / `127.0.0.1` 来源，不提供 token 或密码鉴权。它适合本机浏览器、iframe 嵌入或本机脚本调用；不会暴露到 LAN。若需要远程访问，请自行在外层配置反向代理、TLS 与鉴权。

浏览器 runtime 通过 HTTP/SSE 访问数据 API；桌面专属能力（系统托盘、Dock badge、OS native notification、应用内更新、Rosetta 检测）不会在浏览器中提供，相关入口会隐藏或禁用。

运行时 smoke 记录：PR3 在 `just dev` 下验证 Settings 开关、`http://localhost:3456` 打开 UI、项目列表、会话详情、SSE 事件和浏览器 Settings 隐藏 Browser Access；release bundle 的静态资源路径由 `src-tauri/src/server_mode.rs` 自动探测 `resource_dir()` 常见候选。

## 项目结构

```
crates/
├── cdt-core       # 共享类型（no runtime deps）
├── cdt-parse      # session-parsing
├── cdt-analyze    # chunk-building / tool-linking / context-tracking / team-metadata
├── cdt-discover   # project-discovery / session-search
├── cdt-watch      # file-watching
├── cdt-config     # configuration-management / notification-triggers
├── cdt-ssh        # ssh-remote-context
├── cdt-api        # ipc-data-api / http-data-api
└── cdt-cli        # 二进制 entrypoint (`cdt`)
ui/                # Svelte 5 + Vite 前端
src-tauri/         # Tauri 2 Rust 后端（excluded from workspace）
openspec/
├── specs/                       # 行为契约真相源（authoritative）
└── TS_BASELINE_DEVIATIONS.md    # TS port 偏差预警 + UI 隐式契约（main 既有 bug 走 GitHub Issue）
```

## 开发与贡献

`main` 是发布分支，**不直接提交**。日常开发走 feature 分支 + PR：

```bash
git checkout -b feat/xxx        # 或 fix/xxx
# ...改代码
just preflight                  # 本地自测
git commit -m "..."
git push -u origin feat/xxx
gh pr create --base main
```

PR 合入前 CI 必须全绿（`.github/workflows/ci.yml` 跑 fmt / clippy / test）。
建议在 GitHub `Settings → Branches` 给 `main` 开启 branch protection：
`Require pull request before merging` + `Require status checks`（勾选 fmt / clippy / test）。

## 发布流程

版本号同步在三处：`Cargo.toml`（workspace）、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`。

```bash
# 1. 在 feature 分支 bump 版本三处同步 → PR 合入 main
# 2. 在 main 上打 tag（tag 格式必须是 vX.Y.Z）
git checkout main && git pull
just release-check              # 验版本一致 + 工作树干净 + preflight
git tag v0.2.0
git push origin v0.2.0

# 3. .github/workflows/release.yml 自动触发：
#    macOS arm64/x64 + Linux + Windows 并行构建 → 产物上传到 Draft Release
# 4. GitHub Releases 页面审查产物 + 补发布 notes → Publish
```

CI 使用 [`tauri-apps/tauri-action`](https://github.com/tauri-apps/tauri-action) 构建。应用集成 `tauri-plugin-updater` 实现应用内自动升级（macOS / Windows / Linux AppImage；`.deb` 不支持）。签名密钥治理与 GitHub Secrets 配置流程见 [Tauri 官方文档](https://tauri.app/plugin/updater/)，本仓不复述。

## Claude Code 集成

`cdt` CLI 提供两种方式与 Claude Code 协作：**MCP Server** 和 **Skills**。

### MCP Server

将 `cdt` 注册为 Claude Code 的 MCP server，让 Claude 直接调用 session 查询工具：

```bash
# 自动注册
cdt setup mcp --apply

# 或手动执行
claude mcp add cdt-devtools -- cdt mcp serve
```

注册后 Claude Code 可使用 `list_projects`、`list_sessions`、`search_sessions`、`get_session_detail`、`get_session_stats` 等工具。

### Skills（推荐）

安装预置的 session 分析 skill 到当前项目：

```bash
# 安装到 .claude/skills/（已存在则跳过）
cdt setup skills

# 强制覆盖已有文件
cdt setup skills --force
```

安装 `session-insights` skill，涵盖：错误分析、token 消耗统计、全文搜索、单 session 诊断。在 Claude Code 中用 `/session-insights` 触发，或直接描述需求（如"看看最近 session 有什么错误"）自动匹配。Skill 直接调用 `cdt` CLI 命令，无需 MCP 配置。

## 开发者文档

- **项目约定 / 架构要点**：[`CLAUDE.md`](./CLAUDE.md)
- **Rust 编码规范**：[`.claude/rules/rust.md`](./.claude/rules/rust.md)
- **行为契约**：`openspec/specs/<capability>/spec.md`
- **OpenSpec workflow**：[`openspec/README.md`](./openspec/README.md)

## License

MIT
