# 项目任务 runner；`just` 或 `just -l` 列出所有 recipe。

default:
    @just --list

# ──────── 构建 ────────

# 编译整个 workspace
build:
    cargo build --workspace

# 编译 src-tauri（独立 manifest，不在 workspace 内）
build-tauri:
    cargo build --manifest-path src-tauri/Cargo.toml

# 启动桌面应用（dev 模式）
dev:
    cargo tauri dev

# 首次 clone 后的一次性依赖安装
bootstrap:
    npm install --prefix ui

# ──────── 测试 ────────

# 全量测试（Rust + 前端类型检查）
test: test-rust check-ui

# Rust workspace + cdt-watch 单线程补跑（FSEvents 并发 flaky，--test-threads=1 稳定）
# `cdt-api/test-utils` feature 启用集成测试访问 cache 内部状态的 helper（详
# change `parsed-message-lru-cache`）；release/默认构建不含。
test-rust:
    cargo test --workspace --exclude cdt-watch --features cdt-api/test-utils
    cargo test -p cdt-watch -- --test-threads=1

# 单 crate 测试，例：`just test-crate cdt-analyze`
test-crate CRATE:
    cargo test -p {{CRATE}}

# 前端 svelte-check + tsc
check-ui:
    npm run check --prefix ui

# 前端 vitest 单测（含 mockIPC + store + theme + ipc-contract 镜像测）
test-ui-unit:
    npm run test:unit --prefix ui

# 前端组合测试：vitest + svelte-check
test-ui: test-ui-unit check-ui

# Playwright user story 测试（启 vite dev + chromium 跑 5 spec 文件）
test-e2e:
    npm run test:e2e --prefix ui

# ──────── Lint + Format ────────

# clippy 严格模式（workspace + src-tauri 两个 manifest 都过）
lint:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings

# 格式化整个 workspace（含 src-tauri）
fmt:
    cargo fmt --all

# ──────── OpenSpec ────────

# 严格校验所有 spec + change
spec-validate:
    openspec validate --all --strict

# 阻止已完成但未归档的 active change 漏进 PR
spec-archive-check:
    bash scripts/check-openspec-archives.sh

# 校验指定 change，例：`just spec-check 2026-04-17-auto-notification-pipeline`
spec-check CHANGE:
    openspec validate {{CHANGE}} --strict

# ──────── 综合 ────────

# 提交前预检：fmt → lint → test → 前端 vitest → spec 校验（对齐 .claude/rules/opsx-apply-cadence.md）
# e2e 不在 preflight 内（启动浏览器较慢，由 CI 跑）；本地手动 `just test-e2e` 验证
preflight: fmt lint test test-ui-unit spec-validate spec-archive-check

# ──────── 发布 ────────

# 发布前检查：版本号三处一致 + 工作树干净 + preflight
release-check:
    #!/usr/bin/env bash
    set -euo pipefail
    ROOT_VER=$(grep -E '^version\s*=' Cargo.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/')
    TAURI_CARGO_VER=$(grep -E '^version\s*=' src-tauri/Cargo.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/')
    TAURI_CONF_VER=$(grep -E '"version":' src-tauri/tauri.conf.json | head -1 | sed -E 's/.*"version":[[:space:]]*"([^"]+)".*/\1/')
    echo "workspace Cargo.toml:     $ROOT_VER"
    echo "src-tauri/Cargo.toml:     $TAURI_CARGO_VER"
    echo "src-tauri/tauri.conf.json: $TAURI_CONF_VER"
    if [ "$ROOT_VER" != "$TAURI_CARGO_VER" ] || [ "$ROOT_VER" != "$TAURI_CONF_VER" ]; then
        echo ""
        echo "❌ 版本号三处不一致，请先同步"
        exit 1
    fi
    if ! git diff --quiet || ! git diff --cached --quiet; then
        echo ""
        echo "❌ 工作树不干净，请先 commit 或 stash"
        exit 1
    fi
    echo ""
    echo "✅ 版本一致 + 工作树干净，跑 preflight..."
    just preflight
    echo ""
    echo "✅ release-check 通过；下一步："
    echo "    git tag v$ROOT_VER && git push origin v$ROOT_VER"

# 本地全量构建 Tauri 安装包（验证 CI 前）；先 build 前端再 tauri build
release-build:
    npm run build --prefix ui
    cargo tauri build

# ──────── 维护清理 ────────

# 扫 worktree，列出已 merge 且工作树干净的可清理候选（dry-run）
clean-worktrees:
    bash scripts/clean-worktrees.sh

# 真删可清理的 worktree + 本地分支
clean-worktrees-apply:
    bash scripts/clean-worktrees.sh --apply

# ──────── Background 任务分派 ────────
#
# 详见 .claude/rules/bg-task-dispatch.md（拆分判断框架 + bg vs subagent 选择 + 6 个踩坑）
# prompt 模板：.claude/templates/bg-pr-pipeline.md（通用填空）

# 起一个 background claude session 跑 PR 流水线
# 用法：just bg-pr <name> <prompt-file>
# 例：  just bg-pr "PR-alpha" .claude/perf-prompts/pr-alpha.md
bg-pr NAME PROMPT_FILE:
    @echo "起 background session：{{NAME}}（prompt: {{PROMPT_FILE}}）"
    @(cd {{justfile_directory()}} && \
      claude --bg --name "{{NAME}}" --effort high \
        "$(cat {{PROMPT_FILE}})")

# 列所有 background session 状态摘要（grep result:/needs input:/failed:）
bg-status:
    #!/usr/bin/env bash
    set -e
    if [ ! -d ~/.claude/jobs ]; then echo "(no bg sessions)"; exit 0; fi
    for id_dir in ~/.claude/jobs/*/; do
        id=$(basename "$id_dir")
        echo "=== $id ==="
        if claude logs "$id" 2>/dev/null | tr -d '\033' | sed -E 's/\[[0-9;]*[a-zA-Z]//g' | grep -aE "result:|needs input:|failed:" | tail -3; then
            :
        else
            echo "  (running 或 logs 未提炼到状态)"
        fi
        echo
    done

# 停所有 background session（不删 worktree）
bg-stop-all:
    #!/usr/bin/env bash
    set -e
    if [ ! -d ~/.claude/jobs ]; then echo "(no bg sessions)"; exit 0; fi
    for id_dir in ~/.claude/jobs/*/; do
        id=$(basename "$id_dir")
        claude stop "$id" 2>&1 | head -1
    done

# 停 + 删某个 bg session 的 worktree（用法：just bg-clean <id>）
bg-clean ID:
    -claude stop {{ID}} 2>&1
    claude rm {{ID}}

# 跑所有 .claude/hooks/*.sh 单次模拟耗时，对比 .claude/rules/hooks-performance.md 预算
# 用法：just bench-hooks       # cold path（99% 不命中关键模式）
#       just bench-hooks --hot # hot path（命中关键模式跑真业务）
bench-hooks *MODE:
    @bash scripts/bench-hooks.sh {{MODE}}
