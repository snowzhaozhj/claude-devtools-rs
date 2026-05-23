# claude-devtools-rs

[claude-devtools](../claude-devtools)（Electron 原版）的 Rust 端口，Tauri + Svelte 桌面应用。
用户视角的安装 / 开发 / 发布流程见 `README.md`；本文件是 contributor 专用的**跨域共识**与索引。

## 按域去哪查（子目录 CLAUDE.md 按需自动加载）

| 工作域 | 子目录 CLAUDE.md | 承载内容 |
|---|---|---|
| 前端 Svelte / Vite | `ui/CLAUDE.md` | Svelte 5 陷阱、渲染依赖、列表反闪烁、测试基础设施（vitest / playwright / mockIPC） |
| Tauri 后端 / IPC | `src-tauri/CLAUDE.md` | Tauri 配置链、IPC payload 瘦身模式、tauri-plugin-updater、通知/托盘、devtools feature |
| Rust workspace | `crates/CLAUDE.md` | Rust 命名/error/async/log/clippy、serde camelCase、Windows 兼容、后台任务 per-key cancel、IPC vs HTTP 分叉、chunk-building 语义契约 |
| openspec | `openspec/CLAUDE.md` | spec 变更约定 7 条、spec delta 写法、archive 顺序坑、引用约定 |

**跨域规则**散文件（按需 Read，模型自行决策）：

| 文件 | 何时读 |
|---|---|
| `.claude/rules/opsx-apply-cadence.md` | 任何 PR 推进流水线（业务段 + 发布尾段 N.1-N.4） |
| `.claude/rules/codex-usage.md` | 何时调 codex 二审 / rescue + prompt 模板 |
| `.claude/rules/perf.md` | 涉及启动路径 / IPC / 后端算法 / 列表渲染的 PR |
| `.claude/rules/parallelism-modes.md` | 主 session 之外起并行执行单元前（覆盖三形态：subagent / agent team / bg） |

## Parent repo

TS 原版位于同级目录 `../claude-devtools`，仅作历史参考；**所有行为契约以 `openspec/specs/` 为准**。已知 TS impl-bug 见 `openspec/followups.md`——按 spec 走，**不复刻** TS 的 bug。

## Workspace layout

```
claude-devtools-rs/
├── Cargo.toml                # workspace root
├── rust-toolchain.toml       # stable channel
├── crates/                   # cdt-core / cdt-parse / cdt-analyze / cdt-discover /
│                             # cdt-watch / cdt-config / cdt-ssh / cdt-api / cdt-cli
├── ui/                       # Svelte 5 + Vite 前端
├── src-tauri/                # Tauri 2 Rust 后端 (excluded from workspace)
├── openspec/
│   ├── specs/<cap>/spec.md   # 主 spec（行为契约真相源，由 archive 自动 sync）
│   ├── changes/<slug>/       # 进行中 change：proposal.md + design.md + tasks.md + specs/<cap>/spec.md (delta)
│   ├── changes/archive/YYYY-MM-DD-<slug>/  # 冻结历史快照
│   ├── followups.md          # TS impl-bugs to fix, not replicate
│   └── README.md             # workflow + capability map
└── .claude/rules/            # 跨域规则散文件（4 个：parallelism-modes / codex-usage / opsx-apply-cadence / perf）
```

详细 crate → capability 映射见 `crates/CLAUDE.md::crate 边界`；UI 布局见 `ui/CLAUDE.md::架构与布局`。

## Capability → crate map（一句话）

`cdt-parse`(session-parsing)、`cdt-analyze`(chunk-building / tool-linking / context-tracking / team-metadata)、`cdt-discover`(project-discovery / session-search)、`cdt-watch`(file-watching)、`cdt-config`(configuration-management / notification-triggers)、`cdt-ssh`(ssh-remote-context)、`cdt-api`(ipc-data-api / http-data-api)、`cdt-telemetry`(application-telemetry — Signal Bus)。详见 `openspec/README.md`。

## Common commands

所有任务通过 `just` 跑——首次安装 `brew install just`，跑 `just` 列出所有 recipes（真相源在 `justfile`）。最常用：

- `just preflight` —— fmt + lint + test + spec-validate 一把梭
- `just dev` —— 启动 Tauri 桌面应用
- `just test-crate <name>` —— 单 crate
- `just release-check` —— 三处版本一致 + lock 同步 + preflight
- `just bg-pr <name> <prompt>` —— 一键起 bg 后台 session 跑 PR 全流水线
- `just bg-status` / `just bg-stop-all` / `just bg-clean <id>` —— bg 管理
- `just clean-worktrees` / `just clean-worktrees-apply` —— 清理已 merge 的 worktree

直接跑 `cargo xxx` 仍可用，但注意：`cd src-tauri/` 后 Bash tool 的 cwd 会持久化，后续 `cargo test --workspace` 会误入 tauri 子 manifest——优先用 `just` 或 `--manifest-path`。

## macOS 开发陷阱（跨域）

详细 `TempDir` / FSEvents canonicalize 等测试陷阱见 `crates/CLAUDE.md::测试基础设施陷阱`。本节只列跨域：

