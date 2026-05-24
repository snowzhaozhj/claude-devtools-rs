---
name: refactor
description: 代码质量审计工具——扫描指定 scope（file / crate / surface / workspace）输出结构化 findings。
disable-model-invocation: true
---

# refactor

按 scope 跑 audit，输出 findings 表。本 skill **只**回答两件事：

1. 这段代码结构上哪里糟糕（god-function / dead code / 错误处理 / 边界可见性 / serde 等结构反模式）
2. 这是真 refactor（行为不变的结构改）还是伪 refactor（看似搬代码、实际碰了行为契约边界）

**不**回答：用什么流程走 / 该走 openspec 还是直接 PR / 怎么 commit / 测试节拍——这些是 Agent 编排的事。本 skill 只识别问题，不决定流程。

**Refactor 定义**（Fowler）：行为不变的结构改动。命名 / 抽函数 / 拆模块 / 删冗余 / 早返回——这些算。改性能 / 改行为 / 修 bug / 加功能 / 调样式——都不算。

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

下面 5 类**不是 refactor**——它们看起来是搬代码，实际碰了行为契约边界。命中即在 finding 里标 `category: boundary-<n>`，**不**作为纯结构改归档。

1. 改公共 trait / 生命周期约束 / 泛型 bound（即使语义没变也可能 break crate API）
2. async runtime / 调度 / 取消 / 背压 / 错误传播重排（可能改变可观测行为或线程安全契约）
3. **Tauri IPC payload schema** 改动（字段名 / 序列化形状 / 错误模型——契约边界）
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

以下文件本 SKILL **不预加载**，扫描时按 §1 表选读：

- `references/code-smells-catalog.md` — 通用结构反模式（god-function / duplicated / long-param / magic-number / nested-conditionals / dead-code / feature-envy / primitive-obsession / inappropriate-intimacy）
- `references/rust-anti-patterns.md` — Rust 特定结构反模式（错误处理 / 边界可见性 / serde / 测试陷阱 / 模块组织）
- `references/svelte-anti-patterns.md` — Svelte 5 特定结构反模式（runes 误用 / 反应式时序 / 列表 key / 组件边界）
- `references/tauri-ipc-anti-patterns.md` — Tauri IPC 边界识别（contract / payload schema / 错误模型）

## 5. 输出格式

```markdown
## refactor audit — <scope>

### scope
- target: <path>
- files scanned: N（types: .rs N1, .svelte N2, .ts/.json N3）
- LOC: M
- baseline date: YYYY-MM-DD（便于跨次 diff）

### findings
| severity | category | location | issue |
|---|---|---|---|
| high | boundary-1-trait-api | crates/cdt-api/src/ipc/list.rs:42 | trait Reader::list 改 `&mut self` 影响公共 API |
| medium | god-function | ui/src/lib/MessageList.svelte:120 | 函数 156 行做 5 件事 |
| low | magic-number | crates/cdt-discover/src/scan.rs:201 | `if depth > 7` 应抽常量 |

severity：high = 已影响维护性 / 已是 bug 候选；medium = 累积技术债；low = nice-to-have
category：`boundary-<n>-<short>` 表示伪 refactor（命中 §2）；`structural-*` 或具体反模式名（god-function / magic-number / rust-overpub 等）表示真 refactor 候选。**不**输出 suggested path / suggested skill——那是 Agent 的事

### out-of-scope（识别为非 refactor 问题）
- 命中 perf 反模式 / 已知 bug 信号 / 视觉问题 / 测试基础设施问题的 finding 在此列出，**只列位置 + 简短描述，不分类不打 category**——交给 Agent 决定怎么处理
```

定期跑时把每次 report 落到 `target/refactor-audit-<YYYY-MM-DD>.md` 便于跨次 diff 看技术债趋势。
