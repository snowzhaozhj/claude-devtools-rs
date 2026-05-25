## Context

`openspec/specs/<cap>/spec.md` 是行为契约真相源（`openspec/CLAUDE.md::硬约束 1`）。S+XS 档 18 个 cap 累计 ~3.4k 行 / ~1.0k 反引号——其中相当一部分是把"内部 fn 名 / 源码路径 / `tracing::xxx!(target:...)` 实现选择 / IPC contract test 级 scenario / 每枚举一个 scenario"塞进 spec 的产物。结果：

1. spec 把 implementation note 与行为契约混着写；reviewer 评估行为变更要先扒掉实现路径。
2. IPC contract test 级 scenario（`EXPECTED_TAURI_COMMANDS` / `KNOWN_TAURI_COMMANDS` / `invoke_handler!` 三处同步）落在 `app-auto-update` / `server-mode` 等多个 cap 内，本属 `frontend-test-pyramid::Rust IPC contract test 守护字段形状` 一处兜底。
3. `tracing::xxx!(target:...)` 是诊断手段而非用户可观察行为；调用方代码改 target 不应破契约。

L 档已用同口径（先前 PR）跑过一次。本 change 把同样的 6 条尺子下降到 S+XS 档：

1. 内部 fn / type / mod 名 → 行为描述（IPC 对外字段名保留）
2. 源码路径 / crate::module 引用 → 删
3. 实现选择类 SHALL（`tracing::xxx!(target:...)` / `Mutex<Option<ServerHandle>>` 等）→ 删，必要时移 design.md
4. IPC contract test 级 scenario（`EXPECTED_TAURI_COMMANDS` 同步、`<原字段>Omitted` 命名规范单测）→ 下沉到 `frontend-test-pyramid` 兜底，spec 内删除
5. "每枚举 / 每按键 / 每 hotkey 一个 scenario" → 合成"行为类别 + spec id 白名单"
6. issue 号 / PR 号 / 原版 TS 文件路径（`TeammateMessageItem.tsx` 等）→ 删

约束：
- normative SHALL/MUST 句行为语义不变（删的是实现路径，不是契约规则）
- capability 边界不动（边界重构走 GitHub Issue #296）
- IPC 字段名 / 字段语义 / Tauri command 名 / SSE event 名 / `xxxOmitted` 命名规范一律保留
- 仅修 `openspec/specs/<cap>/spec.md`（通过 `openspec/changes/openspec-slim-S-tier/specs/<cap>/spec.md` delta + archive sync）

## Goals / Non-Goals

**Goals:**

- G1：18 个 S+XS cap 的 spec.md 按 6 条尺子瘦身；IPC 字段名 / `xxxOmitted` 命名 / Tauri command 名 / SSE event 名 / 性能预算等行为契约 byte-equal 不变
- G2：normative SHALL/MUST 句一句不动语义——只重写表述里嵌入的实现路径 / fn 名 / module 名
- G3：颗粒过细 scenario 合成"行为类别 + 白名单常量"，scenario 数下降但行为覆盖不退化

**Non-Goals:**

- 不动 capability 边界（移 Requirement 跨 cap 走 GitHub Issue #296）
- 不补 missing scenario / 不修 main 上既有行为 bug（走 GitHub Issue）
- 不动 design.md 已 archive 的内容（archive 是冻结快照）

## Decisions

### D1：每个 cap 的 delta 用 `MODIFIED Requirement` 全文重写

`openspec validate --strict` 要求 delta 的 `MODIFIED Requirement` body 含完整 `### Requirement: ...` 标题 + 第一段含 SHALL/MUST + 全部 `#### Scenario:` 段（archive 时按整个 Requirement body 替换主 spec）。本 change 每个 cap 的 delta 都用 `MODIFIED Requirement` 全文重写，**不**用 `REMOVED Requirement` + 重新 `ADDED Requirement` 二段式（更不易读、reviewer 难 diff）。

### D2：IPC contract test 级 scenario 下沉到 `frontend-test-pyramid`，不在 capability spec 重复

`app-auto-update::IPC 字段约定同步` Requirement 的 Scenario "IPC 字段约定同步"（断言 `check_for_update` 加进 `EXPECTED_TAURI_COMMANDS` / `KNOWN_TAURI_COMMANDS` / `invoke_handler!` 三处）以及 `server-mode::Tauri 桌面应用 SHALL 暴露 server lifecycle IPC 控制` 中类似的 "3 个 command 名 SHALL 加入 ..." 段——这是 `frontend-test-pyramid::Rust IPC contract test 守护字段形状` 的 `Scenario: 新加 command 必须新加 contract test` 已兜底的契约。在每个新 IPC command 的 cap spec 里复述一遍属于跨 cap 重复。

**做法**：删除这些 scenario / 段；行为契约由 `frontend-test-pyramid` 一处统一守护。reviewer 看 PR 时若新加了 IPC command，去 `frontend-test-pyramid::Rust IPC contract test 守护字段形状` 找契约。

### D3：`tracing::xxx!(target:...)` 实现选择 SHALL 一律删

`app-auto-update` / `server-mode` / `notification-triggers` / `file-watching` / `wsl-distro-discovery` 等 cap 反复出现 `tracing::error!(target: "cdt_xxx::yyy", ...)` SHALL 句——这属于"诊断手段"而非用户可观察行为契约。

**做法**：spec 里把"`tracing::xxx!(target:...)` 记录"删除；如果该日志是行为前提（如 D8 提到的 `degrades to size-only fingerprint`），改写为"系统 SHALL 把该退化路径在日志中标注"等行为级表述，不绑死 `target` 字符串。

