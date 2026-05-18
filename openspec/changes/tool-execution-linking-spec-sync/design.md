# Design

## D1：补 spec 而不是删 / 改实现

**问题**：三条 followups 都是"实现先行 + spec 没写全"。一种处理是反过来"按 spec 简化的版本删掉实现里多余分支"（让实现追 spec），另一种是"按实现把 spec 补全"（让 spec 追实现）。

**决策**：选**让 spec 追实现**——补 5 个 scenario 把当前 Rust 行为冻结进 spec。

**理由**：
- 实现里多出来的分支（`Duplicate tool_use ids`、`SendMessage shutdown_response` / `broadcast` / 无 recipient）都是**真实场景**：
  - `Duplicate tool_use ids` 是 Claude Code 流式 rewrite + retry 时偶发的真实重复（pair.rs:36-43 注释里的 impl-bug fix 路径）；删掉会让重复 tool_use 进入 pending map 同时存在两份，后续 result 误配第二份，并失去 metrics 计数。
  - `SendMessage` 的 4 branch 来自原版 TeamCreate 协议——`shutdown_response` 是 teammate 关闭流程协议消息（不是普通信息发送，需要简洁特殊文案）；`broadcast` 是无 recipient 的群发；删掉会让这三类工具调用 summary 退化成 `"SendMessage"` 字面量，UI 上无法区分。
- spec 的角色是"把已有真实行为冻结成回归契约"。这些行为已被 Rust 单测覆盖（`pair.rs::duplicate_*` / `summary.rs::send_message_*`），spec 写齐之后这些单测就成了 scenario 的执行映射，未来无意改动违反时能被拦下。
- 反向选择会引入回归（删掉真实分支）+ 丢失功能（UI summary 失真）+ 删测试，代价远高于补 5 行 markdown。

## D2：SendMessage `Other status values` 与 `default + recipient + body` 是否合并

**问题**：`format_send_message` 的 default branch 内部还分两个子 branch（含 recipient / 不含 recipient）。是把"含 recipient"作为单独 scenario 加保留 D1 中的 4 个新 scenario，还是把"不含 recipient"塞进现有 `SendMessage with recipient and body` scenario 改写成 "with or without recipient"？

**决策**：保留现有 `SendMessage with recipient and body`（**不**改写）+ 新加 `SendMessage default type without recipient` 作为独立 scenario。

**理由**：
- 现有 scenario 的 SHALL 句"summary SHALL 同时含 recipient 与截断后的 message 预览"在 `to` 字段缺失时**不成立**（Rust 实现退化为 `truncate(msg_type, 50)`，不含 message body）。如果改写为 "with or without"，SHALL 就要被弱化成"含 recipient 时 ... 不含 recipient 时 ..."这种条件分支描述，对 spec 强约束性是损伤。
- 拆成两个 scenario 让每个 SHALL 都是"无条件成立"句，回归测试映射也更清晰：单测 `send_message_to_recipient` ↔ scenario A；潜在新测 `send_message_default_no_recipient` ↔ scenario B（实测当前 4 个 send_message 单测含 `_shutdown_approved` / `_broadcast` / `_to_recipient` 三个，**确实**没有"default 无 recipient"单测——这是一个 followups 之外的次要 coverage-gap，**本 change 仅补 spec scenario，把"建议补单测"留作后续 implicit 的 coverage 提示**，不在本 change 改代码）。

## D3：Duplicate tool_use ids scenario 的 SHALL 描述粒度

**问题**：现有 `Duplicate result ids` scenario 的 THEN 句把 `duplicates_dropped += 1` 与 `tracing::warn! 上报 id` 都写进契约。新加的 `Duplicate tool_use ids` 是否同样冻结这两个具体行为？

**决策**：**同等粒度冻结**——明文要求 `keep first + duplicates_dropped += 1 + tracing::warn! 上报 tool_use_id`，与现有 result 侧 scenario 保持一致。

**理由**：
- `duplicates_dropped` 是 `ToolLinkingResult` 的 IPC 可见字段，是 metrics 派生的真相源（前端可基于此报警 / 聚合）。仅写"keep first"会留下"是不是要计数"的歧义。
- `tracing::warn!` 是 dev 调试 / 用户反 issue 复现时的关键线索——历史上 PR #38 race 条件就是靠 warn 行抓到的。spec 不写就有可能在未来重构时被无意去掉。
- 两条 scenario 措辞对称便于对照阅读 + 减少 reviewer 认知成本。

## D4：followups 标 ✅ 还是直接删除三条

**问题**：第三条（三阶段 fallback）spec 已经写齐，followups 这条已经过期。是把它**删除**还是**标 ✅ 已修**？

**决策**：**全部标 ✅ 已修**（不删），并在条目末尾加一行"**Rust 实现** + 引用 change name + 引用 spec scenario 名"，与 followups 文件中其它已修条目（如 `[impl-bug?] requestId 去重函数` 标 ✅ 后追加 Rust 实现说明）保持一致格式。

**理由**：
- followups.md 的角色是"baseline cross-check 留痕"——标 ✅ 后的条目不是垃圾，而是"我们意识到了这个 gap、并已经修了，下面是落地路径"的审计追溯。这种留痕在 Rust port 后期定位"为什么这里这么写"时是关键线索。
- 与已有同文件行业惯例一致（`session-parsing` / `notification-triggers` 已修条目都是 ✅ + Rust 实现说明）。
- 删除会丢失"曾经是 baseline gap"的信息，未来 reviewer 看到 spec 里这些 scenario 不知道是后补的、与原版 TS 行为是否一致。

## D5：spec delta 用 MODIFIED 而非 ADDED

**问题**：补 scenario 到已有 Requirement 内是 MODIFIED 还是 ADDED？

**决策**：**MODIFIED**——OpenSpec 中 `MODIFIED Requirement` 完整 body 替换主 spec 对应 Requirement，包括所有 scenario。所以新增 scenario 通过 MODIFIED 写整段 Requirement（含原有 scenario + 新加 scenario）来表达。

**理由**：
- OpenSpec convention（见 `openspec/CLAUDE.md::archive 顺序坑`）：MODIFIED 用 delta 整段替换主 spec，不做三方合并。
- ADDED Requirement 是为"全新 Requirement"用，本 change 没有新加 Requirement，只是给已有 Requirement 补 scenario。
- 写法风险：MODIFIED 必须**保留**原有所有 scenario（不能漏），否则 archive 会丢失。本 change 在 spec delta 中明文写出每条原有 scenario + 新加 scenario，靠 reviewer 与 spec validate 双重防漏。

## D6：是否同时补"建议加单测"到 tasks.md

**问题**：D2 提到 "default 无 recipient" branch 当前 Rust **没有单测**，是 followups 之外的 coverage-gap。本 change 是否顺带把"加该单测"列进 tasks.md？

**决策**：**不加**——本 change 严格限定"纯 spec 同步无代码改动"，加单测属于 Rust 代码改动，会扩大本 change 的 review 半径并触发额外 CI 节拍。把"补该单测"作为 followups 一条新 [coverage-gap] 留给后续 change 处理。

**理由**：
- 用户初始 ask 明确"纯 spec 同步无代码改动，开一个 change 一并归档"。
- 加单测会让 PR 描述从"只动 spec markdown"变成"动 spec + 加 Rust 测试"，CI 会跑 cargo test，codex 二审会扩展到测试代码评估，节拍变长。
- followups.md 的设计就是"次要 gap 留痕、按需开新 change"——这条非常符合该模式。
