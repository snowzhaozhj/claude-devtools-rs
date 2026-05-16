## Context

claude-devtools-rs 的 dev / CI / 用户安装三条路径都依赖 `ui/` 前端的 Node 工具链。当前用 npm（lockfile = `package-lock.json`）。worktree 工作流被 CLAUDE.md 列为标准开发流（`EnterWorktree` 后开工），但 npm 每次 `install` 都做"真复制依赖到 worktree 的 `node_modules`"——~300 MB 文件 + 30 s-90 s 时延 + 跨 worktree 不共享。

迁移到 pnpm 后用 content-addressable global store + hardlink，能解决三件事：(a) 跨 worktree 共享磁盘；(b) lockfile 未变时 install 近瞬时；(c) `node_modules` 严格扁平避免幽灵依赖。

但选包管理器涉及多维 trade-off：兼容性、CI 设置成本、贡献者上手摩擦、生态支持。本文记录决策与放弃的候选。

## Goals / Non-Goals

**Goals**:

1. worktree 切换 / rebase 后 `install` 从 30-90 s 降到 < 5 s（lockfile 未变时 < 2 s）
2. 跨 worktree 共享 `node_modules` 数据，磁盘占用从 N × 300 MB 降到 ~300 MB（全局 store 一份 + hardlink）
3. CI 不变慢（pnpm install --frozen-lockfile 在缓存命中场景 ≤ npm ci）
4. 不破坏既有行为契约（vitest / svelte-check / playwright / vite build / bundle DCE 全过）

**Non-Goals**:

- 不改前端测试金字塔结构（四层职责不变）
- 不改 vite / svelte / tauri 版本（纯工具链替换）
- 不引入 pnpm workspace（本仓 ui/ 是单包，Tauri Rust 后端走 cargo workspace）
- 不改用户视角下载产物（dist / bundle byte-compatible）

## Decisions

### D1: 选 pnpm 不选 bun / yarn / 保留 npm

候选对比：

| 方案 | install 速度（lockfile 未变）| Vite/Svelte/Tauri 兼容性 | CI 设置成本 | 贡献者上手 |
|---|---|---|---|---|
| **pnpm**（采用）| ~1-2 s（hardlink 校验）| 官方支持，docs 有现成示例 | 加 `pnpm/action-setup` 一行 | `brew install pnpm` 一行 |
| bun | ~0.5 s | 中等（esbuild/rollup native binary 偶发坑 + svelte-check 内部走 node 解析路径）| 加 `oven-sh/setup-bun` | `brew install oven-sh/bun/bun` |
| yarn berry | ~2-5 s（PnP 模式有兼容性坑）| 中等（与 Tauri / vite 配合稍弱）| `corepack enable` | 同 npm 节奏 |
| 保留 npm | 30-90 s（worktree 复制全量）| - | - | - |

**取舍**：bun 理论快一倍但在本仓踩过的 vite optimizer / Playwright / svelte-check 链路里多了一个未知变量（CLAUDE.md 测试金字塔段记录了不少时序 flake，不愿引入新变量）。pnpm 兼容性最稳、全球生态文档示例最多，加速幅度（30× 量级）已超过区分 bun vs pnpm 的边际差异。yarn berry PnP 模式与 Tauri / vite 配合在社区里偶有坑，且贡献者熟悉度更低。

### D2: lockfile 替换是一次性硬切，不并存

候选：（a）保留 `package-lock.json` 让 npm 用户仍可装；（b）只用 `pnpm-lock.yaml` 删掉 `package-lock.json`。

**采用 b**。两个 lockfile 并存会导致：CI cache key 取哪个不一致；新增依赖在 npm / pnpm 各 resolve 一次解析结果可能不同（解析算法差异）造成 lockfile drift；贡献者本机环境分裂。

风险缓解：README 第一步加 "Install pnpm" 步骤；本 PR commit message 显眼写明迁移。

### D3: CI 上 `npm install -g @fission-ai/openspec` 保留 npm

candidate：(a) 改成 pnpm 全局；(b) 保留 npm。

**采用 b**。理由：

- GitHub Actions runner 预装 npm，无需额外 setup
- pnpm 全局工具需要 `pnpm setup` 配 PATH（增加 step）
- 这一步 1 次性安装在 CI 上耗时几秒，不影响 PR 体验
- 与 worktree dev 痛点无关——本痛点是 `ui/node_modules` 频繁重装

一致性弱化但实际开销最低。

### D4: `npx` 全替换为 `pnpm exec`

candidate：(a) `npx` 在 pnpm 项目里仍能工作（npx 5+ 自动找 `node_modules/.bin`）；(b) 改 `pnpm exec`。

**采用 b**。`npx` 会触发 npm 子进程，与 pnpm 链路混用偶发奇怪 cache 行为（npx 默认 install 临时包到 ~/.npm）。`pnpm exec` 直接走 pnpm 自己的 bin 解析，路径单一。

唯一例外：`.claude/hooks/svelte-check-after-edit.sh` 里若改 `pnpm exec` 而项目还没 `pnpm install` 过会报错。但 CLAUDE.md 已要求贡献者首次开工要装依赖，符合现有约定。

### D5: 不引入 `.npmrc` / `.pnpmrc` 配置文件

pnpm 默认行为（`auto-install-peers=true` 自 pnpm 8 起为默认；`strict-peer-dependencies=false` 也是默认）足够 covers 本仓需求。如果未来需要锁 store 路径或调 hoist 策略再加。

