# SPEC_GUIDE — `openspec/specs/<cap>/spec.md` 写什么、不写什么

> 给写 spec 与 review spec PR 的人 / agent 用。不是 lint，是下笔与判断的指引。
>
> 与既有产物的关系：
> - 6 条**已实践的"删除尺子"** 见 change `openspec-slim-S-tier::design.md`（PR #300）与 `openspec-slim-M-tier`（PR #301）。本文不重复，只引用。
> - **capability 边界重构**远期计划见 GitHub Issue #296，本文不规划拆分。
> - **工作流硬约束**（不直接 Edit 主 spec / archive 冻结 / 引用约定 / archive 顺序坑等）见 `openspec/CLAUDE.md`，本文不重复。

## 一句话原则

`spec.md` 描述「**用户能感知什么 + 系统对外承诺什么行为**」；`design.md` 描述「**内部怎么实现**」。

如果一段内容，把它的实现换一种方式但**用户感知与外部协议不变**，则该内容不属于 spec，应在 design.md。

## 写 spec 的 4 层骨架

按下列顺序下笔，**不要**反向（先想实现再倒推 spec）。

1. **Purpose**（一段，用户价值）—— 删了这个 capability 用户会失去什么、为谁存在。**禁止**写"基于 X 库实现"、"由 Y 模块承载"等 how。
2. **Functional Requirements**（FR）—— 用户能干什么 / 系统对外暴露什么行为。
3. **Non-Functional Requirements**（NFR）—— 性能预算 / 容量上限 / 延迟阈值等数字契约。**与 FR 分开**写成独立 Requirement，不与功能行为混。
4. **Cross-references**（可选）—— 用 `[[other-capability]]` 引用，不复制；**外部协议（IPC payload / HTTP path / push event）单一 owner**，跨 spec 不重复定义同一协议字段。

> Requirement / Scenario 的格式要求（SHALL/MUST、`#### Scenario:` WHEN/THEN、4-hashtag 必填、每个 Requirement 至少 1 个 Scenario 等）由 OpenSpec 官方 schema instruction 兜底，本文不重复——本文只补**项目特定的内容判断与边界**。

## 该写什么 vs 不该写什么（对照表）

| 内容 | 归宿 | 理由 |
|---|---|---|
| 用户能看到 / 操作 / 感知什么 | spec | 行为契约 |
| IPC payload 字段名（camelCase）/ Tauri command 名 / SSE event 名 | spec | 外部协议 |
| 错误码 / 错误 variant 名 | spec | 外部契约 |
| 性能预算（具体数字）/ 容量上限 / 延迟阈值 | spec NFR | 外部可观察 |
| `xxxOmitted` 类 omit 字段语义 | spec | 协议契约 |
| 内部 fn / type / mod / struct field 名 | design | 实现 |
| 源码路径（`crates/...` / `src-tauri/...` / `ui/src/...`）| design | 位置 |
| Rust / TS 类型签名（`Vec<T>` / `Result<T,E>` / `Option<T>`）| design | 语言绑定 |
| 库选型（`tracing` / `tauri-plugin-X` / `tokio` / `serde` / ...）| design | 实现选择 |
| 实现选择 SHALL（"通过 X 记录"、"用 Y 缓存"）| design | 机制 |
| log target 字符串（`target: "cdt_xxx::yyy"`）| design | 诊断 |
| 测试 fixture 常量名（`EXPECTED_TAURI_COMMANDS` 等）/ 测试文件路径 | 测试代码 + `frontend-test-pyramid` 兜底 | 测试组织 |
| 配置文件字段路径（`tauri.conf.json::xxx`）| `release-runbook` skill / runbook | 部署 |
| Cargo / npm 依赖名 + 版本 | release notes / runbook | 依赖管理 |
| commit / PR / issue 号 / 原版 TS 文件路径 | git 历史 / design 论证段 | 诊断溯源 |
| 实测数据（"95ms"、"60-74ms"、baseline 数字）| `tests/perf-baseline.json` / design | 实证非契约 |
| 回滚开关 const 名（`OMIT_*` / `CROSS_*` / `STALE_*`）| design + runbook | 实施细节 |

## 反例 → 修法（3 段真实对照）

### 反例 1：日志调用作为 Scenario AND 子句

❌ 当前（`app-auto-update::Scenario: 检查更新失败`）：
```
- AND `tracing::error!(target: "cdt_tauri::updater", ...)` SHALL 记录该事件
```

✅ 改写：
```
- AND 系统 SHALL 在日志中标注更新检查失败（含错误来源），
       不绑死具体 log crate / target 字符串
```

理由：日志路径是诊断手段；改 log target / 切换 log crate / 改用结构化字段都不应破契约。

### 反例 2：测试 fixture 当 SHALL MUST

❌ 当前（`app-auto-update`）：
```
- THEN `crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS`
       MUST 包含 `"check_for_update"`
- AND  `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS`
       MUST 包含 `"check_for_update"`
```

✅ 改写：删除这两行；行为契约由 `frontend-test-pyramid::Rust IPC contract test 守护字段形状` 一处兜底。