- **worktree rebase 后 ui 依赖可能变**：origin/main 合并的 PR 若加新 ui 依赖（典型 `tauri-plugin-opener` 这类），worktree 的 `ui/node_modules` 没装会让 `svelte-check` / `vitest` 报 "Cannot find module"。修法：rebase 后跑 `pnpm --dir ui install` 重装（pnpm hardlink + global store，lockfile 未变时近瞬时；变了也只下载差量）。`cargo` 依赖走 workspace lockfile 自动同步，**只有 ui 依赖**有这坑。

## 发布与分支策略

- `main` 是发布分支，**不直接提交**（hook 会 deny）。日常走 `feat/xxx` / `fix/xxx` 分支 → PR → merge（详见 README）。
- **release commit 也走 PR**：开 `chore/release-X.Y.Z` 分支 → bump 三处版本号 → `just release-check`（**会顺带改 `Cargo.lock` / `src-tauri/Cargo.lock`**，需 amend 进 release commit 否则 release PR CI 上 lock 与 manifest 不一致）→ push + PR + merge → 在 main 上 `git tag vX.Y.Z && git push origin vX.Y.Z`。
- 版本号同步在三处：`Cargo.toml`（workspace）、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`；`just release-check` 跑一致性校验。
- 打 `vX.Y.Z` tag 触发 `.github/workflows/release.yml` —— macOS / Linux / Windows 矩阵构建 Tauri bundle 到 Draft Release。**workflow 必须 `create-release` 前置 job + `build` matrix 用 `releaseId` 复用**——直接让 4 个 matrix job 各自带 `tagName/releaseDraft` 跑 `tauri-action` 会并发 race，同一 tag 下产出 4 个 draft 各自只含本平台 asset + 各自的 latest.json，**无法 publish**（v0.4.2 事故案例）。`tauri-action` 在给定 `releaseId` 时只上传 + merge `latest.json`，不再调 createRelease。
- 完整发版流水线 + F1-F7 known fixes：`release-runbook` skill。

## 维护清理

worktree 编译产物每个 ~6 G，merge PR 后忘清会快速吃光磁盘。半自动清理：

- `just clean-worktrees`：dry-run，扫所有 `.claude/worktrees/*`，gh 查 PR 状态，列出已 merge 且工作树干净的候选 + 预计释放空间
- `just clean-worktrees-apply`：真删 worktree + 本地分支（仅删 PR merged/closed && uncommitted=0 && unpushed=0 的；脏 worktree / 活跃分支保留）

习惯：merge 任何 PR 后跑一次 `just clean-worktrees`；候选 OK 就 `--apply`。

## 性能（硬约束 — 详 `.claude/rules/perf.md`）

本仓 Rust 重写原 TS 项目的根本动机就是性能。**预算 / 反模式 / bench 入口 / PR 模板**全部沉淀在 `.claude/rules/perf.md`——任何涉及启动路径 / IPC / 后端算法 / 列表渲染的 PR 都 SHALL 读一遍那个文件并按规则评估 perf impact。

大会话首屏卡顿走 IPC payload 瘦身路径——模式见 `src-tauri/CLAUDE.md::IPC payload 瘦身模式`；bench 入口 / 探针 target 详 `perf.md::bench 入口`；历次实现查 `git log --grep="feat(perf)"`。

## 测试金字塔

四层职责互斥，命中改动类型用对应层（详见 `openspec/specs/frontend-test-pyramid/spec.md`）：

| 层 | 跑命令 | 覆盖 | 何时改 |
|---|---|---|---|
| Rust IPC contract test | `cargo test -p cdt-api --test ipc_contract` | Tauri command 序列化字段名 / `xxxOmitted` 命名 / enum tag 值 | 改 `LocalDataApi` 公开方法返回字段时 SHALL 同步 |
| Vitest 单测 + mockIPC | `just test-ui-unit` | 纯函数 / store 状态机 / mockIPC 完整性 | 加纯函数 / 改 store 时 |
| Playwright user story | `just test-e2e` | 浏览器真渲染 + 键鼠事件 + 跨组件状态联动 | 改 UI 行为 / 跨组件交互时 |
| 手动 `just dev` | `cargo tauri dev` 桌面窗口 | 真 Tauri IPC + 平台 API（通知 / 托盘 / setBadgeCount） | 发版前 smoke + 涉及 Tauri-only API 时 |

IPC 字段改动 checklist（硬约束）+ 浏览器调试入口 + 测试基础设施陷阱 详见 `src-tauri/CLAUDE.md::IPC 字段改动 checklist` 与 `ui/CLAUDE.md::浏览器调试入口` / `测试基础设施陷阱`。

## 自动化与索引

- **Hooks**（`.claude/hooks/`）：`.rs` 编辑后自动跑所属 crate 的 `cargo clippy -- -D warnings`；`.svelte` 编辑后自动跑 `svelte-check`；`git commit` 前自动跑 `openspec validate --strict`；编辑 `main` 分支 deny。
- **本仓自建 skill**（`.claude/skills/<skill>/SKILL.md`）：
  - 开工：`preflight`（fetch + 分支 + OpenSpec + Explore 四件套）
  - 发版：`bump-version`（三处版本同步）/ `release-runbook`（完整流水线 + F1-F7 known fixes）
  - CI：`wait-ci`（push 后 SHALL 调）
  - 性能：`perf-bench`（SessionDetail 首屏 IPC 瘦身诊断）
  - 测试：`insta-review`（接受 insta 快照变更）
  - 维护：`port-dashboard` / `ts-parity-check` / `ts-ui-compare`（仅显式调）/ `mark-tasks-done`（仅显式调）
- **自建 skill 维护**（`.claude/skills/<skill>/SKILL.md`，非 openspec/opsx 也非下载）：description 用 pushy 措辞"**只要**用户说 X / Y / Z **都用这个 skill**"提升触发率；有副作用 / 与节拍冲突 / 默认不该触发的动作加 `disable-model-invocation: true`；不硬编码 capability 数 / 列表 / 文件映射，用 `ls openspec/specs/` 等运行时探测（历史踩坑：`port-dashboard` 老版硬编码"13 个 capability"）；引用 `.claude/rules/*.md` 真相源而非重抄内容。
- **Subagent**：`spec-fidelity-reviewer` 按 capability 审计 scenario→test 覆盖；`rust-conventions-reviewer` / `tauri-config-reviewer` / `windows-compat-reviewer` 按域审查；`codex:codex-rescue` 跑异构二审。
- **MCP**：`.mcp.json` 注册 GitHub MCP，需要 `GITHUB_PERSONAL_ACCESS_TOKEN` 环境变量。

## What to do first in a fresh session

0. **识别开工信号 SHALL 第一时间跑 preflight**：用户首条 message 含**开工信号词且未含停手词** → SHALL `Skill(preflight)` 跑 4 件套自检（分支 / OpenSpec / Explore / 流水线终点）；**不要**跳过 preflight 直接 Edit/Bash。开工信号词 / 停手词的权威定义见 `.claude/skills/preflight/SKILL.md::Q4` 的「触发词表（本节是项目内唯一权威源）」段。跳过 preflight = 跳过"提需求 = 默认走到 codex+CI 全绿"的契约，是已被否决的下策（本条来自 PR #139 反思——业务改完忘走 N.1-N.3 让用户监工）。
1. 开新工作前先 `git fetch origin main && git checkout -b feat/<slug>`，**不要**直接在 `main` 上写代码——`fetch` 是硬约束：EnterWorktree 默认 `worktree.baseRef=fresh` 用本地 origin/main 指针，不 fetch → 从过期 SHA 起 → PR 一上来 conflict（PR #122 案例：本地 origin/main 落后 24h+，错过 #119/#120/#121，PR 上来就 dirty 多走一轮 merge + push）。
2. UI 功能迭代分流：**行为契约改动**（IPC 字段 / 后端算法 / 状态判定 / 数据流语义）走 openspec（`/opsx:propose` → `/opsx:apply` → `/opsx:archive`，design.md 必备）；**纯视觉对齐 / 单点样式修复 / Trigger CRUD 等**直接写 + PR。判断不准默认走 openspec。**视觉/规范级 UI 任务**（"样式优化 / 重新设计 / 统一规范 / polish / a11y / typography / 调整 UX / 重写组件视觉"等关键词）SHALL 在动手前先 invoke `impeccable` skill 拿 PRODUCT.md + DESIGN.md 上下文与设计禁令。
3. 性能 / 卡顿排查：先跑 `perf-bench` skill 看数据再定方向，不要瞎猜。
4. 提交前跑 `just preflight` 把 fmt / lint / test / spec-validate 一把梭。
5. **默认每个 PR push 后**都调 `Agent({ subagent_type: "codex:codex-rescue", ... })` 跑 codex 异构二审。豁免：bump version / 纯 docs / typo / CI 配置微调（跳过时 PR 描述写明理由）。判断规则与 prompt 模板见 `.claude/rules/codex-usage.md`。**codex CR 可能多轮**：每轮都用 `SendMessage` 接续同一 agentId 复用 context，不要起新 agent 重头读。
6. **PR push 后 SHALL `/wait-ci` 直到全绿**——`scripts/check-openspec-archives.sh` 等 CI-only check 不会被本地 `just preflight` 拦下。CI 红了 SHALL 自己 `gh run view --log-failed` 定位 + 修 + 再 push，不要宣称"完成"就走人。完整发布尾段（push → wait-ci → codex → archive）见 `.claude/rules/opsx-apply-cadence.md`。
7. **并行执行形态**（决策树详 `.claude/rules/parallelism-modes.md`）：单点 / 中等 PR 主 session 自跑 + subagent 按需调；**大改动**（>2 天 AND 多角色 / 视觉重构 / 跨 capability 任一）SHALL 用 **Agent team**（需 `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`）；**N≥2 独立 PR** SHALL 用 `claude --bg`（subagent 跑长流水线会爆主 context；codex-rescue 二审作为 subagent 调用不在此禁）。拆 PR 前用 4 ✓ 框架（独立 / 可验证 / 工作量值得 / wall time），任一不满足合并 1 PR。
