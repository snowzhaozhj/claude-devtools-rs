# Design — openspec-slim-M-tier

## 上下文

`change spec-purity-mechanism` 已落地"反模式注入到 propose 流程"，但已存在的 8 个 M 档 capability 在 port 期写入大量实现描述。它们的"反引号密度 + 行数"显著高于其他 capability，是 reviewer / spec-fidelity-reviewer 心智负担的主要来源。L 档（>600 行）单独走 spec-slim-L，本 change 收敛 M 档（300-600 行 / 反引号密度 > 0.4 反引号/行）8 个。

## 目标 / 非目标

**目标**：

- 把 8 个 M 档主 spec 行数砍到表格目标值
- 行为契约 SHALL/MUST 句逐条保留（以"删了之后是否影响测试 / 是否让外部观察到不同"判定）
- IPC 对外字段名（`projectId` / `sessionId` / `chunkId` / `messageCount` / `gitBranch` / `isOngoing` / `xxxOmitted` 等）原样保留——前端契约不变
- 把"实现选择 SHALL 句"中真正承载决策审计价值的内容移本 design.md `## Decisions`（不丢决策审计）

**非目标**：

- 不重新划分 capability 边界（边界重构走 Issue #296）
- 不修改 IPC 字段语义、enum tag、camelCase 序列化规则
- 不动测试代码（仅 spec.md 文档级 + 必要时 tasks.md 留 followup）
- 不动 ADDED / REMOVED Requirement——仅 MODIFIED 既有 Requirement body / Scenario

## D1: 瘦身 6 条尺子（保留行为契约的判定原则）

任何"删 / 合并 / 改写"动作 SHALL 通过以下 6 条尺子判定，每条 reviewer 可对照逐句 audit：

1. **内部 fn / type / mod 名（backtick 包的 Rust 标识符）→ 改成行为描述**。例外：IPC 对外字段名（前端可见）保留。
2. **源码路径 / `crate::module` 引用 / 测试文件名 / 行号锚点 → 删**。具体实现位置随重构漂移，不属契约。
3. **实现选择类 SHALL 句**（"走 trait 默认 fallback" / "由 X crate 主动填充" / "在 Y 模块内分流"）→ 删，必要时移 design.md `## Decisions`。
4. **IPC contract test 级 scenario**（"序列化产出含 / 不含 X 键"）→ 下沉测试，spec 删；测试不存在则 tasks.md 加 "补 IPC contract test"。
5. **"每工具 / 每按键 / 每命令一个 scenario" 颗粒过细 → 合成"行为类别 + 白名单常量"**。例：`Numpad 数字键 1..9` 各一个 scenario → 一句"Numpad 数字键 SHALL 与顶部数字键同义"。
6. **issue 号 / PR 号 / 原版 TS 路径 / `change <slug>` 引用 → 删**。历史决策溯源走 `git log` / archive 目录。

**判定要旨**：句子答得了"如果删了这一句，测试 / 外部观察者的契约会变化吗？"= 保留；答不了 / 答"不会变"= 删 / 移 design。

## D2: 不变量

- SHALL / MUST normative 句**保留**（删一句 SHALL = 缩小契约 = 行为变更）
- capability 边界**不动**（无新增 / 删除 / 移 Requirement）
- IPC 字段名 / 字段语义**不动**
- 仅修 `openspec/specs/<cap>/spec.md`（通过 delta sync）与 `openspec/changes/<slug>/`
- spec delta 仅 MODIFIED Requirement，body 第一段 SHALL/MUST 句必含

## D3: 8 个 cap 的瘦身侧重（决策审计）