### D6: hooks 改 `pnpm exec svelte-check` 不改 `node_modules/.bin/svelte-check`

candidate：(a) 直接 path；(b) 经过 `pnpm exec`。

**采用 b**。`pnpm exec` 自动找当前目录或 `monorepo` 上层的 bin，且失败时报错明确（"command not found, did you run pnpm install?"）；直接 path 在 worktree 没装依赖时报 ENOENT 不直观。

### D7: spec delta 只动 npm 字符串，不重写 Scenario 结构

`openspec/specs/frontend-test-pyramid/spec.md` 涉及 4 处 `npm ... --prefix ui` / `npx ...` 字符串散布在 4 个不同 Requirement 的 Scenario 中。两种写法：

- (a) 每个 Requirement 整体 MODIFIED（按 CLAUDE.md openspec archive 语义，MODIFIED body 完整替换）
- (b) 加新 Requirement 单独描述工具链命令

**采用 a**。`npm → pnpm` 是命令字面替换，行为契约（vitest 跑、playwright 跑、bundle DCE）不变；新增 Requirement 反而割裂"测试金字塔"语义。每个被影响的 Requirement 完整重写，body 保留 SHALL/MUST 句不变，仅 Scenario WHEN 子句的命令字符串改写。

### D8: release.yml 的 tauri-action **不**加 `tauriScript: pnpm tauri`

candidate：(a) 加 `tauriScript: pnpm tauri`（要求 `@tauri-apps/cli` 进 ui/devDep）；(b) 不动，让 tauri-action 自己装 tauri CLI

**采用 b**。理由：

- 本仓的 `@tauri-apps/cli` **不在** `ui/package.json` 也**不在** `src-tauri/Cargo.toml`——tauri-action 在 `projectPath: src-tauri` 下找不到 `package.json`，自动 fallback 用 `npm install -g @tauri-apps/cli` 装 GitHub runner 全局 binary
- 这一步 npm install 与 ui/ 下的 pnpm 链路**正交**——前端 dist 在前一步 `pnpm --dir ui run build` 已产出到 `ui/dist/`，tauri-action 后续只跑 `tauri build` 编译 Rust + 打 bundle，不再碰前端依赖
- 本仓 v0.4.x 系列（v0.4.0 - v0.4.10）全部 release 成功，证明该链路稳定。pnpm 切换不引入新风险
- 如果加 `tauriScript: pnpm tauri` 就**必须**同步把 `@tauri-apps/cli` 加进 ui/devDep——额外 80 MB+ 装依赖换不到任何收益（pnpm 与 npm 装的都是同一个 binary）

风险点：tauri-action v0+ 内部实现若日后变更（如要求 `package.json` 必须存在并指定 packageManager），需在 PR fail 时回头加 `tauriScript`。本仓首个 release（本 PR merge 后）跑通验证 D8 决策有效。

## Risks / Trade-offs

- **R1**：贡献者首次开工没装 pnpm 会报 `command not found`。**缓解**：README 第一步强调，配合 hook `svelte-check-after-edit.sh` 在 `pnpm` 找不到时优雅 fallback（待定，先看 hook 测试是否需要）。
- **R2**：pnpm 严格扁平 `node_modules`，会暴露幽灵依赖。**缓解**：先跑 `pnpm install` + `pnpm --dir ui run check` + `pnpm --dir ui run test:unit`，如出现 `Cannot find module` 类错误就在 `package.json` 显式加缺失的依赖。
- **R3**：CI 缓存 key 切换期间一次 cold miss（runner 上无 pnpm store cache）。**缓解**：一次性的，第二次 PR 后稳定；GitHub Actions runner 上 pnpm install --frozen-lockfile cold ~30-60 s 与 npm ci 同量级。
- **R4**：`tauri-action` 内部 detect 包管理器逻辑——它默认探测 `lockfile` 文件名。**缓解**：tauri-action v0+ 支持 pnpm（识别 pnpm-lock.yaml），实测在社区 Tauri 项目里通用；若不识别需在 workflow 里加 `beforeBuildCommand: "pnpm install --frozen-lockfile"`。**待 CI 验证**。
- **R5**：worktree 共享 store 路径在 macOS / Linux / Windows 行为略不同（Windows 无 hardlink 用 copy）。**缓解**：本仓主要开发在 macOS；CI 上 Linux 也是 hardlink。Windows 贡献者会退化到 copy（仍比 npm 复制快，因为 store cache 命中）。
- **R6**：`tauri.conf.json::beforeDevCommand` 改 `pnpm` 后老贡献者 IDE 集成（如 RustRover Tauri 配置）需重新认；属于一次性切换成本。

## Migration Plan

1. `openspec validate frontend-pnpm-migration --strict` 过
2. 删 `ui/package-lock.json`，跑 `cd ui && pnpm install` 生成 `pnpm-lock.yaml`，确认 `pnpm --dir ui run check` 通过
3. 替换 `justfile` / hooks / `tauri.conf.json` / `tauriMock.bundle.test.ts` 命令
4. 替换 `.github/workflows/frontend-test.yml` + `release.yml`（加 pnpm/action-setup，改 cache + install + run）
5. 同步 README + CLAUDE.md + opsx-apply-cadence rules 中所有命令片段
6. `just preflight` 全绿（fmt + lint + test + ipc-sync + spec-validate + spec-archive-check）+ `just test-ui-unit` + `just test-e2e`
7. push + PR + wait-ci，CI 红了 fix；codex 二审；archive
