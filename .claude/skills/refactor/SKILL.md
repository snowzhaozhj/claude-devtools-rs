---
name: refactor
description: 代码质量审计工具——扫描指定 scope（file / crate / surface / workspace）输出结构化 findings。
disable-model-invocation: true
---

# refactor

按 scope 扫描代码，识别结构反模式与碰行为契约边界的伪 refactor，输出结构化 findings 表。

Refactor 定义（Fowler）：行为不变的结构改动。

## 1. Scope 选择

**解析优先级**（从上到下，先匹配先用）：

1. **显式 surface flag** `--surface ui` / `--surface ipc`
2. **路径前缀匹配**（最长前缀优先）：`src-tauri/` / `ui/` / `crates/`
3. **路径存在性检查**：参数若是路径（含 `/` 或 `.`）→ `[ -e <path> ]` 校验，不存在报错让用户重试
4. **裸 crate 名解析**：参数无 `/` 且 `crates/<arg>/Cargo.toml` 存在 → 展开为 `crates/<arg>/`
5. **特殊字面量** `workspace` → 全仓
6. **fallback**：单文件按扩展名（`.rs` / `.svelte` / `.ts`）

**消歧速查**：

| 用户输入 | 解析 | 读哪些 references |
|---|---|---|
| `/refactor cdt-api` | step 4 裸 crate → `crates/cdt-api/` | rust + smells |
| `/refactor crates/cdt-api/src/ipc.rs` | step 2 prefix `crates/` | rust + smells |
| `/refactor src-tauri/src/lib.rs` | step 2 prefix `src-tauri/` 优先于 .rs ext | tauri + rust + smells |
| `/refactor ui/src/lib/Foo.svelte` | step 2 prefix `ui/` | svelte + smells |
| `/refactor --surface ipc` | step 1 surface flag | tauri + rust + smells |
| `/refactor workspace` | step 5 字面量 | 全部 4 个 |

读 references 前先 `Glob` / `wc -l` 摸目标体量；> 5000 行先按 crate / 文件群拆。

## 2. boundary-sensitive guard（伪 refactor 识别）

下面 5 类碰了行为契约边界，命中即在 finding 里标 `category: boundary-<n>-<short>`：

1. 改公共 trait / 生命周期约束 / 泛型 bound（即使语义没变也可能 break crate API）
2. async runtime / 调度 / 取消 / 背压 / 错误传播重排（可能改变可观测行为或线程安全契约）
3. Tauri IPC payload schema 改动（字段名 / 序列化形状 / 错误模型）
4. Svelte 5 reactivity 状态流 / 派生值 / 事件时序变动（纯机械 `$:` → `$derived` 不算 boundary）
5. Tauri plugin / capability 边界拆分时 API / permission / command surface 改动

### 降级回"真 refactor"的 4 条证据（满足全部才允许标 `category: structural-*` 而非 `boundary-*`）

**默认每条都不通过**——降级是 fail-safe 反操作，需显式证据逐条 satisfy，模型不允许"凭直觉"判 OK。

- **测试覆盖**（machine gate）：被改 API 有现存测试，`cargo test -p <crate>` 改前 + 改后两次都全 pass。**无现存测试 = 不通过**
- **callsite grep**（machine + manual）：先 `grep -rn '<被改符号>'` 列出所有 caller；逐 caller 显式说明依赖了哪些语义。**caller > 5 处或 cross-crate caller → 默认不通过**
- **IPC contract test 不动**（machine gate）：`cargo test -p cdt-api --test ipc_contract` 跑过且无字段差异（仅 IPC 适用）
- **codex diff 自审**（异构二审）：调 `Agent({ subagent_type: "codex:codex-rescue" })` 让 codex 评估"是否只改实现没改契约"。**SHALL 用 codex 不能自检自己 diff**

任一条不通过 → 保持 `boundary-*` 标记。

## 3. 3 条不变量（真 refactor 主路径）

1. **行为不变** — 缺测先 `cargo test -p <crate>` / `pnpm --dir ui run check` 摸覆盖；测覆盖不到位 SHALL 先补测再改结构。"无测就是 editing 不是 refactoring"
2. **小步提交** — 每步独立 `git revert`
3. **不混 feature / 不顺手优化** — surgical diff；删 dead code OK，补 logging / 加 fallback / "既然在改不如…"不行

## 4. 反模式 catalog（按 scope 选读 references）

按 §1 表选读对应文件：

- `references/code-smells-catalog.md` — 通用结构反模式（god-function / duplicated / long-param / magic-number / nested-conditionals / dead-code / feature-envy / primitive-obsession / inappropriate-intimacy）
- `references/rust-anti-patterns.md` — Rust 错误处理 / 边界可见性 / serde / 测试陷阱 / 模块组织
- `references/svelte-anti-patterns.md` — Svelte 5 runes 误用 / 反应式时序 / 列表 key / 组件边界
- `references/tauri-ipc-anti-patterns.md` — Tauri IPC 契约边界识别

## 5. 输出格式

```markdown
## refactor 审计报告 — <scope>

### 范围
- 目标：<path>
- 扫描文件数：N（按类型：.rs N1 个 / .svelte N2 个 / .ts/.json N3 个）
- 总行数：M
- 基线日期：YYYY-MM-DD（便于跨次 diff 看技术债趋势）

### findings
| 严重度 | 类别 | 位置 | 问题描述 |
|---|---|---|---|
| high | boundary-1-trait-api | crates/cdt-api/src/ipc/list.rs:42 | trait Reader::list 改 `&mut self` 影响公共 API |
| medium | god-function | ui/src/lib/MessageList.svelte:120 | 函数 156 行做 5 件事 |
| low | magic-number | crates/cdt-discover/src/scan.rs:201 | `if depth > 7` 应抽常量 |

严重度：high = 已影响维护性 / 已是 bug 候选；medium = 累积技术债；low = nice-to-have
类别：`boundary-<n>-<short>` 对应 §2 boundary guard 5 类；其它命中 §4 reference 命名（god-function / magic-number / rust-overpub / svelte-key-index 等）
```

定期跑时把每次报告落到 `target/refactor-audit-<YYYY-MM-DD>.md`。
