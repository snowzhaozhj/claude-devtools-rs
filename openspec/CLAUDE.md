# openspec/ — spec 工作流与变更约定

仅在 Claude 读写 `openspec/**` 下的文件时由 Claude Code 自动加载。推进节拍详 `.claude/rules/opsx-apply-cadence.md`。

## 三处真相源

- `openspec/specs/<capability>/spec.md` —— 行为契约真相源（archive 自动 sync 回这里）
- `openspec/changes/<slug>/` —— 进行中的 change（proposal + design + tasks + specs delta）
- `openspec/changes/archive/<日期>-<slug>/` —— 历史快照，**冻结**

## 写 spec 前必读

`openspec/SPEC_GUIDE.md` —— spec 该写什么 / 不该写什么 / 反例对照 / 下笔顺序 / reviewer checklist。本文件只列工作流硬约束；写 spec 内容的判断标准全在 SPEC_GUIDE。

## 硬约束

### 1. 不能直接 Edit 主 spec

**任何**对主 spec `openspec/specs/<cap>/spec.md` 的改动——无论修改已有 capability 还是新增——都必须走 `openspec/changes/<slug>/specs/<cap>/spec.md` delta（修改用 `MODIFIED` / `REMOVED`，新增用 `ADDED`），由 `openspec archive <slug> -y` 自动 sync 回主 spec。**禁止**直接 Edit `openspec/specs/<cap>/spec.md`——那是 archive 的产出物，不是输入源。

### 2. archive 是历史快照

`openspec/changes/archive/<日期>-<slug>/` 内的所有文件（含 `specs/<cap>/spec.md` delta、proposal、design、tasks）**冻结**——绝对不要事后 Edit；如需修订同一 capability 行为，开新 change 走 delta。

### 3. 引用约定

CLAUDE.md / TS_BASELINE_DEVIATIONS.md / commit message 引用一个已归档 change 时：

- **只**写 `change <slug>`（如 `change session-detail-lazy-render`）
- **不要**写 `archive 2026-XX-XX-<slug>` 也**不要**写 `openspec/changes/archive/...` 路径

理由：日期前缀只是文件系统位置，不是引用单位；行为契约的真实来源是主 spec。需要溯源到具体 Requirement 时，引用 `openspec/specs/<cap>/spec.md` `<Requirement 标题>`。

### 4. archive 顺序坑（多 change 同 Requirement）

`openspec archive <slug>` 用 delta 的 `MODIFIED Requirement` 完整 body **替换**主 spec 对应 Requirement，**不做三方合并**。

如果你刚 archive 了 change A（修改 Req X），紧接着 archive 一个**更老**的 change B（也修改 Req X 但 delta 里没有 A 的内容），B 的 archive 会把 A 写入主 spec 的内容覆盖丢掉。

规避：
- (a) 按 change 创建顺序 archive，**先老后新**
- (b) 已经倒序 archive 时手工 diff 主 spec、把丢失的段落 merge 回去再 commit

本仓库 commit `1173885` 是案例。

### 5. 行为契约级改动先 propose 再 apply

涉及 IPC 字段语义 / 后端算法 / 状态判定 / 数据 omit 策略 / Tauri command 协议的改动：

1. **先**写 `proposal.md` + `tasks.md`（空 checkbox）+ spec delta 并 `openspec validate <slug> --strict`
2. **再**动 code 边写边勾 checkbox
3. 最后 archive

事后补 change 是已被否决的下策（reviewer 看 PR 时 spec 还旧、propose 阶段的设计取舍机会被跳过）。纯视觉对齐 / 文案 / SVG 路径仍按"小改动直接 commit"。**判断不准默认走 openspec**。

### 6. OpenSpec 工作流走 skill，不要手写

- 开 change：`/opsx:propose <slug>` 一次生成 proposal + design + tasks + specs delta + validate
- apply：`/opsx:apply <slug>` 按 tasks.md 推进
- archive：`/opsx:archive <slug>`（等价 CLI `openspec archive <slug> -y`）

**禁止**手 `mkdir openspec/changes/<slug>` + `Write` 三件套——易漏 design.md、易写错 delta 格式。

`design.md` **不是可选项**——任何 change 都要写明 D1/D2/D3... 决策记录（候选方案 / 取舍 / 风险），让 reviewer 能从设计层评估。

### 7. apply 阶段反转 design 决策时三处同步

实测后发现原 design 决策不符合实际时（典型：change `teammate-message-rendering` 的 D5 把"按 reply_to 紧贴 SendMessage"反转成"按 timestamp 排序"），SHALL **同一个 commit** 内同步三处：

- (a) `design.md` 加 `### D<n>b: ...` 修订块（**不删原 D<n>**，保留决策审计）
- (b) 对应 spec delta 的 Scenario 改写
- (c) `proposal.md` 与 `tasks.md` 的描述

spec 改了但 proposal/tasks 仍写旧策略 = codex / 人审会发现脱节。

## spec delta 写法

`ADDED/MODIFIED Requirement` 体的**第一段**必须含 `SHALL` 或 `MUST`，否则 `openspec validate --strict` 报 `must contain SHALL or MUST`；中文背景描述要放在规约句之后。

`MODIFIED Requirement` 的 title 必须与主 spec 现有 title **字符精确匹配**（whitespace-insensitive，但 backtick / 标点 / 大小写都要一致）——`openspec archive` 用 title 做匹配键去主 spec 找对应段替换，title 不匹配会 `MODIFIED failed for header "..." - not found` 拒 archive。改名场景**必须走 `RENAMED Requirement`**（`FROM:` / `TO:` 形式），不要在 `MODIFIED` 里偷偷改 title。清理 PR 想"顺手把 title 也抽象掉"是高频踩坑（PR #312 案例：title 改名后 archive 拒 3 次，最终恢复原 title 留 follow-up）——title 改名 SHALL 单独走 RENAMED 段，否则保持原 title 不动留 follow-up。

## 推进节拍（速查）

详见 `.claude/rules/opsx-apply-cadence.md`。核心：

- 业务段：Edit → clippy → fmt → test → pnpm check → validate → 勾 checkbox → 文本总结，**不中途停手**
- 发布尾段 N.1-N.4：push → wait-ci → codex 二审 → archive（archive commit 作为 PR 最后一个 commit）

design 阶段 codex 二审默认强制（IPC 字段改 / 跨 capability / 性能关键 / 状态机 / UI 重构 / BREAKING 任一命中即调）。详见 `.claude/rules/codex-usage.md` 第 3 节。

工作目录结构见根 `CLAUDE.md::Workspace layout` 的 `openspec/` 部分。