理由：调用方改日志 target / 改 log level / 切到结构化字段时不应触发 spec validate failure；契约的 surface 是用户看到的行为（pill 不显示 / inline 错误显示），不是 log line 形态。

### D4：内部 fn / type / mod 名一律降为"行为描述"，IPC 入口公开 fn 保留

**删除示例**：

- `team-coordination-metadata::Detect operational noise...` 段第 139 行 `实现 SHALL 落在 cdt-analyze::team::noise 模块，导出 detect_noise(...) 与 detect_resend(...) 两个纯函数` —— 改成"系统 SHALL 暴露纯函数级 noise / resend 检测路径，独立单测覆盖"
- `team-coordination-metadata::Link teammate messages...` 第 84 行 `实现 SHALL 把配对算法落在纯函数 team::reply_link::link_teammate_to_send_message(teammate, candidate_chunks, used_set) -> Option<String>` —— 改成"系统 SHALL 把配对算法实现为纯函数（无副作用、可独立单测覆盖）"
- `notification-triggers::Notifier 按 FileSignature 缓存...` 第 116 行 `详 design D1f` 引用 —— 改成行为表述（"Windows 与其它平台允许退化为仅依赖 mtime+size 的 best-effort 等价"）
- `memory-viewer::Operate memory CRUD over current backend` 第 83 行 `cdt-fs::FileSystemProvider` trait —— 改成"通过统一的文件系统抽象层调用"

**保留示例**（IPC 入口公开 fn 名 / Tauri command 名是契约的一部分）：

- `agent-configs::Expose agent configs through data API` 中 `read_agent_configs()` Tauri command 名（前端用这个名字 invoke）—— 保留
- `app-auto-update::手动检查更新 IPC` 中 Tauri command 名 `check_for_update` —— 保留
- `wsl-distro-discovery::枚举本机 WSL distro` 中 IPC command 名 `list_wsl_distros` 与字段名 `WslDistroScanReport { candidates, distrosWithoutHome }` —— 保留

### D5：颗粒过细 scenario 合并为"行为类别 + 白名单"

`tab-management::Pane 生命周期` 内 8 个 hotkey scenario（`pane.split` / `pane.focus.next` / `pane.focus.prev` / `tab.switch.<n>` / `tab.close` / `tab.next` / `tab.prev` 等各 1 个 scenario）——每个 hotkey 一个 scenario 颗粒过细，行为类别一致：键盘事件命中 spec id → registry dispatcher 命中 → 对应 pane / tab 操作。

**做法**：合并为 1-2 个"行为类别"scenario：(a) "用户按下任一 pane / tab spec id 当前 binding，registry dispatcher SHALL 命中该 spec → 触发对应 pane / tab 操作"，(b) "用户改自定义 binding 后 SHALL 生效"。配合白名单常量列表（pane.split / pane.focus.next / pane.focus.prev / tab.switch.1..9 / tab.close / tab.next / tab.prev）放进 Requirement body 而非每个一 scenario。

### D6：原版 TS 文件路径 / `Sidebar.svelte` 等组件名作为"举例"保留 vs 删除

判定标准：

- **作为契约目标本身保留**：`frontend-test-pyramid::改 UI 组件触发 Playwright 而非 vitest 组件测` 中提的 `ui/src/components/Sidebar.svelte` —— 是契约规则的"输入"（修改这种文件 → 走某层测试），不是实现细节。保留。
- **作为实现细节删除**：`team-coordination-metadata::Detect operational noise...` 中 `正则集与原版 TeammateMessageItem.tsx::RESEND_PATTERNS 同一` —— TS 原版是历史参考，spec 不应绑死。改成"正则集 SHALL 覆盖 resend / re-send / sent earlier / already sent / sent in my previous 五种关键词"。

### D7：`session-search` / `notification-ui` 不写 delta

扫描后这两个 cap 的 spec 已经按行为契约写——`session-search` 83 行 / 11 scenario 全是 input → output 行为；`notification-ui` 203 行 / 30 scenario 也都是用户行为类。没有 6 条尺子能命中的内容。

**做法**：本 change 不为这两个 cap 写 delta（无须改）。proposal.md 也明示这两个 cap 已干净。

## Open Questions

无：6 条尺子明确，每个 cap 的瘦身候选可在 apply 阶段逐 cap 落地。

## Risks / Trade-offs

**风险**：

- R1：删某个 fn 名后该 fn 被 reviewer / 后续 PR 改名失去 spec 提示——`team::reply_link::link_teammate_to_send_message` 删后下次重命名 fn 不会触发 spec 改动。**mitigation**：fn 名是实现细节本就不该锁死；行为契约（"配对算法 SHALL 是纯函数"）保留，单测仍守住。
- R2：IPC contract test scenario 删后，新加 IPC command 的 reviewer 可能找不到"加 IPC 后该改哪些列表"的提示。**mitigation**：`frontend-test-pyramid::Rust IPC contract test 守护字段形状::Scenario: 新加 command 必须新加 contract test` 已 normalize 这一契约，PR CI 会拦。

**Trade-off**：

- T1：spec 文本长度下降（目标 18 个 cap 净减 ~200 行 / ~150 反引号），不再当 implementation note 用。代价：第一次读 spec 的人需要去 design.md / `crates/<cap>/CLAUDE.md` 找实现指针。**接受**——spec 是行为真相源，不是 onboarding 文档。

## Migration Plan

archive 后 spec.md 同步生效。无运行时 migration 需求（spec 改动不动代码）。