| capability | 主要瘦身路径 | 移到 design 的内容 | 估行 |
|---|---|---|---|
| configuration-management | 合并三个 enum 字段（`externalEditor` / `terminalApp` / `searchEngine`）的 per-value scenario 为"白名单 + invalid 拒"两条 | 跨平台 fallback 策略、`pre-merge-composite.bak` 备份命名 | < 400 |
| fs-abstraction | 剔除 H1-H6 enforce 路径（xtask 路径 / 测试文件名）；12 方法 trait 的 attribute 链改述行为；`InstrumentedFs` 内部接线细节移 design | counter 注入机制（`task_local!` / wrapper 链路）；`StaleCheckStrategy` 设计 | < 350 |
| tool-execution-linking | "工具块右键菜单"两 Requirement 收敛为"右键菜单契约引用 frontend-context-menu" | `SendMessage` 4 个 type branch 的具体语义保留为 SHALL（不属"实现选择"） | < 250 |
| chunk-building | `Embed teammate messages` 5 步实现细节移 design；删 `EMBED_TEAMMATES=false` 回滚 Scenario | 5 步状态机（`pending_teammates` 缓冲 / `flush_buffer` 触发 / interrupt 分支） | < 230 |
| project-discovery | 合并 Windows 路径解析 4 Scenario 为单一 fallback 链；删 `worktree_grouper.rs:78-117` 源码引用 | git identity 解析的"0 git 子进程 syscall"性能基线（保留为 NFR 附注） | < 320 |
| http-data-api | 三组路由表（项目/会话/搜索/通知/lazy）合并为"路由是 IPC 镜像 + camelCase 一致"行为契约 + 完整路由表移 design | EVENT_BRIDGE_CAPACITY = 1024 决策、SSE producer 5 信号源装载 | < 270 |
| keyboard-shortcuts | `normalizeBindingToMod` 11 步算法 → "幂等 / 跨平台一致 / 保留辅助修饰键 / token-level 不重排"四条契约 | token-level 算法实现细节、Numpad 归一化清单 | < 300 |
| frontend-context-menu | 8 个 factory function 名清单 → "按 surface 分 factory（用户消息 / 助手消息 / Bash 工具 / 文件工具 / 选区 / project / worktree / tab）" | 三层 handler 分流（Layer1 surface / Layer2 selection / Layer3 fallback）触发顺序 | < 320 |

## D4: 决策审计的接续位置

- "实现选择"的 D 决策（如 `configuration-management` 的"跨平台 enum 不报错而是 fallback"）原本散在 SHALL 句里——本 change 收敛后 SHALL 把它们沉淀为本 design.md 的 D 决策块（D-Impl-1, D-Impl-2, ...），让 reviewer 读 design 而非读散在 spec 里的实现注释。
- 已 archive change 的 design.md（典型 `add-keyboard-shortcut-system` 的 D4 pending overlay 冲突检测）SHALL 作为新 design 的"上游决策引用"用 `[[archived-slug]]` 链接，**不**复制内容。

## D-Impl-1: configuration-management 跨平台 enum 不严格校验

`terminalApp` 跨平台 enum（macOS 写 `windows_terminal` / Windows 写 `i_term`）SHALL 接受持久化，运行时 `open_in_terminal` 调用时 warn 级日志记录 + fallback 平台默认终端。

**为什么保留在 design 而非 spec**：fallback 文案 / log 级别 / 不报错决策属于运行时容错策略，把它放主 spec 会让契约绑死实现行为；放本 design 让未来策略调整（如改成"首次跨平台不报错但持续 N 次后弹通知"）不需要走主 spec delta。

**理由**：用户跨平台同步配置（典型 dotfiles git）时不希望被严格 enum 校验拒绝；运行时 fallback 让"配置可移植 + 行为容错"两边都拿到。

**反论点（被拒绝的替代方案）**：
- "严格 enum 校验跨平台值拒绝"——拒绝理由：违背配置可移植性，dotfiles 同步场景永远拒。
- "持久化但运行时直接报错给用户弹模态"——拒绝理由：用户体验断崖，typical 场景仅是悄悄跨平台同步，不该弹模态打断。

## D-Impl-2: fs-abstraction `InstrumentedFs` 注入机制

counter 通过 task-local + `InstrumentedFs<P>` wrapper 在 trait 调用边界自动计数；provider 实现 SHALL NOT 内嵌 record hook（向后兼容、零侵入测试）。

**为什么保留在 design 而非 spec**：注入机制（task-local vs 全局 atomic vs 显式 ctx 传参）是实现路径选择，未来若 tokio 替换为别的 runtime 或加新 wrapper 中间层都不希望走主 spec delta；spec 只规约"counter 不跨 task 污染 / wrapper 自动计数 / 未包 wrapper 不计数"三个可观察行为。

**理由**：让 fake provider / 真实 provider 共享同一计数代码路径，避免每个 provider 维护一套 hook 实现的"加测试 = 改 5 处" friction。

**反论点（被拒绝的替代方案）**：
- "全局 atomic counter"——拒绝理由：跨并发 IPC command 互相污染，无法分 IPC command 维度统计。
- "每 provider impl 显式调 record hook"——拒绝理由：每个 fake provider 测试要同步实现一套 hook 调用，加新 fs op 时 N 处改动。

## D-Impl-3: chunk-building teammate 嵌入的 5 步状态机（移自主 spec）

