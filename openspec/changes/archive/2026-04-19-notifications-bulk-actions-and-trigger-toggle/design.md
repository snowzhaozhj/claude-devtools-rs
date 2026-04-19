## Context

**当前状态（代码引用 @ commit `0c06602`）：**

- `crates/cdt-config/src/manager.rs::ConfigManager::update_notifications`（line 173-209）是 section 级更新入口，match 只识别 4 个 key，其它 key 命中 `_ => {}` 直接吞掉。前端通过 `update_config("notifications", { triggers: [...] })` 批量写 triggers 数组时这条路完全无效。
- `crates/cdt-config/src/trigger.rs::TriggerManager` 另外提供 `add / update / remove / set_triggers` 独立 API，在 `ConfigManager::add_trigger / remove_trigger` 里调用（manager.rs line 344-357），这两个走独立 Tauri command `add_trigger / remove_trigger`。**单独删一个 trigger**走这条路径是正确的，但**批量替换 triggers（含 toggle enabled 状态）**走 `update_config` 路径就失效。
- 原版 TS `ConfigManager.updateConfig` line 501-514：object-spread 整段替换 section 数据，然后 `saveConfig()`；line 946/960 在敏感路径（reload、merge）调 `triggerManager.setTriggers(this.config.notifications.triggers)` 保持 TriggerManager 内存与 config 同步。
- `NotificationManager`（crates/cdt-config/src/notification_manager.rs）有 `clear_all / mark_all_as_read`，但**没有 `delete_one`**，也没有任何 IPC 暴露。
- 前端 `NotificationsView.svelte` 只有单条 ✓ 按钮，无批量操作、无删除。`SettingsView.svelte` 的 4 处开关用自制 `.toggle-btn`（文字 "开/关"），视觉与原版 Linear 滑块差距明显。

**原版参考：**

- `../claude-devtools/src/renderer/components/notifications/NotificationsView.tsx`（完整批量 UX，含 filter chips + 二次确认清空）；
- `../claude-devtools/src/renderer/components/notifications/NotificationRow.tsx`（行 hover 出 Archive/Delete 小图标）；
- `../claude-devtools/src/renderer/components/settings/components/SettingsToggle.tsx`（Linear 滑块，9×5 track + 16px 白 thumb + `#6366f1` 启用色）；
- `../claude-devtools/src/main/services/infrastructure/ConfigManager.ts` line 501-514 + 946/960（updateConfig 行为 + TriggerManager 内存同步契约）。

## Goals / Non-Goals

**Goals:**
- 消除 `update_notifications` 对 `triggers` 字段的静默丢弃；该路径写入后 `config.notifications.triggers` 与 `TriggerManager` 内存一致，下次 `get_enabled_triggers()` 立即反映最新启用集。
- 通知面板对齐原版核心操作：全部已读、清空（二次确认）、单条删除。
- Settings 中所有布尔开关（通用 + 通知 + 每个 trigger）统一采用 Linear 风格滑块，消除"看不出开关状态"。
- 不回退现有 `add_trigger / remove_trigger` 独立 CRUD 路径——两条路径都需要可用，因为前端新建/删除 trigger 的 UX 仍然想要"最小字段"乐观更新，不想序列化整条列表。

**Non-Goals:**
- 不实装原版 filter chips（按 triggerName 过滤）。`clear_notifications` IPC 签名预留 `trigger_id: Option<String>` 参数与后续 filter chips 对齐，但本轮前端只调用 `None` 即 "清空全部"。
- 不引入 `notification-added` 事件的新推送逻辑；已有的 `subscribe_detected_errors` 广播不变。
- 不修改通知的持久化格式、100 条上限、确定性 id 规则。
- 不做原版 `TriggerCard` 展开的完整编辑体验（正则匹配、ignore patterns、repository scope 等高级字段）——Rust port 现有"新建表单只暴露最小字段"保持不动。
- 不实装桌面通知重放 / 未读恢复逻辑。
- 不修改 HTTP server 行为以外的同步路径（`list_sessions_sync` 等）。

## Decisions

### D1: `update_notifications` 对 `triggers` 采用整体替换 + TriggerManager 同步

新增 match 分支：

