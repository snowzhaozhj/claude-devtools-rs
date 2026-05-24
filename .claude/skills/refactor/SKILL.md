---
name: refactor
description: 显式触发的代码质量审计 + 重构机会识别工具——按 scope 扫描 Rust crate / Svelte 组件 / Tauri IPC，输出结构化报告（findings 表 + quick wins + 升级项 + 纯结构改项），按 4 路分流给出执行路径。本 skill 是 refactor 请求的**分诊台 + 边界守卫**，不是重构方法论百科：SOLID / 通用重构操作模型自带知识就能填，本 skill 只解决"用户嘴上说重构、这个改动到底该走哪条流水线 + 这段代码哪里糟糕"。**显式调用**：`/refactor [scope]`、"用 refactor skill 看一下 X"、"跑一次代码质量审计"、定期 milestone 审查。**不**用于 bug/报错（→ debug-first）/ 加新行为（→ openspec）/ 启动自检（→ preflight）/ 已在 opsx-apply 流程内的 PR。
disable-model-invocation: true
---

# refactor

显式触发的代码质量审计工具。用户主动调用，**不**靠 description 自动注入主对话。三个使用场景：
1. **指定范围审查**：`/refactor cdt-api` / `/refactor ui/src/lib/SessionDetail.svelte` / `/refactor --surface ipc`
2. **定期 milestone 审查**：每个 release 前 / 大功能合并后跑一次拿基线
3. **接续具体重构 PR**：识别问题后按本 skill 的 4 路分流决定走哪条流水线

## 1. Scope 选择

**解析优先级**（从上到下，先匹配先用，避免歧义）：

1. **显式 surface flag** `--surface ui` / `--surface ipc` → 直接走表第 3-4 行
2. **路径前缀匹配**（最长前缀优先）：以 `src-tauri/` 开头 → tauri + rust + smells；以 `ui/` 开头 → svelte + smells；以 `crates/` 开头 → rust + smells
3. **路径存在性检查**：参数若是路径（含 `/` 或 `.`）→ `[ -e <path> ]` 校验，不存在报错让用户重试
4. **裸 crate 名解析**：参数无 `/` 且 `crates/<arg>/Cargo.toml` 存在 → 当 crate 名展开为 `crates/<arg>/`，按 prefix 规则继续
5. **特殊字面量** `workspace` → 全仓
6. **fallback**：单文件按扩展名（`.rs` → rust+smells / `.svelte` → svelte+smells / `.ts`/`.json` → smells only）

| Scope 形式 | 解析后路径 | 读哪些 references |
|---|---|---|
| `--surface ui` | `ui/` | svelte + smells |
| `--surface ipc` | `src-tauri/` | tauri + rust + smells |
| `<crate-name>` (e.g. `cdt-api`) | `crates/<crate-name>/` | rust + smells |
| `crates/<crate>/...` | as-is | rust + smells |
| `src-tauri/...` (含子路径) | as-is | tauri + rust + smells（含 .rs 时 rust 仍读）|
| `ui/...` | as-is | svelte + smells |
| `workspace` | 全仓 | 全部 4 个 references |
| `<file>.rs` (其它) | as-is | rust + smells |
| `<file>.svelte` (其它) | as-is | svelte + smells |

**消歧示例**：`/refactor src-tauri/src/lib.rs` 路径前缀 `src-tauri/` 优先于扩展名 `.rs`，读 tauri + rust + smells（rust 仍读因为是 .rs）；`/refactor cdt-api` 先查 `crates/cdt-api/Cargo.toml` 存在 → 展开为 `crates/cdt-api/`。

scope 决定要 Read 哪些 `references/*.md`——**不要一次读全 4 个**。读 references 前先 `Glob` / `wc -l` 摸目标体量，> 5000 行的目标先按 crate / 文件群拆。

## 2. 4 路分流（识别问题后选路）

每个发现的问题打上 **suggested path** 标签，落到下面四档之一：

