## Context

issue #303 9-PR 计划批次 B 合并清理两个 cap：

- `configuration-management`（10 hits, baseline `spec/configuration-management 10`）：p1=3 / p2=1 / p6=6，集中在 5 个 Requirement body（更新 triggers / 自动检查更新 / 跳过版本 / pinned 迁移 / HTTP server lifecycle）+ 1 个 Scenario WHEN（mention `@src/foo.ts` 示例）
- `frontend-context-menu`（6 hits, baseline `spec/frontend-context-menu 6`）：全 p4 metric，分布在「SessionContextMenu / TabContextMenu 重构兼容」与「AppContextMenu submenu 渲染」的 body / Scenario 标题 / Scenario 内 ms 数字

姊妹工艺已在两个 archive 落地：
- `2026-05-25-spec-cleanup-frontend-test-pyramid`（PR #309，单 cap 重写 7 Requirement）
- `2026-05-25-ssh-remote-context-cleanup`（PR #312，单 cap 重写 14 Requirement，apply 阶段 codex 二审反转过粗抽象 → D-1b 数字三分表格）

本 change 工艺直接复用，是同批姊妹工艺第三次实战，特点是**两 cap 合并 1 PR**——共享 design 决策 + 同 commit 双 cap baseline 刷新。

## Goals / Non-Goals

**Goals:**

- `configuration-management` 10 hits 降至 0（顺手把 HTTP server lockstep Requirement 也清了，配置文件 JSON 字段名 + 协议常量名都用裸字符串保留不算命中）
- `frontend-context-menu` 6 hits 降至 3（保留 3 处可断言用户感知阈值数字作 NFR 契约——按 D-2b 三分）
- 行为契约语义 100% 保持不变——Requirement / Scenario 的 SHALL / MUST 句语义对等，外部可观察行为不改
- 移除的实现细节作为"参考实现指引"记录在本 design.md，方便后续 reviewer / 维护者溯源到当前实现

**Non-Goals:**

- 不改代码 / 测试 / 配置
- 不改 Requirement / Scenario 数量级（允许等价改写但不允许丢语义）
- 不改其它 capability spec
- 不改 Purpose 段（已经简洁，无反模式）
- 不动 `pinned_sessions` / `hidden_sessions` / `httpServer.port` 等配置文件 JSON 字段名（数据契约 + 跨版本迁移可观察）

## Decisions

### D-1：行为契约 100% 不变

**问题**：10 + 6 = 16 hits 中部分句子表面像反例（如「`复制反馈"已复制!"600ms 关闭`」）实则承载用户可感知的契约。

**决策**：所有 SHALL / MUST 句的**语义**完全对等迁移：

- Rust 内部类型签名（`ConfigData` / `Vec<NotificationTrigger>` / `HashMap<String, Vec<PinnedSession>>` / `Box<dyn ...>`）→ 概念描述（"应用配置" / "通知触发器数组" / "pinned 数组" / "异步流式句柄"）
- 具名内部 fn 路径（`ConfigManager::update_notifications` / `ConfigManager::load` / `TriggerManager::set_triggers` / `validate_trigger` / `save()`）→ 行为描述（"更新 notifications 段" / "配置加载" / "同步给运行时 trigger 调度器" / "trigger 校验" / "持久化到磁盘"）
- `tracing::warn!(target: ..., key = %k, ...)` → "在日志中以 warn 级别附带键名记录"——保留**可观察事件**（warn 级别 + 含哪些字段），具体 log crate / target 字符串属诊断手段
- `#[serde(default = "<fn>")]` / `#[serde(default, skip_serializing_if = "Option::is_none")]` → "缺省值为 X，配置文件缺该字段时反序列化为默认值；持久化时 null 值省略键以保持文件简洁"——保留**用户可观察的 schema 演进契约**（缺字段不报错 / null 不写入）
- 协议常量 / 数据契约（IPC camelCase 字段名 `autoUpdateCheckEnabled` / `skippedUpdateVersion` / `keyboardShortcuts` / `httpServer.port` / `pinned_sessions` 等配置文件键名）→ **保留**，外部协议契约
- RFC2119 关键词（SHALL / MUST / SHOULD / MAY）保留英文

**理由**：SPEC_GUIDE 反例对照表明确「内部 fn / type / mod / struct field 名 → design」+「IPC payload 字段名（camelCase）/ 错误码 / 错误 variant 名 → spec」。本 change 严格按表分流。

### D-2：configuration-management 反例分类处理

按 10 hits 各类分别给出处理规则，apply 阶段照表批改：