```rust
"triggers" => {
    let list: Vec<NotificationTrigger> = serde_json::from_value(v.clone())
        .map_err(|e| ConfigError::validation(format!("triggers must be an array of NotificationTrigger: {e}")))?;
    // 每条都校验，任一非法立即 Err
    for t in &list {
        let r = validate_trigger(t);
        if !r.valid {
            return Err(ConfigError::validation(
                format!("Invalid trigger \"{}\": {}", t.id, r.errors.join(", "))));
        }
    }
    self.config.notifications.triggers = list.clone();
    self.trigger_manager.set_triggers(list);
}
```

未识别键加一条 `tracing::warn!(key = %k, "unknown notifications update key ignored")`，便于以后再漏同类 bug 时排查。

**为什么整体替换而不是逐条 merge**：
- TS 原版就是整体替换（object-spread），port 对齐；
- 前端 `handleToggleTrigger` 的语义就是"送整个新列表过去"，保持契约最简单；
- 单条操作（新增/删除）另有独立 IPC 路径，不走这里。

**为什么要额外 `set_triggers`**：
- `TriggerManager::triggers` 是 `ConfigManager` 持有的独立状态，`get_enabled_triggers` 从它读，不读 `self.config.notifications.triggers`。若只改 config 不同步 manager，下次读仍旧。TS 原版 line 946/960 已经踩过这个坑并显式同步，Rust port 需对齐。

**拒绝的替代方案**：
- "把 `TriggerManager::get_enabled` 改成直接读 `self.config.notifications.triggers`" —— 等于抹掉 TriggerManager 的独立性，得同时改 `add / update / remove` 的 impl，改动面大且破坏现有测试。
- "让 `update_config` 遇到 `triggers` 时丢 ApiError" —— 强迫前端必须走 `add_trigger / remove_trigger`。但这要前端每个 toggle 都算 delta 发 N 个 IPC，语义重；且 TS 原版就支持整体替换。

### D2: 新增 `NotificationManager::delete_one(id)`，而不是复用 `mark_as_read` + client-side 过滤

**签名**：
```rust
pub async fn delete_one(&mut self, notification_id: &str) -> Result<bool, ConfigError>
```
从 `self.notifications` 中 `retain(|n| n.error.id != id)`，若 len 变化即 `save` 并返回 `Ok(true)`，否则 `Ok(false)`。

**拒绝**：直接在前端过滤掉 "已读" 并不展示——这样通知持久化仍膨胀到上限，用户清空只是 UI 假象；后端数据无 source of truth。

### D3: `DataApi` trait 新增 3 个方法，trait 对象对所有实现都要求

```rust
async fn delete_notification(&self, notification_id: &str) -> Result<bool, ApiError>;
async fn mark_all_notifications_read(&self) -> Result<(), ApiError>;
async fn clear_notifications(&self, trigger_id: Option<&str>) -> Result<usize, ApiError>;
```

- `clear_notifications` 参数 `trigger_id: Option<&str>`：`None` 清全部，`Some("...")` 清指定 trigger 产生的通知——本轮前端只用 `None`，但签名和原版 `clearNotifications(triggerName?)` 对齐，后续 filter chips 实装直接用；
- 返回 `usize` = 被删条数，便于 HTTP / 测试验证；`LocalDataApi` 内部实现。

**拒绝**：trait 加 `default` impl 抛 `unimplemented!` —— crate rule 禁止；所有实现显式写。`HttpDataApi` / 测试 `StubDataApi` 若有都要实现。

### D4: Tauri commands 命名与 emit

- `delete_notification(id)` / `mark_all_notifications_read()` / `clear_notifications(trigger_id: Option<String>)` 三个 command。
- 每个命令结束后 `app.emit("notification-update", ())`，前端 TabBar + NotificationsView 已经 listen 这个事件并 reload，无需额外 event；
- 无需新 event。

### D5: 前端"清空"二次确认态在组件内 local state，3 秒自动取消

`let clearPending = $state(false);` + `setTimeout(() => clearPending = false, 3000)`。
- 第一次点："确认清空" 红色按钮；
- 第二次点内完成 confirm：调 `clear_notifications(undefined)`，然后 reload；
- 3 秒内无点击：自动复原，下次点又要重新确认。

**对齐原版**：`NotificationsView.tsx` line 143-152 的完全相同 UX。

