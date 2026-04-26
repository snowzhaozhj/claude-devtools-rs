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

## 从源码构建

依赖：Rust stable（`rust-toolchain.toml` 锁 1.85+）、Node.js 20+、[just](https://github.com/casey/just)。

```bash
brew install just           # 没装 just 先装
just bootstrap              # 首次装前端依赖
just dev                    # 启动桌面应用 dev 模式
```

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
npm run dev --prefix ui
# 浏览器打开 http://localhost:5173/?mock=1&fixture=multi-project-rich
```

`?mock=1` 启用 dev-only mockIPC，所有 Tauri command 走 fixture 数据；fixture 选项见 `ui/src/lib/__fixtures__/`（`empty` / `single-project` / `multi-project-rich`）。production bundle 完全不含 mockIPC（vite DCE 验证）。

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
├── specs/         # 行为契约真相源（authoritative）
└── followups.md   # TS impl-bug 反向修复清单 + 性能待办
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

## 开发者文档

- **项目约定 / 架构要点**：[`CLAUDE.md`](./CLAUDE.md)
- **Rust 编码规范**：[`.claude/rules/rust.md`](./.claude/rules/rust.md)
- **行为契约**：`openspec/specs/<capability>/spec.md`
- **OpenSpec workflow**：[`openspec/README.md`](./openspec/README.md)

## License

MIT