| 改动性质 | 走哪 |
|---|---|
| 行为契约改（IPC 字段语义 / 后端算法 / 状态判定 / 数据流语义 / 错误模型 / 暴露面）| `/opsx:propose → /opsx:apply → /opsx:archive` |
| 性能驱动（启动慢 / 卡顿 / IPC payload 大 / hot path）| `Skill(perf)` 取 4 维 baseline 再决定方向 |
| 跨 crate 大重构（> 2 天 OR 多角色 OR 跨 capability OR 视觉重构）| 升 **agent team**（详 `.claude/rules/parallelism-modes.md::大改动判定`）|
| 纯结构改（rename / extract / move / split / 模块边界 / 删冗余 / 类型收紧 / 早返回 / 提取常量 / dead code）| 直接 feat 分支 → PR → 走 `opsx-apply-cadence::业务推进段 + N.1-N.3 发布尾段` |

## 3. boundary-sensitive guard（看似重构的伪结构改）

**优先级硬规则**：boundary guard **优先于** §2 的 4 路分流。一个 finding 同时命中"纯结构改（rename / extract / move / split / 模块边界 / 类型收紧）"和下面 5 类时——**默认升级**，**禁止**模型自主判定"行为契约不变"绕过。

### 5 类 boundary

1. 改公共 trait / 生命周期约束 / 泛型 bound（即使语义没变也可能 break crate API）→ openspec 或 large cross-crate
2. async runtime / 调度 / 取消 / 背压 / 错误传播重排 → perf 或 openspec
3. **Tauri IPC payload schema** 改动（字段名 / 序列化形状 / 错误模型）→ openspec + IPC 字段 checklist（`src-tauri/CLAUDE.md::IPC 字段改动 checklist`）
4. Svelte 5 reactivity 迁移：纯机械 `$:` → `$derived` 走纯结构改；状态流 / 派生值 / 事件时序变了走 openspec
5. Tauri plugin / capability 边界拆分：API / permission / command surface 不变才走纯结构改；新增暴露面走 openspec

### "证明行为契约不变"的具体步骤（满足全部 4 条才能降级）

只有以下 4 条**全部**通过才允许把 boundary 项降级为纯结构改路径：

- ✅ **测试覆盖**：被改 API / 边界有现存测试，且测试在改前 / 改后跑通且断言不变（运行 `cargo test -p <crate>` 出全 pass 截图或 list）
- ✅ **callsite grep**：用 `grep -r '<被改符号>' --include='*.rs' --include='*.svelte' --include='*.ts'` 扫全仓，确认所有 caller 用法不依赖被改的语义细节（参数顺序 / 返回类型 / lifetime / Send + Sync bound）
- ✅ **IPC contract test 不动**：`cargo test -p cdt-api --test ipc_contract` 跑过且无字段差异（仅当涉及 IPC 时）
- ✅ **diff 自审**：粘贴 diff 给自己（或 codex）问"这是不是只改了实现没改契约"，列出每处具体改动 + 为什么不破坏向后兼容

任一条不通过 → 升级。判断不准默认升级——降级回纯结构改的成本远小于"以为是重构、其实改了行为契约 + 漏 spec delta + codex 抓不到"。

## 4. 4 条不变量（纯结构改主路径）

1. **行为不变** — 缺测先 `cargo test -p <crate>` / `pnpm --dir ui run check` 摸覆盖；测覆盖不到位 SHALL 先补测再改结构。原因："without tests, you're not refactoring, you're editing"——结构改没有行为基线时无法证明等价
2. **小步提交** — 每步独立 `git revert`；按 `opsx-apply-cadence::业务推进段` 节拍走（fmt / clippy / test / push 节拍照旧）
3. **不混 feature / 不顺手优化** — surgical diff；删 dead code OK，补 logging / 加 fallback / "既然在改不如…"不行（karpathy guideline）
4. **性能关键路径前后跑同一 bench** — 涉及 `cdt-discover/` / `cdt-api/src/ipc/` / `cdt-analyze/` 等启动 / IPC / hot path 的结构改，PR 描述 SHALL 贴 4 维对比（详 `.claude/rules/perf.md::PR Perf impact 模板`）