### D6: `SettingsToggle.svelte` 组件 API

```svelte
<script lang="ts">
  interface Props {
    enabled: boolean;
    onChange: (v: boolean) => void;
    disabled?: boolean;
  }
  let { enabled, onChange, disabled = false }: Props = $props();
</script>

<button type="button" role="switch" aria-checked={enabled}
  class="toggle-switch" class:toggle-switch-on={enabled} class:toggle-switch-disabled={disabled}
  disabled={disabled} onclick={() => !disabled && onChange(!enabled)}>
  <span class="toggle-thumb" class:toggle-thumb-on={enabled}></span>
</button>
```

CSS 使用项目现有的 CSS 变量（`--color-surface-raised / --color-border / --color-text / --color-accent`）。紫色启用色 `#6366f1`（与原版完全一致）通过新 token `--color-switch-on` 注册在 `app.css` `:root` + `[data-theme="dark"]`，方便主题化。

### D7: trigger 启用 toggle 继续走 `update_config` 路径

前端 `handleToggleTrigger`：
1. 乐观更新 `config.notifications.triggers[i].enabled = !...`；
2. `updateConfig("notifications", { triggers: config.notifications.triggers })`；
3. **修完 D1 后这条路径真正落盘**；失败时 `getConfig()` 重新拉刷新。

拒绝改为 "toggle 走新增 `update_trigger` IPC"：
- 增 IPC 面大；
- 原版就是走 update_config 批量替换；
- 真正的 bug 在后端，不在前端路径。

## Risks / Trade-offs

- **[Risk] D1 整体替换若前端误传缺失 builtin trigger，会丢 builtin**。→ Mitigation：`update_notifications` 写入后不调 `merge_triggers(&list, &default_triggers())` 补齐——TS 原版也不补（builtin 只在 `load_from_disk` 的 `merge_with_defaults` 路径补）。若用户手工删了 builtin，下一次 app 启动 `load` 会补回来；本 change 不改此行为。测试 `update_notifications_missing_builtin_preserved_on_reload` 覆盖。
- **[Risk] `delete_one` + `mark_all` + `clear_all` 并发同时落盘**。→ Mitigation：`NotificationManager` 由 `Arc<Mutex<...>>` 保护，所有写操作串行；无额外锁粒度问题。
- **[Risk] 前端清空二次确认期间用户切到别的 tab，`setTimeout` 仍会跑**。→ Mitigation：Svelte `$effect` cleanup 里 `clearTimeout`；组件 destroy 时自然清理。
- **[Risk] Linear 紫色 `#6366f1` 与现有 Soft Charcoal 色板风格不完全一致**。→ Mitigation：接受——原版本身就是这个色；用户反馈"看得清"优先于色板纯粹。
- **[Trade-off] IPC 增 3 个**。→ 收益：标准通知面板批量管理体验；成本可接受，符合渐进式 IPC 扩展约定。

## Migration Plan

1. 先修 D1（后端 bug 修复，独立可落盘），test-then-commit。
2. 再加 D2/D3/D4（新 IPC + Tauri command）。
3. 再加 D5（NotificationsView header 批量 + row hover delete）。
4. 最后 D6（SettingsToggle 组件 + SettingsView 4 处替换）。
5. 每一步都 `just preflight`（fmt + lint + test + spec-validate）可过后再进下一步。
6. 无数据迁移；配置文件 schema 不变；回滚只需 revert 对应 commit。

## Open Questions

- **Q1**：修完 D1 后用户仍反馈"删除 trigger 无效"怎么办？→ 计划：在 apply 阶段跑 `just dev`，手测 "新建 trigger → 触发一次通知 → 删除 trigger" 全流程；若 remove_trigger 路径也有 bug，补一个 followup change（预估在 `cdt-api` 的 Arc clone 层面，概率低）。本轮 proposal 假设根因是 D1 描述的 update_notifications bug。
- **Q2**：是否应在删除 trigger 时一并清理该 trigger 产生的历史通知？→ 暂不做。原版 TS 也不做——历史通知作为用户已看到的审计记录保留更合理，用户想清理可以用本 change 新增的 "清空" 按钮。如果后续需要按 `trigger_id` 精准清，`clear_notifications(trigger_id: Some("..."))` 已预留。