理由：测试组织是测试代码自己的事；spec 只承诺"系统 SHALL 暴露 `check_for_update` Tauri command"。

### 反例 3：Rust 类型签名当行为契约

❌ 当前（`agent-configs`）：
```
调用方 IPC 路径 `LocalDataApi::read_agent_configs` SHALL 在构造
`Vec<(project_id, cwd)>` pairs 时 ... `cdt_discover::agent_configs::
read_agent_configs(pairs)` 公开签名不变。
```

✅ 改写：
```
系统 SHALL 暴露 `read_agent_configs` Tauri command，按 (project, cwd)
对枚举本机所有 agent 配置文件，session 无 cwd 时退化为项目根。
```

理由：Rust 类型 / module path / 公开签名都是实现选择；外部 Tauri command 名是协议、保留。

更多反例见 PR #300 design.md `D1-D6`。

## 写新 spec 的下笔顺序

1. 先回答 **"为什么存在这个能力 / 为谁服务"**——写 Purpose 一段（用户视角，不是实现概要）。
2. 用**用户视角的能力地图**列 Requirement——按用户能看到 / 操作 / 感知的维度组织，不按"启动期 / 后台扫描期 / 渲染期"等技术分层组织，更不按 PR 时间追加到末尾。
3. 每个 FR Requirement 体说清楚一个外部可观察契约（格式见上方"4 层骨架"段引用的官方 instruction）。Scenario 标题用**用户/系统视角**短语，禁止用 const 名 / 回滚开关名 / 实现术语。
4. NFR 单独写成 Requirement（"性能预算"、"容量上限" 等），不掺进 FR Body。
5. 跨 capability 引用用 `[[other-cap]]`；外部协议字段（IPC / SSE / HTTP）保留单一 owner spec，其它 spec 引用不复制。
6. **写完检查**：把 Purpose 给一个不懂代码的产品同事看，他能否说出"这个能力对用户做什么"；把 Requirement 给后端 / 前端各看一遍，他们能否各自实现却不撞协议。

## 改既有 spec 的判断顺序

1. 这次改动是**行为契约改动**还是**纯实现重构**？纯实现重构的话 spec 不该动（除非要顺手清前面落进来的实现细节，那是 cleanup change）。
2. 行为契约改动按 `openspec/CLAUDE.md::硬约束 5` 走 propose → apply → archive 流程。
3. 写 spec delta 时**默认用现有反例对照表自检一遍**——新写的 SHALL 句是否含上表"不该写"的内容；含了就移到 design.md。

## reviewer 看 spec PR 的 checklist

按重要性排：

- [ ] Purpose 是否描述用户价值，没写成实现概要？
- [ ] 新 / 改 SHALL 句是否含库名 / Rust 类型签名 / log target / 内部 fn 名？
- [ ] Scenario 标题是否描述用户 / 系统可观察行为，没用 const / 回滚开关 / 实现术语命名？
- [ ] 跨 spec 同一外部协议字段是否被重复定义（grep 一下字段名是否在多个 spec 出现 SHALL）？
- [ ] FR 与 NFR 是否分开，没混在同一个 Requirement Body 里？
- [ ] 测试 fixture 常量 / 测试文件路径 / Cargo 依赖 / 配置文件字段是否被错放进 SHALL？

发现问题时建议表述："这条 SHALL 实际承诺的是 *用户感知层* 的什么？换种实现还成立吗？" —— 让作者自检比直接挑刺好。

## 处理历史污染（archive port-* sync 进主 spec）

历史 `port-*` 系列 change 在移植期把"TS → Rust 模块映射 / 数据结构 / 公开签名"写进 delta，archive 时这些被自动 sync 到主 spec，**主 spec 至今还有可观察量的实现笔记残留**。

处理原则：

- **不动 archive**（冻结快照，硬约束 2）。
- **遇到一个修一个**：任何 PR 改动到含历史污染的 Requirement 时，顺手按反例对照表清理；这次没改的 Requirement 不强制清。
- **批量清理走 cleanup change**：如 PR #300 / #301 那种纯瘦身 PR，每个 cap 走 `MODIFIED Requirement` 全文重写（不用 `REMOVED + ADDED` 二段式）。
- **capability 边界重构**走 GitHub Issue #296 单独立项，不混在瘦身 PR 里。

## 不做的事（避免范围蔓延）

- 不引入自造黑名单 hard-fail lint
- 不强加 spec 体积 / Purpose 长度 / Scenario 数量上限
- 不重复 OpenSpec 上游已有的 validation（`MIN_PURPOSE_LENGTH=50` / `MAX_REQUIREMENT_TEXT_LENGTH=500` 等由 `openspec validate --strict` 兜底）
- 不发明 `spec.alias` / `redirect-to` 等上游不识别的概念

## 与既有 lint 的关系

`scripts/check-spec-purity.sh` 抓 6 类**词法**反模式（mod-path / src-path / commit-hash / metric / impl-flag / lib-framework），baseline ratchet 模式防恶化——它的命中可以当**这份指引的雷达提示**：词法命中 ≈ 极可能违反本文规则；但反过来不成立（语义违规未必有词法痕迹）。

判断主权在 reviewer + codex 二审手里，不在词法 lint 上。
