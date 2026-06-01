---
name: dep-update
description: 检查 Cargo workspace + ui 前端依赖更新状态，列出可升级项、breaking risk、安全公告。**只要**用户说"检查依赖 / 依赖更新 / outdated / 有没有新版本 / upgrade deps / 升级依赖"或显式 `/dep-update`，**都用这个 skill**。
disable-model-invocation: true
---

# dep-update

检查项目所有依赖的更新状态，输出结构化报告。

## 流程

### 1. Rust workspace 依赖

```bash
# 列出可更新的依赖（不实际修改 Cargo.lock）
cargo update --workspace --dry-run 2>&1 | grep -E "Updating|Adding|Removing" | head -40

# 检查已知安全漏洞（需 cargo-audit 已装）
cargo audit 2>/dev/null || echo "cargo-audit 未安装，跳过安全检查（建议：cargo install cargo-audit）"
```

### 2. src-tauri 依赖（独立 manifest）

```bash
cd src-tauri && cargo update --dry-run 2>&1 | grep -E "Updating|Adding|Removing" | head -20
```

### 3. UI 前端依赖

```bash
pnpm --dir ui outdated 2>/dev/null || npm --prefix ui outdated 2>/dev/null
```

### 4. 输出报告

按以下格式汇总：

```markdown
## 依赖更新报告

### Rust workspace
| crate | 当前 | 可升级到 | breaking? | 备注 |
|-------|------|----------|-----------|------|

### src-tauri
| crate | 当前 | 可升级到 | breaking? | 备注 |

### UI (pnpm)
| package | 当前 | 最新 | breaking? | 备注 |

### 安全公告
- （cargo audit 结果）

### 建议
- 哪些可以安全 bump（patch/minor）
- 哪些需要注意 breaking change（major）
- 哪些有安全公告需优先处理
```

## 注意事项

- **不修改任何文件**——只读检查 + 报告
- breaking change 判断：major version bump = breaking；tauri 生态 minor bump 也可能 breaking（查 changelog）
- 如果用户想实际升级，告知用 `cargo update -p <crate>` 或 `pnpm --dir ui update <pkg>` 手动操作，并跑 `just preflight` 验证