## 5. 输出格式（结构化报告，跨次可对比）

```markdown
## refactor audit — <scope>

### scope
- target: <crate / surface / workspace / file>
- files scanned: N（types: .rs N1, .svelte N2, .ts/.json N3）
- LOC: M
- baseline date: YYYY-MM-DD（便于跨次 diff）

### findings
| severity | category | location | issue | suggested path |
|---|---|---|---|---|
| high | boundary-guard | crates/cdt-api/src/ipc/list.rs:42 | trait Reader::list 改 `&mut self` 影响公共 API | openspec |
| medium | rust-clone | crates/cdt-analyze/src/chunks.rs:89 | hot loop 内 clone 大 Vec<Message> | perf bench 后再 refactor |
| medium | god-function | ui/src/lib/MessageList.svelte:120 | 函数 156 行做 5 件事 | 纯结构改 |
| low | magic-number | crates/cdt-discover/src/scan.rs:201 | `if depth > 7` 应抽常量 | 纯结构改 |

severity：high = 已影响维护性 / 已是 bug 候选；medium = 累积技术债；low = nice-to-have
category：参考 references 各 catalog 的命名

### quick wins (low risk, high value)
- N 项可以本 PR 直接做（rename / dedupe / 提常量 / 早返回）

### 升级到 openspec / perf / team 的项
- N 项 - 列出来给用户拍板，**不**在本次 audit 里执行

### 纯结构改项的执行计划
- 缺测覆盖：列出涉及但无测试的代码区，**SHALL 先补测**
- 性能关键路径：列出涉及但需 bench 对比的路径
- 拆 PR 建议：按 `parallelism-modes.md::4 ✓` 框架（独立 / 可验证 / 工作量值得 / wall time）判断合并 1 PR 还是拆
```

定期跑时把每次 report 存到 `target/refactor-audit-<YYYY-MM-DD>.md` 便于跨次 diff 看技术债趋势。

## 6. 反模式 catalog（按 scope 选读 references）

具体识别用什么 anti-pattern 取决于 scope。以下文件本 SKILL **不预加载**，扫描时按 §1 表选读：

- `references/code-smells-catalog.md` — 通用 10 种（任何文件都先过一遍）
- `references/rust-anti-patterns.md` — Rust 特定（unwrap 滥用 / 隐式 clone / async 内 std::fs / 错误类型分层 / pub 边界 / cache byte cap / lifetime 复杂化）
- `references/svelte-anti-patterns.md` — Svelte 5 特定（runes 误用 / 反应式陷阱 / cache fallback 反模式）
- `references/tauri-ipc-anti-patterns.md` — Tauri 特定（IPC payload > 1MB / schema 漂移 / capability 漏注册 / tauri.conf.json 与 Cargo.toml 不一致）

## 7. Skip 条件

不要在以下情况调本 skill：
- 单点 typo / 单行 fix / docs / 注释级
- bug 排查 / "X 不工作"（→ `debug-first`）
- 已经在 openspec change 走 `/opsx:propose` 流程（design.md 已经覆盖结构决策）
- bump version / lock 同步 / CI 配置微调

## 8. 引用（不复制）

- 分支 / 分流上游：`.claude/skills/preflight/SKILL.md`
- 节拍：`.claude/rules/opsx-apply-cadence.md`
- codex 二审（结构改 SHALL 跑）：`.claude/rules/codex-usage.md`
- 性能（4 维 + 反模式清单）：`.claude/rules/perf.md`
- 升 team / bg 决策树：`.claude/rules/parallelism-modes.md`
- IPC 字段 checklist：`src-tauri/CLAUDE.md::IPC 字段改动 checklist`
- Svelte 渲染 / cache fallback：`ui/CLAUDE.md`
- Rust 边界 / 错误类型：`crates/CLAUDE.md`
