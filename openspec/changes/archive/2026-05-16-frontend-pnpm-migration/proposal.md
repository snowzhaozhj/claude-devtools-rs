## Why

worktree 开发流当前痛点：每 `EnterWorktree` / `merge 后 rebase` 触发 ui 依赖变化时，都要在新 worktree 跑一次 `npm install`，单次耗时 30s-90s 且产 ~300 MB `node_modules` 真目录复制。CLAUDE.md 已记录这是 dev 环境**只有 ui 依赖**才有的坑，cargo 走 workspace lockfile 自动同步无此问题。

pnpm 用 content-addressable global store（`~/Library/pnpm/store`）+ 每个项目 `node_modules` 全部 hardlink，跨 worktree 共享一份磁盘数据；lockfile 未变时 `pnpm install` 在 100 ms-2 s 完成（只校验链接），实测比 npm 快 10-30 ×，磁盘也省。

## What Changes

替换前端包管理器 npm → pnpm：

- 删 `ui/package-lock.json`，生成 `ui/pnpm-lock.yaml`（首次 `pnpm install` 自动产出）
- `justfile`：5 处 `npm ... --prefix ui` → `pnpm --dir ui ...`（bootstrap / check-ui / test-ui-unit / test-e2e / release-build）
- `.claude/hooks/svelte-check-after-edit.sh`：`npx svelte-check` → `pnpm exec svelte-check`
- `.github/workflows/frontend-test.yml` + `release.yml`：加 `pnpm/action-setup`，`setup-node` 的 `cache: 'npm'` → `cache: 'pnpm'`，`npm ci` → `pnpm install --frozen-lockfile`，`npm run xxx` → `pnpm xxx`，`npx playwright` → `pnpm exec playwright`
- `.github/workflows/ci.yml::openspec job`：保留 `npm install -g @fission-ai/openspec`（runner 预装 npm，与本仓 ui/ 依赖管理无关；改 pnpm 需额外 `pnpm setup` 配 PATH 步骤增加复杂度且非主要痛点）
- `src-tauri/tauri.conf.json::beforeDevCommand`：`npm run dev --prefix ../ui` → `pnpm --dir ../ui dev`
- `ui/src/lib/tauriMock.bundle.test.ts`：`execSync('npm run build', ...)` → `execSync('pnpm build', ...)`
- 同步更新 `README.md` / `CLAUDE.md` / `.claude/rules/opsx-apply-cadence.md` / `openspec/specs/frontend-test-pyramid/spec.md` 中所有 `npm ...` 文档片段
- **BREAKING（开发者侧）**：贡献者需本机安装 pnpm（`brew install pnpm` 或 `corepack enable && corepack prepare pnpm@latest --activate`）；README 加这一步前置说明

行为契约**不变**：vitest / svelte-check / playwright / vite build 链路完全一致，dist 产出 byte-for-byte 兼容；CI 测试矩阵不动；mockIPC bundle DCE 校验、production bundle 不含 fixture 字符串等 invariants 保留。仅"调用入口命令"和"lockfile 文件名"改变。

## Impact

- Affected specs: `frontend-test-pyramid`（4 处 Scenario WHEN 子句的命令字符串更新）
- Affected code: `justfile`、`.claude/hooks/svelte-check-after-edit.sh`、`.github/workflows/*.yml`、`src-tauri/tauri.conf.json`、`ui/package-lock.json`（删）、`ui/pnpm-lock.yaml`（新增）、`ui/src/lib/tauriMock.bundle.test.ts`、`README.md`、`CLAUDE.md`、`.claude/rules/opsx-apply-cadence.md`
- Affected workflow：贡献者首次开工需 `brew install pnpm`；worktree 切换后跑 `pnpm --dir ui install`（lockfile 未变时近乎瞬时）