| 类 | 数 | 处理方式 |
|---|---|---|
| **p1 内部模块/类/函数名** | 3 | `ConfigManager::update_notifications` / `ConfigManager::load` → 行为描述（"更新 notifications 段" / "配置加载"）；`TriggerManager::set_triggers` → "同步给运行时 trigger 调度器"；`Vec<NotificationTrigger>` / `HashMap<String, Vec<PinnedSession>>` → "通知触发器数组" / "pinned 数组按 project_id 分桶" |
| **p2 源文件路径** | 1 | Scenario WHEN `mention @src/foo.ts` 示例 → `mention @docs/note.md`（避开 `\.ts` + `/src/` 词法命中，保留 mention 解析示例的可读性） |
| **p6 库框架名** | 6 | `tracing::warn!(...)` → "在日志中以 warn 级别记录"；`#[serde(default = "<fn>")]` → "缺省值为 X"；`#[serde(default, skip_serializing_if = "Option::is_none")]` → "缺省值为 null，持久化时 null 值省略键"；`HttpServerConfig.enabled: bool` / `HttpServerConfig.port: u16` Rust 字段定义 → 直接说 JSON 字段名 `httpServer.enabled` / `httpServer.port` |

### D-2b：frontend-context-menu 数字三分（用户感知 vs body 重复 vs 标题命名）

**触发**：6 个 ms 数字命中按 SPEC_GUIDE 反例 4 + ssh-remote-context-cleanup D-1b 工艺审查后，结论是**全部承载用户感知契约**——不能整体抽象掉。但同一阈值在「Requirement body 概要」+「Scenario 标题」+「Scenario WHEN/THEN 可断言行」三处可能重复——只在 body / 标题处抽象，Scenario 内**保留具体数值**作可测契约。

| 数字 | 出现位置 | 类别 | 处置 |
|---|---|---|---|
| 600ms（"已复制!"反馈关闭）| Requirement body 概要描述 | body 重复（Scenario 内已有） | body 删去具体数字，描述简化为"短反馈后关闭"，**Scenario 内保留 600ms 作可断言契约** |
| 600ms（"已复制!"反馈关闭）| Scenario `Sidebar 会话项右键菜单回归` AND | **用户感知**（toast 时长契约） | **保留** |
| 200ms（hover 触发 submenu）| Requirement body 概要描述 | body 描述（Scenario 内已有） | body 抽象为"hover 短延迟弹出（具体阈值见 Scenario）" |
| 200ms（hover 触发 submenu）| Scenario 标题 `hover 200ms 打开 submenu` | Scenario 标题（命名视角）| 标题改为 `hover 短延迟打开 submenu`（SPEC_GUIDE: Scenario 标题用用户视角短语），**Scenario WHEN 内保留 200ms** |
| 200ms（hover 触发 submenu）| Scenario WHEN `用户鼠标 hover ... 持续 200ms` | **用户感知**（hover 触发阈值契约） | **保留** |
| 200ms（"无 200ms 延迟"对照键盘 vs 鼠标）| Scenario THEN `submenu SHALL 立即弹出（无 200ms 延迟）` | **用户感知**（键盘 vs 鼠标行为对照契约）| **保留** |

**结果**：6 hits → 3 hits（保留的 3 处都是 Scenario WHEN/THEN 内可断言契约）。

**理由**：toast 时长 600ms（用户能看见反馈）+ submenu hover 200ms（用户主观能区分"瞬间 vs 略延迟"）+ 键盘即时 vs 鼠标 200ms 对照（用户能感知键盘比鼠标快），三者都是 SPEC_GUIDE 反例 4 表述的"用户能感知阈值（用户等多久 / 卡多久 / 恢复多久）→ 保留数值"。把这些抽象掉 = NFR 失去守护能力，未来后端把 600ms 改 50ms 用户根本看不见 toast、改 5s 又太久——契约失守。

### D-3：5 秒启动延迟移 design

`持久化「启动时自动检查更新」开关` 原 body 含「该字段 SHALL 控制应用启动后 5 秒后台自动检查更新行为」。**5 秒延迟**按 SPEC_GUIDE 反例 4 三分：

- 启动后 5 秒后台 spawn 检查更新——**用户不感知**（无 UI 等待 / 无可见 loading），是为了不和启动其它任务并发争 CPU 的实现调优旋钮
- 改为 0.1 秒 / 30 秒，用户在 SettingsView 看到的更新提示 banner 出现时机几乎无感（启动后用户不会立刻去 settings）
- 因此 5 秒属"实现层调优数字"，移 design.md 参考实现指引段

spec body 改为 "应用启动后台自动检查更新行为（详 [[app-auto-update]]）"——把"启动时机"具体规约 owner 交给 [[app-auto-update]]，配置 spec 只承诺开关字段。

### D-4：configuration JSON 字段名保留例外

**问题**：D-2 表说 "Rust 类型签名抽象掉"，但 `pinned_sessions` / `hidden_sessions` / `httpServer.port` 等是配置文件 JSON 字段名（snake_case 或 camelCase），属用户可观察的数据契约——升级版本时跨版本迁移行为依赖这些字段名稳定。

