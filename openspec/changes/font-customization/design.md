## Context

Port 当前的 UI 字体策略与 TS 原版（`../claude-devtools/src/renderer/`）相比有两个差距：

1. **fallback 栈短**：`ui/src/app.css:129-130` 的 `--font-sans` 只列了 5 项，`--font-mono` 只列了 5 项，对比原版的 10 项 sans + 6 项 mono 在跨平台兼容上偏弱（缺 `Oxygen` / `Cantarell` / `Fira Sans` / `Droid Sans` / `Helvetica Neue` / `Monaco` 等历史 fallback）
2. **不可配置**：原版本身也没字体配置功能；port 决定**新增**这个能力——这是 port 相对原版的合理超集

现有基础设施可直接复用：
- `lib/theme.ts::applyTheme(theme)` 已有「读 config → 写 `:root` 属性」的范式
- `crates/cdt-config/src/types.rs::DisplayConfig`（line 149）是字体字段的自然落点
- `getConfig` / `updateConfig` 的 IPC 路径自动透传 serde 字段，无须改 Tauri 层
- `SettingsView.svelte` 已有「显示」段配 toggle，加 input 是同段扩展

## Goals / Non-Goals

**Goals:**

- 用户可在 Settings 输入自定义 sans / mono `font-family` 字符串（CSS 标准格式，例如 `"JetBrains Mono", monospace`）
- 默认值（用户未设置时）覆盖 TS 原版字体栈，跨 macOS / Linux / Windows 都有合适 fallback
- 「恢复默认」一键清除用户覆盖
- 改字体后立即生效，无需重启
- 老配置文件升级零迁移（serde default → None）

**Non-Goals:**

- 字体大小 / 行高调整（独立功能，本次不做）
- 字体预设下拉（System / JetBrains Mono / Fira Code 等候选，MVP 用纯文本输入；后续迭代可加）
- 字体加载验证（不检查用户输入的字体是否真存在；浏览器对未知字体宽容降级）
- 网页字体（@font-face / Google Fonts 加载）
- 主题字体（区分浅色 / 深色字体，无意义）

## Decisions

### D1：字段挂在 `display.*` 而非 `general.*`

`DisplayConfig` 已包含 `show_timestamps` / `compact_mode` / `syntax_highlighting` 这类纯展示偏好，字体属于同语义类别。`general.*` 装的是「应用级行为」（launchAtLogin / dockIcon / theme / claudeRootPath），字体不属于此类。

**候选方案**：`appearance.*` 子分组——但目前没有 appearance 分组，新建一个 schema 段会让向后兼容路径更复杂，valued less than 重用 `display`。

### D2：`Option<String>` vs 空字符串

选 `Option<String>`：
- Rust 端 `None` 语义最清晰（"用户未设置"，区别于 `Some("")` 这种"用户故意设空"歧义）
- serde 配 `#[serde(default, skip_serializing_if = "Option::is_none")]` 让老配置文件读取时缺字段→None，不需要数据迁移
- 序列化老前端不见新字段，向后兼容

前端把用户输入 `trim()` 后空白视同 None：调 `updateConfig` 时传 `null`；后端把 `Some(s)` 中 `s.trim().is_empty()` 也归一化为 `None` 持久化（防御）。

**候选方案**：用 `String`（默认空字符串）——Rust 端要到处写 `if !s.is_empty()` 检查，难看且容易漏。

### D3：不做 font-family 字符串解析校验

CSS `font-family` 语法宽松（多个候选用逗号分隔，含空格的字体名加引号），完整解析需要 CSS parser，复杂度极高且收益低——浏览器对未知字体宽容降级到下一个 fallback，最坏情况是用户看到默认字体，可逆。

只做：
- 后端 `manager.rs::update_field` 不限制字符串内容（不像 `claude_root_path` 要校验绝对路径）
- 前端 input 不加 pattern；提交前 `trim()`

**候选方案**：用 `<select>` 限定预设字体——见 D4。

### D4：MVP 用文本输入，预设下拉留作后续

文本输入足够灵活（用户可填任何 CSS font-family），后续迭代叠加「预设下拉 + 自定义」两段式 UI 不破坏数据格式。MVP 阶段：
- placeholder 给原版字体栈作示例
- 旁边一行 muted 提示「示例：`"JetBrains Mono", monospace`」
- 「恢复默认」按钮清空字段调 `updateConfig({ display: { fontSans: null } })`