主循环维护 pending teammate 缓冲，5 处 flush 触发点（普通 user / `<local-command-stdout>` / Compact 边界 / Slash user / Interruption marker）共享 empty-AI 回收逻辑。Interruption 分支调 flush 触发 empty-AI 后再 append interruption 语义步骤。

**为什么保留在 design 而非 spec**：5 步状态机是构造层算法实现路径，spec 只规约"teammate 不产 UserChunk / 嵌入到下一 AIChunk / 末尾 trailing teammate 挂最后 AIChunk / 无 AI 时静默丢弃 / 5 处 empty-AI 边界场景的 chunk 列表形态"等可观察行为；未来 builder 重构（如改成事件流而非 buffer 模式）不需要走主 spec delta。

**理由**：状态机是实现细节但有不变量价值——empty-AI 的 chunk_id base 取首条 pending teammate uuid、metrics zero、pending slash 通过 take 消费——让未来 reviewer 改 builder 时知道"empty-AI" 不是异常路径。

**反论点（被拒绝的替代方案）**：
- "teammate user 消息直接产 UserChunk 但加视觉标记区分"——拒绝理由：与既有"teammate 嵌入 AIChunk.teammate_messages"的渲染契约冲突，UI 层逻辑要重写。
- "把 5 处 flush 逻辑展开成 5 条独立 Requirement"——拒绝理由：Scenario 已覆盖 5 处边界，5 条独立 Requirement 反而把同一状态机契约拆碎。

## D-Impl-4: http-data-api 完整路由表（移自主 spec）

主 spec 仅声明"`/api` 镜像 IPC + camelCase 一致 + lookup 失败按 code → status 表返"，**完整路由表**作为本附录列出，让重构期路由名变化（如未来 RESTful 重命名）不需要走主 spec delta。

**为什么保留在 design 而非 spec**：method + URL 的具体形态是实现路径选择，spec 只规约"行为类别（列项目 / 取详情 / 批量 / 搜索 / 辅助 / 通知 / lazy 镜像）+ 与 IPC 等价方法同形 + 错误 code 映射"；任何 RESTful 重构（典型把 `/api/sessions/batch` 改成 `/api/sessions:batch`）不应触发主 spec delta，否则 spec 沦为"路由说明书"。

**完整路由清单（实现真相源；reviewer / 实现层维护，与 IPC 等价方法 1:1 对应）**：

**项目 / 会话域**：

- `GET /api/projects` — 列所有项目
- `GET /api/projects/{projectId}/sessions` — 列指定项目下的会话（query `pageSize` / `cursor`），返回骨架 `PaginatedResponse<SessionSummary>` + SSE 异步 patch
- `POST /api/projects/{projectId}/session-summaries/batch` — 按 id 列表批量取会话 summary（body 为 string 数组）
- `GET /api/sessions/{sessionId}` — 取会话详情（仅 session_id，handler 内部反查 project_id）
- `POST /api/sessions/batch` — 按 id 列表批量取会话详情（body 为 string 数组，混合存在性返 200 + per-item status）

**搜索域**：

- `POST /api/search` — 会话搜索（body 含 query / 可选 projectId / 可选 sessionId）

**辅助域**：

- `GET /api/repository-groups` — 列仓库分组（聚合多 worktree 的 project）
- `GET /api/worktrees/{groupId}/sessions` — 列指定 repository group 下所有 worktree 的会话
- `POST /api/validate/path` — 校验文件系统路径
- `GET /api/claude-md?project_root=...` — 读取 global / project / project-local CLAUDE.md
- `POST /api/mentioned-file` — 读取 `@<path>` 提及的文件内容
- `GET /api/agent-configs?project_root=...` — 读取 agent config 文件清单
- `GET /api/contexts` / `POST /api/contexts/switch` / `POST /api/ssh/connect` / `POST /api/ssh/disconnect` / `GET /api/ssh/resolve-host` — context / SSH 管理

**配置 / 通知域**：

- `GET /api/config` / `PATCH /api/config`
- `GET /api/notifications?limit=N&offset=M` / `POST /api/notifications/{id}/read` / `DELETE /api/notifications/{id}` / `POST /api/notifications/mark-all-read` / `POST /api/notifications/clear`

**lazy 镜像（IPC 侧懒加载的 HTTP 直接命中端点；HTTP 路径 SHALL **不**应用 omit 裁剪）**：