**决策**：保留以下名称作为**协议常量**，spec 直接引用：
- 配置文件根字段：`general.claudeRootPath` / `ssh.profiles[]` / `ssh.last_connection` / `ssh.auto_reconnect` / `httpServer.enabled` / `httpServer.port` / `keyboardShortcuts` / `pinned_sessions` / `hidden_sessions` / `notifications.triggers`
- IPC payload camelCase：`autoUpdateCheckEnabled` / `skippedUpdateVersion` / `fontSans` / `fontMono` / `timeFormat` / `externalEditor` / `searchEngine` / `terminalApp`
- composite key 分隔符 `"::"` 与 fold 后 base_dir 形态——pinned_sessions 跨版本迁移行为依赖具体格式
- 备份文件后缀 `.bak.<unix_timestamp_ms>` / `.pre-merge-composite.bak`——人工 / 测试 / 调试时可见
- 错误 variant：`ApiError::ValidationError`（外部协议契约）

**实施**：grep `ConfigData::xxx` / `ConfigManager::xxx` / `Vec<NotificationTrigger>` / `HashMap<String, _>` / `PinnedSession` / `HiddenSession` 这些 Rust 内部类型 / fn 路径全部改为概念描述；上面列的 JSON 字段名 / 协议常量裸写保留。

### D-5：Scenario 命名扫描

逐 Scenario 看命名是否符合 SPEC_GUIDE.md::Scenario 命名视角（"用户/系统外部可观察事件视角"）。两 cap 当前 Scenario 标题里**只 1 处** D-2b 已识别需要改：

| 当前命名 | 改为 | 理由 |
|---|---|---|
| `Scenario: hover 200ms 打开 submenu` | `Scenario: hover 短延迟打开 submenu` | 标题用用户视角短语，具体 200ms 留 Scenario WHEN |

其它 Scenario 标题已是用户/系统视角（如 `First launch with no config file` / `triggers 字段被整体替换并落盘` / `外点关闭` 等），不动。

## Risks / Trade-offs

- **[语义漂移]** 16 hits 批量改写有可能漏掉某句的隐含行为约束 → Mitigation：每个 Requirement 独立 commit；apply 完成后跑 spec-guide-reviewer 自审；codex 二审重点查行为对等（特别是 D-2b 数字保留是否守住）
- **[残留命中]** D-1 决策保留 IPC camelCase 字段名 + 配置文件 JSON 键名 + 错误 variant 名，可能剩 1-3 处灰色命中 → Mitigation：接受 `configuration-management` 目标 ≤ 1，超出由 lead 在 baseline 上 approve
- **[阅读成本]** 移除具体 5 秒延迟 / 内部 fn 路径后，新 contributor 看 spec 难判断"当前实现长啥样" → Mitigation：本 design.md 作为参考实现指引留档；contributor 找具体值时去 design.md 而非 spec.md
- **[archive 顺序]** 本 PR 同时改 `configuration-management` + `frontend-context-menu` 两 cap，与同期其它清理 PR 不撞 cap，可独立 archive 不撞顺序坑

## 参考实现指引（从 spec 移出的实现细节）

以下为当前实现对应关系，供维护者参考。**本节内容不属行为契约**，纯文档备忘——重构这些实现不需要改 spec。

### configuration-management

#### Rust 类型与抽象层映射

| spec 抽象 | 当前实现 |
|---|---|
| 应用配置（顶层数据结构）| `cdt_config::ConfigData` struct |
| 配置管理器（加载 / 持久化 / 更新入口）| `cdt_config::ConfigManager` |
| 通知触发器数组 | `Vec<NotificationTrigger>` |
| trigger 校验函数 | `cdt_config::validate_trigger` |
| 运行时 trigger 调度器 | `cdt_config::TriggerManager::set_triggers` |
| 持久化到磁盘 | `ConfigManager::save()` |
| pinned 数组按 project_id 分桶 | `SessionsConfig.pinned_sessions: HashMap<String, Vec<PinnedSession>>` |
| hidden 数组（同形态）| `SessionsConfig.hidden_sessions: HashMap<String, Vec<HiddenSession>>` |
| 配置加载入口 | `ConfigManager::load` |
| 在日志中以 warn 级别记录 | `tracing::warn!(target: ..., key = %k, ...)` |
| 缺省值反序列化默认机制 | `#[serde(default = "<fn>")]` / `#[serde(default, skip_serializing_if = "Option::is_none")]` 注解 |
| HTTP server 配置 section | `cdt_config::HttpServerConfig { enabled: bool, port: u16 }` |

#### 具体数值（实现层调优）

| spec 抽象 | 当前数值 |
|---|---|
| 应用启动后台检查更新延迟 | 5 秒（避免启动期与其它任务争 CPU） |

### frontend-context-menu

#### 数字（保留作 spec NFR 的用户感知阈值，不在此节）

按 D-2b：toast 600ms / hover 200ms / 键盘即时 vs 鼠标 200ms 三处都留在 spec Scenario，不进 design——因为属于用户可感知契约。

本节仅记录与本 cleanup change 无关的次级实现细节（无）。
