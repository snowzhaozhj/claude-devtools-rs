## Why

`configuration-management` 主 spec 有 10 处 spec-purity 反模式命中（p1=3 / p2=1 / p6=6），主要是 Rust 类型签名（`ConfigData` / `Vec<NotificationTrigger>` / `HashMap<String, _>` / `tracing::warn!`）、`#[serde(default ...)]` 注解、内部 fn 路径（`ConfigManager::update_notifications` / `ConfigManager::load`）。`frontend-context-menu` 主 spec 有 6 处命中（全 p4 metric），分布在「SessionContextMenu / TabContextMenu 重构兼容」与「AppContextMenu submenu 渲染」两个 Requirement 的 body / Scenario 标题 / Scenario 内 ms 数字。

按 SPEC_GUIDE.md::反例对照表 + 反例 4，前者属典型实现细节落进 spec，应迁 design；后者部分数字承载用户感知阈值（toast 显示时长 / hover 触发延迟）SHALL 留 spec NFR，仅 body 重复描述与 Scenario 标题里的数字抽象。作为 issue #303 9-PR 计划批次 B 合并 1 PR 清理。

## What Changes

- `configuration-management`：6 个 Requirement MODIFIED 重写——「Resolve and read mentioned files safely」/「Update notifications SHALL accept full triggers replacement」/「持久化「启动时自动检查更新」开关」/「持久化跳过的更新版本号」/「Migrate composite project IDs in pinned sessions on load」/「HTTP server enabled / port SHALL be persisted in lockstep with lifecycle」，移除 fn 路径 / 类型签名 / serde 注解 / `tracing::` 具名引用 / 含 `.ts` 的源码扩展示例
- `frontend-context-menu`：2 个 Requirement MODIFIED 重写——「SessionContextMenu / TabContextMenu 重构兼容」/「AppContextMenu submenu 渲染」，仅抽象 Requirement body 与 Scenario 标题里的 ms 数字（用语义化"短延迟"替换），Scenario WHEN/THEN 内承载用户感知契约的 200ms / 600ms 数字一律保留
- 实现细节（5 秒启动延迟 / 内部 fn 路径 / Rust 类型签名 / serde attribute）移入 design.md 参考实现指引段
- 同步刷新 `scripts/spec-purity-baseline.txt` 两个 cap 计数

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `configuration-management`：6 个 Requirement MODIFIED 全文重写——行为语义不变，去实现细节词法命中
- `frontend-context-menu`：2 个 Requirement MODIFIED 全文重写——行为语义不变；按 SPEC_GUIDE 反例 4 三分，可观察阈值数字保留作可测契约

## Impact

- 仅影响 `openspec/specs/configuration-management/spec.md` + `openspec/specs/frontend-context-menu/spec.md` 文档内容
- 不改代码 / 测试 / 配置
- `scripts/spec-purity-baseline.txt` 两个 cap 计数下降（configuration-management 10 → 0；frontend-context-menu 6 → 3，3 处保留为可断言用户感知阈值）