- `GET /api/projects/{projectId}/memory` — 镜像 get_project_memory
- `POST /api/projects/{projectId}/memory-files` — 镜像 read_memory_file
- `GET /api/sessions/{rootSessionId}/subagents/{subagentSessionId}/trace` — 镜像 get_subagent_trace
- `GET /api/sessions/{rootSessionId}/subagents/{sessionId}/blocks/{blockId}/image` — 镜像 get_image_asset（Tauri-only `asset://` URL SHALL 转 `data:` URI）
- `GET /api/sessions/{rootSessionId}/subagents/{sessionId}/tools/{toolUseId}/output` — 镜像 get_tool_output（保留 outputBytes / outputOmitted）
- `POST /api/notifications/triggers` / `DELETE /api/notifications/triggers/{triggerId}` — trigger CRUD（caller SHALL 提供非空 id）
- `POST /api/projects/{projectId}/sessions/{sessionId}/pin` / DELETE 同 / hide / DELETE 同
- `GET /api/projects/{projectId}/session-prefs` — 镜像 get_project_session_prefs

**反论点（被拒绝的替代方案）**：
- "完整路由表保留在主 spec"——拒绝理由：method + URL 是 14 行清单，每次 RESTful 调整都触发主 spec delta，让 spec 沦为路由说明书。
- "完全删除路由表（仅保 IPC 等价方法引用）"——拒绝理由：reviewer / OpenAPI 生成 / 第三方客户端实现都需要权威路由清单源；放 design 让"行为契约 (主 spec) vs 实现真相源 (design)"分层清晰。

## D-Impl-5: keyboard-shortcuts `normalizeBindingToMod` token-level 算法

11 步实现细节 fold 为"幂等 / 跨平台一致 / 保留辅助修饰键 / token-level 不重排"四条契约后，token-level 算法步骤（split / detect mod / replace meta-or-ctrl / preserve order）作为 design 决策审计保留。

**为什么保留在 design 而非 spec**：spec 已规约 4 条可观察行为契约 + 关键边界 Scenario（meta / ctrl / 辅助修饰键 / 异常 meta+mod 共存 / 双修饰键 mac record 产物），未来若加新主修饰键（如 hyper key）只需扩展 detect 阶段，本 design 留下的算法骨架是这次扩展的接续点；spec 不应绑死 token 序列处理顺序。

**反论点（被拒绝的替代方案）**：
- "保留 11 步算法在主 spec"——拒绝理由：未来加 hyper key / 改 modifier 优先级都触发 spec delta，等于把"实现选择"绑成契约。
- "完全去除算法骨架（spec 仅留 4 条契约）"——拒绝理由：未来 reviewer / 加新 modifier 时找不到扩展点；design 留个算法骨架是"实现真相源"的合理位置。

## D-Impl-6: frontend-context-menu 三层 handler 分流（移自主 spec）

Layer 1（surface 右键 action）→ Layer 2（window selection handler）→ Layer 3（global fallback 初始化入口）的注册顺序 + 触发优先级。Layer 2 SHALL 在 Layer 3 之前注册以保证 selection 菜单优先级；HMR 重复调用通过 window sentinel flag 守护幂等。

**为什么保留在 design 而非 spec**：spec 已规约"surface 优先 + selection 居中 + global 兜底" 三层行为契约 + Scenario 覆盖 6 个组合场景；本 design 记录的注册顺序 / sentinel 命名 / HMR 幂等机制是实现路径选择，未来若框架换（典型从 Svelte 迁到别的）这些 sentinel 命名不应触发主 spec delta。

**反论点（被拒绝的替代方案）**：
- "三层注册顺序写进主 spec"——拒绝理由：注册时机（mount 之前 / 之后 / 与 Layer 1 同步）是框架挂载顺序细节，应随框架演进而非走 spec delta。
- "全局 mutex 守护 HMR 幂等"——拒绝理由：HMR 仅开发期场景，sentinel flag 足够；mutex 引入运行时开销 + 死锁风险。

## 风险与权衡

- **风险 1：误删 SHALL/MUST 行为句** → 缓解：codex design 二审强制（8 cap 跨域命中"跨 ≥ 2 个 capability"判据）+ apply 阶段每 cap 单独 commit + reviewer 逐 commit 检视。
- **风险 2：测试存在性 tasks 落空** → 缓解：tasks.md 的"补 IPC contract test"项 SHALL 实际跑 grep 确认（archive 时 spec-fidelity-reviewer 抓覆盖）。
- **风险 3：spec-purity 反模式分数恶化（边界 case）** → 缓解：archive 后 `change spec-purity-mechanism` 的 ratchet 测试（`scripts/check-spec-purity.sh`）作回归 gate。
- **取舍**：把"实现选择 SHALL"全删 vs 全移 design——选**全移**，决策审计不丢。
