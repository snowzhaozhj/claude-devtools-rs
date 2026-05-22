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
    pnpm --dir ui install

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
    pnpm --dir ui run check

# 前端 vitest 单测（含 mockIPC + store + theme + ipc-contract 镜像测）
test-ui-unit:
    pnpm --dir ui run test:unit

# 前端组合测试：vitest + svelte-check
test-ui: test-ui-unit check-ui

# Playwright user story 测试（启 vite dev + chromium 跑 5 spec 文件）
test-e2e:
    pnpm --dir ui run test:e2e

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

# 校验三处 Tauri command 清单 1:1 同步
ipc-sync-check:
    bash scripts/check-ipc-command-sync.sh

# ──────── 综合 ────────

# 提交前预检：fmt → lint → test → 前端 vitest → spec 校验 → IPC 三处同步
# e2e 不在 preflight 内（启动浏览器较慢，由 CI 跑）；本地手动 `just test-e2e` 验证
preflight: fmt lint test test-ui-unit spec-validate spec-archive-check ipc-sync-check

# 四维性能 baseline gate：跑两个 bench 取 min-of-5 对比 tests/perf-baseline.json
# 涉及关键路径的 PR push 前 SHALL 跑；CI 上同名 workflow 仅作 smoke 校验（详 .claude/rules/perf.md "CI 自动 gate"）
perf-check *ARGS:
    bash scripts/run-perf-bench.sh {{ARGS}}

# 真实 Docker SSH e2e：cdt-ssh-test + cdt-cli HTTP/SSE 用户可见行为验收
verify-ssh-docker:
    bash scripts/verify-ssh-docker-e2e.sh

# ──────── 发布 ────────

# 发布前检查：版本号三处一致 + preflight（含 cargo build 会顺带刷新 Cargo.lock）
# bump 版本号后 SHALL 在 commit 前跑——preflight 编译会同步刷新两个 Cargo.lock，
# 全部改动一次 commit 即可，不再需要"commit bump → check → amend lock"两步走
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
    echo ""
    echo "✅ 版本一致，跑 preflight..."
    just preflight
    echo ""
    echo "✅ release-check 通过；下一步："
    echo "    git status  # 确认 Cargo.lock 已同步刷新"
    echo "    git add -A && git commit -m \"chore(release): $ROOT_VER\""
    echo "    git push + PR + wait-ci + merge"
    echo "    git tag v$ROOT_VER && git push origin v$ROOT_VER"

# 本地全量构建 Tauri 安装包（验证 CI 前）；先 build 前端再 tauri build
release-build:
    pnpm --dir ui run build
    cargo tauri build

# 一键 bump：sed 三处版本号 + just release-check + 本地 commit（不 push）
# 用法：先 `git checkout -b chore/release-X.Y.Z`，再 `just release-bump X.Y.Z`
# 后续 push / open PR / wait CI / merge / tag 仍走 Agent 或手工
release-bump VERSION:
    bash scripts/release-bump.sh {{VERSION}}

# ──────── 维护清理 ────────

# 扫 worktree，列出已 merge 且工作树干净的可清理候选（dry-run）
clean-worktrees:
    bash scripts/clean-worktrees.sh

# 真删可清理的 worktree + 本地分支
clean-worktrees-apply:
    bash scripts/clean-worktrees.sh --apply

# 一键清理：merged worktree + 主仓 cargo target + 活跃 worktree 里的 cargo target（dry-run）
clean-all:
    bash scripts/clean-all.sh

# 一键真删（merged worktree + 所有 cargo target）；下次 cargo/tauri 命令需重编译
clean-all-apply:
    bash scripts/clean-all.sh --apply

# ──────── Background 任务分派 ────────
#
# 详见 .claude/rules/bg-task-dispatch.md（拆分判断框架 + bg vs subagent 选择 + 6 个踩坑）
# prompt 模板：.claude/templates/bg-pr-pipeline.md（通用填空）

# 起一个 background claude session 跑 PR 流水线
# 用法（两种皆可，推荐 inline）：
#   just bg-pr <name> '<inline prompt>'        # 短任务直接 inline（含 backtick / 双引号 / $ 都安全）
#   just bg-pr <name> <path-to-prompt-file>    # 长任务 / 想留审计 trail 落文件
# PROMPT 是文件路径还是 inline 字符串由 `[ -f "$PROMPT" ]` 自动判断
#
# Quoting 安全性：用 just `quote()` 函数把 NAME / PROMPT 编码为 shell-safe 单引号字面量，
# 避免 inline prompt 含双引号 / 反引号 / `$` 时被 shell 解释（change `unify-fs-direct-calls` 修订）。
#
# echo 段用 ASCII 半角 + ${var} 显式分隔变量名 —— bash 3.2 (macOS) + `set -u` 下
# 全角中文标点（如 `（` U+FF08 起首字节 0xEF）会被当作变量名延续字符，触发
# `${name<全角字节>}: unbound variable`。
bg-pr NAME PROMPT:
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{justfile_directory()}}"
    name={{quote(NAME)}}
    prompt={{quote(PROMPT)}}
    if [ -f "$prompt" ]; then
        echo "起 bg session: ${name} (prompt 文件: ${prompt})"
        # `cat -- "$prompt"` 避免文件名以 `-` 开头被当 flag；外层 `"$(...)"` 保整个文件内容作单参数
        claude --bg --name "$name" --effort high -- "$(cat -- "$prompt")"
    else
        echo "起 bg session: ${name} (inline prompt)"
        # `--` 隔断让后续 `$prompt`（即便以 `-` 开头）不被当作 flag 解析
        claude --bg --name "$name" --effort high -- "$prompt"
    fi

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
