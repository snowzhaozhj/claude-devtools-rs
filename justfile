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
test-rust:
    cargo test --workspace --exclude cdt-watch
    cargo test -p cdt-watch -- --test-threads=1

# 单 crate 测试，例：`just test-crate cdt-analyze`
test-crate CRATE:
    cargo test -p {{CRATE}}

# 前端 svelte-check + tsc
check-ui:
    npm run check --prefix ui

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

# 校验指定 change，例：`just spec-check 2026-04-17-auto-notification-pipeline`
spec-check CHANGE:
    openspec validate {{CHANGE}} --strict

# ──────── 综合 ────────

# 提交前预检：fmt → lint → test → spec 校验（对齐 .claude/rules/opsx-apply-cadence.md）
preflight: fmt lint test spec-validate

# ──────── 发布 ────────

# 发布前检查：版本号三处一致 + 工作树干净 + preflight
release-check:
    #!/usr/bin/env bash
    set -euo pipefail
    ROOT_VER=$(grep -E '^version\s*=' Cargo.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/')
    TAURI_CARGO_VER=$(grep -E '^version\s*=' src-tauri/Cargo.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/')
    TAURI_CONF_VER=$(grep -E '"version":' src-tauri/tauri.conf.json | head -1 | sed -E 's/.*"version":\s*"([^"]+)".*/\1/')
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

# 本地全量构建 Tauri 安装包（验证 CI 前）
release-build:
    cargo tauri build
