# Tasks

## 1. lockfile 与依赖切换

- [x] 1.1 删 `ui/package-lock.json`
- [x] 1.2 在 `ui/` 跑 `pnpm install` 生成 `ui/pnpm-lock.yaml`
- [x] 1.3 确认 `pnpm --dir ui run check`（svelte-check + tsc）通过
- [x] 1.4 确认 `pnpm --dir ui run test:unit` 通过

## 2. 命令替换：构建链路

- [x] 2.1 `justfile`：bootstrap / check-ui / test-ui-unit / test-e2e / release-build 五处命令改 pnpm
- [x] 2.2 `.claude/hooks/svelte-check-after-edit.sh`：`npx svelte-check` → `pnpm exec svelte-check`
- [x] 2.3 `src-tauri/tauri.conf.json::beforeDevCommand`：`npm run dev --prefix ../ui` → `pnpm --dir ../ui dev`
- [x] 2.4 `ui/src/lib/tauriMock.bundle.test.ts`：`execSync('npm run build', ...)` → `execSync('pnpm build', ...)` + 同文件注释里的 `RUN_BUNDLE_TESTS=1 npm run test:unit --prefix ui` 也同步

## 3. 命令替换：GitHub Actions

- [x] 3.1 `.github/workflows/frontend-test.yml`：unit job + e2e job 加 `pnpm/action-setup@v4`，`setup-node` 的 `cache: 'npm'` → `cache: 'pnpm'`，`cache-dependency-path` 改 `ui/pnpm-lock.yaml`
- [x] 3.2 `frontend-test.yml`：`npm ci` → `pnpm install --frozen-lockfile`，`npm run test:unit` → `pnpm test:unit`，`npm run check` → `pnpm check`，`npm run test:e2e` → `pnpm test:e2e`
- [x] 3.3 `frontend-test.yml::e2e job`：`npx playwright install` → `pnpm exec playwright install`，Playwright cache key 的 `hashFiles('ui/package-lock.json')` → `hashFiles('ui/pnpm-lock.yaml')`
- [x] 3.4 `.github/workflows/release.yml::build job`：同 3.1/3.2 加 pnpm setup + cache 切换 + `npm ci --prefix ui` → `pnpm --dir ui install --frozen-lockfile` + `npm run build --prefix ui` → `pnpm --dir ui build`
- [x] 3.5 `.github/workflows/ci.yml::openspec job`：保留 `npm install -g @fission-ai/openspec`（D3：runner 预装 npm 无需 pnpm setup 步骤，与 worktree 痛点无关）

## 4. 文档同步

- [x] 4.1 `README.md` 首段安装步骤加 `brew install pnpm` 前置步骤；用户视角"开发"段 `npm run dev --prefix ui` → `pnpm --dir ui dev`
- [x] 4.2 `CLAUDE.md`：陷阱段 `npm run check --prefix ui` / `npm install --prefix ui` / `npm run build --prefix ui` 全替换；测试金字塔 + 浏览器调试入口 + bundle DCE 验证命令同步
- [x] 4.3 `.claude/rules/opsx-apply-cadence.md` 第 5 步 `npm run check --prefix ui` → `pnpm --dir ui run check`
- [x] 4.4 `.claude/rules/codex-usage.md` "npm lockfile" 描述加 pnpm lockfile（语义一致即可）

## 5. spec delta

- [x] 5.1 `openspec/changes/frontend-pnpm-migration/specs/frontend-test-pyramid/spec.md` 写 MODIFIED Requirements 覆盖 4 处命令字符串变化
- [x] 5.2 `openspec validate frontend-pnpm-migration --strict` 通过

## 6. 验证

- [x] 6.1 `just preflight`（fmt + lint + test + test-ui-unit + spec-validate + spec-archive-check + ipc-sync-check）全绿
- [x] 6.2 `just test-e2e`（Playwright user story 5 个 spec）全绿
- [x] 6.3 `pnpm --dir ui run build` 产出 dist 完整；`RUN_BUNDLE_TESTS=1 pnpm --dir ui run test:unit` 通过 mockIPC bundle DCE 校验

## 7. 发布

- [ ] 7.1 push 分支 + 开 PR
- [ ] 7.2 wait-ci 全绿
- [ ] 7.3 codex 二审通过（如发现 bug：修 → push → 回到 7.2 重跑；可循环 M 次）
- [ ] 7.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
