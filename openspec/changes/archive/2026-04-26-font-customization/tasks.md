## 1. cdt-config（后端 schema + 持久化）

- [x] 1.1 `crates/cdt-config/src/types.rs::DisplayConfig` 加 `pub font_sans: Option<String>` + `pub font_mono: Option<String>`，`#[serde(default, skip_serializing_if = "Option::is_none")]`
- [x] 1.2 `crates/cdt-config/src/defaults.rs` 默认 `DisplayConfig` 两个新字段填 `None`
- [x] 1.3 `crates/cdt-config/src/manager.rs::update_display` 加 `fontSans` / `fontMono` 分支：实现 SHALL 整次原子（先在临时变量上验证所有字段，全部通过才写入 `self.config` 并 `save`，任一字段非法则整次返回 error）。校验规则：长度 > 500 字符拒绝；`Some(s)` 中 `s.trim().is_empty()` 归一化为 `None`；非空保留原字符串；`null` 归一化为 `None`
- [x] 1.4 单测：`tests/` 或 mod tests 覆盖以下 scenario
  - 老配置文件无字体字段 → 加载后 `font_sans` / `font_mono` 都是 `None`，其他字段保留
  - 设非空字符串 → 持久化往返一致
  - 设全空白字符串 → 持久化为 `None`
  - 显式设 `null` → 持久化为 `None`
  - 设超长字符串（> 500 char）→ 返回 validation error，已有值不变
  - 同次 update sans+mono 一个超长 → 整次拒绝，两个字段都不变（原子性）
  - reset_to_defaults 把已设的字体覆盖清空回 `None`（默认配置语义自带，但单测显式断言）
- [x] 1.5 无新增 cargo 依赖（明示 reviewer，避免 schema 扩展时误以为要加 dep）

## 2. cdt-api（IPC contract）

- [x] 2.1 `crates/cdt-api/tests/ipc_contract.rs` 加 contract test：`getConfig` 响应 `display` 含 `fontSans` / `fontMono` 键（camelCase 序列化）
- [x] 2.2 `crates/cdt-api/tests/ipc_contract.rs` 加 contract test：`updateConfig({ display: { fontSans: null } })` 反序列化成功
- [x] 2.3 `crates/cdt-api/tests/ipc_contract.rs` 加 contract test：`updateConfig({ display: { fontMono: "\"Fira Code\", monospace" } })` 反序列化成功
- [x] 2.4 `crates/cdt-api/tests/ipc_contract.rs` 加 contract test：`updateConfig({ display: { fontSans: "   " } })`（仅空白）持久化后读回 `null`

## 3. ui（前端类型 + 运行时应用）

- [x] 3.1 `ui/src/lib/api.ts::DisplayConfig` interface 加 `fontSans: string | null` + `fontMono: string | null`
- [x] 3.1b `ui/src/lib/__fixtures__/*.ts`（empty / single-project / multi-project-rich）的 mock config 同步加 `fontSans: null` + `fontMono: null`（CLAUDE.md "IPC 字段改动 checklist (c)" 硬约束）
- [x] 3.2 新建 `ui/src/lib/fonts.ts`：导出 `applyFonts(config: ConfigData)`；非空值 `setProperty('--font-sans' / '--font-mono', value)`，空/null `removeProperty`
- [x] 3.3 `ui/src/App.svelte` 启动 onMount 顺序：读 config → `applyTheme` → 新增 `applyFonts(config)`
- [x] 3.4 `ui/src/app.css:129-130` `--font-sans` / `--font-mono` 默认值改成 D6 字符串（对齐原版栈）
- [x] 3.5 `ui/src/components/TeammateMessageItem.svelte:277` 硬编码 `ui-monospace, SFMono-Regular, Menlo, monospace` 改成 `var(--font-mono)`

## 4. ui Settings UI

- [x] 4.1 `ui/src/routes/SettingsView.svelte` 「显示」段加两个 `<input type="text">`：sans / mono
  - placeholder 给原版栈作示例
  - 旁边 muted 提示文案：示例语法
  - 「恢复默认」按钮：调 `updateConfig({ display: { fontSans: null, fontMono: null } })`
- [x] 4.2 乐观更新模式：input 改值 → 本地 `$state` 更新 → 立即 `applyFonts(localConfig)` → 异步 `updateConfig`，失败回滚（重新 `getConfig`）
- [x] 4.3 input 失焦或 Enter 时 `trim()` 后空字符串归一化为 `null`

## 5. 验证 + 文档

- [x] 5.1 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 5.2 `cargo fmt --all`
- [x] 5.3 `cargo test -p cdt-config -p cdt-api`
- [x] 5.4 `npm run check --prefix ui`
- [x] 5.5 `openspec validate font-customization --strict`
- [x] 5.6 `cargo tauri dev` 手动 smoke：设置 sans = `"Comic Sans MS"`，UI 立即换字体；点恢复默认，回到原版栈；改 mono 同理（用户已验证生效）