**候选方案**：直接做下拉——MVP 不必，等用户反馈再加预设清单。

### D5：默认 token 写在 `app.css`，`fonts.ts` 只做 `removeProperty`

避免「默认值」在两处维护（app.css 和 ts 常量）后失同步。流程：
- `app.css` `:root { --font-sans: ...; --font-mono: ...; }` 是单一真相源
- `fonts.ts::applyFonts(config)`：
  - `config.display.fontSans` 非空 → `document.documentElement.style.setProperty('--font-sans', value)`
  - 空/null → `document.documentElement.style.removeProperty('--font-sans')`，让 `:root` 默认值复活
- mono 同理

App.svelte 启动时（`onMount` 已读 config 调 applyTheme）追加调 `applyFonts(config)`；SettingsView 改字段乐观更新本地 `$state` + 即时调 `applyFonts(localConfig)`，再异步 `updateConfig`。

**候选方案**：在 `fonts.ts` 维护默认值常量，applyFonts 永远 setProperty——会把 `:root` 默认 token 形同虚设，且默认值改动要同步两处。

### D5b：显式接受首帧 FOUT，不引入启动前 inline style 注入

codex 二审指出（2026-04-27）：D5 onMount 后读 config 调 applyFonts 的路径，对**已设非默认字体**的用户首帧会吃 `app.css` 的默认 token，onMount 完成后切换到自定义字体，存在 FOUT（Flash of Unstyled Text）。

候选方案：
- **A. 接受 FOUT（采纳）**：与 `lib/theme.ts::applyTheme` 现有路径一致（首帧吃浅色 / 系统色，onMount 后切深色），项目内已默认接受此行为；字体 fallback 栈跨平台 metrics 相近，layout shift 影响小；不引入新的启动期同步路径
- **B. localStorage bootstrap**：`index.html` 加 inline `<script>` 在 Svelte mount 前 sync 读 localStorage（缓存最近一次的 fontSans / fontMono）写 `<style>` 注入；首次启动仍闪一次（缓存为空），但二次启动起无闪。复杂度中：需要在 `updateConfig` 路径同步写 localStorage、main.ts 入口加 bootstrap 逻辑、与 Tauri 持久化语义双写需保证一致
- **C. SSR 风格静态注入**：vite build 预生成；不适用，因为字体值是用户配置而非编译期常量

采纳 A：FOUT 不是数据正确性问题且可逆；与 theme 路径一致避免引入异构启动模式；后续若用户反馈首帧字体闪烁明显可单独迭代加 B 方案，不影响本 change 的 schema / IPC 契约。

### D6：默认字体栈与原版完全一致

```css
--font-sans:
  -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Roboto', 'Oxygen', 'Ubuntu', 'Cantarell',
  'Fira Sans', 'Droid Sans', 'Helvetica Neue', sans-serif;
--font-mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
```

直接照搬 TS 原版栈（sans 见 `index.css:393-395`，mono 见 `UserChatGroup.tsx:64`）。

## Risks / Trade-offs

- **[风险] 用户输入恶意 / 超长字符串注入 CSS**：`setProperty('--font-sans', value)` 会把字符串当 CSS 变量值；理论上恶意字符串如 `red; } body { display: none } #x {` 不会注入到外层样式（CSS 变量值本身是 token 化保存，只在 `var()` 引用处展开为 `font-family` 值；`font-family` 解析器会对非法值降级 fallback）。**缓解**：仍做长度上限校验（后端拒 > 500 字符），日志记录但不强阻。
- **[风险] 用户填非法字符串导致字体降级到浏览器默认**：可逆——「恢复默认」一键清除。**缓解**：UI 给 placeholder + 示例提示。
- **[风险] 字体改动闪烁**：theme 路径已证明 `applyXxx` 在 onMount 中调用无可见闪烁；fonts 路径同构。**缓解**：保持 onMount 顺序与 theme 平行。
- **[trade-off] 不做预设下拉**：用户需要知道 CSS font-family 语法。可接受——这是开发者工具，目标用户对 CSS 不陌生；后续按反馈加预设。
