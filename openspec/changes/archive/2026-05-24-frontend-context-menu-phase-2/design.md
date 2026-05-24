# Design: frontend-context-menu-phase-2

## Decisions

### D-V1: menu item icon 策略

**候选方案：**
- A) 全部 item 加 16px lucide icon（与 VS Code / Finder 右键对齐）
- B) 仅 submenu trigger + 文件/外部类 action 加 icon（区分"动作类型"）
- C) 维持 Phase 1 无 icon，仅 submenu trigger 加 `›` chevron 指示器

**选了：** C — 维持无 icon + submenu chevron

**取舍：**
- Phase 1 D3 已决定纯文字菜单，Phase 2 若加 icon 会打破既有一致性且 Phase 1 场景（sidebar 右键、tab 右键）需要回溯补 icon
- `PRODUCT.md::Design Principles §3` "密度有层次" 要求用**边框、色阶、字号和折叠**控制认知负荷——icon 不在手段清单中
- `DESIGN.md::The Tool Density Rule` 禁止为风格制造额外视觉元素；菜单已通过 separator 分组提供类型识别，icon 是冗余信号
- submenu 的 `›` chevron 是结构性 affordance（"这里有子菜单"），不是装饰——等同 BaseItem 的 disclosure chevron，属合规

**风险：** 文件操作类 action（"在编辑器打开"/"在 Finder 中显示"）缺 icon 后辨识度可能略低于 Finder 原生菜单。接受——`PRODUCT.md::Anti-references` 明确不重造标准控件仅为"好看"。

---

### D-V2: shortcut hint 右侧显示

**候选方案：**
- A) 不显示快捷键提示
- B) 右侧灰色 `⌘C` / `⌘⇧E` 等文本 hint

**选了：** B — 右侧 hint

**取舍：**
- `PRODUCT.md::Design Principles §2` "熟悉即效率"——macOS / VS Code / Sublime 所有原生右键菜单均右对齐显示 shortcut，桌面用户已有肌肉记忆在菜单右侧找快捷键
- 仅对已注册全局 keyboard shortcut 的 item 显示（如"复制"对应 ⌘C）；无 shortcut 的 item 右侧留空
- `DESIGN.md::The Machine Information Rule` 要求键盘修饰符用 mono 辅助识别

**视觉规格（引用 DESIGN.md token，不新建）：**
- color: `--color-text-muted`
- font: `var(--font-mono)` / `11px` / `400`
- 位置: item 行内右对齐，`margin-left: auto` + `padding-left: 16px`

**风险：** hint 占用水平空间可能让长 label item 折行。通过 D-V6 max-width 约束 + `white-space: nowrap` + `overflow: hidden` + `text-overflow: ellipsis` 对 label 部分截断解决。

---

### D-V3: separator 分组语义

**候选方案：**
- A) 按动作目标分组（剪贴板 / 导航 / 外部）
- B) 按动作类型分组（只读 / 写入 / 跳转）
- C) 不分组，全部 flat 列表

**选了：** A — 三段式按目标分组

**取舍：**
- `PRODUCT.md::Design Principles §3` "密度有层次"——separator 是层次手段之一
- 三段顺序：① 复制/剪贴板（最高频、零风险、⌘C 肌肉记忆）→ ② 应用内导航（deeplink / 跳到 chunk）→ ③ 外部应用（打开编辑器 / 终端 / 浏览器搜索）
- 此顺序对齐 Finder / VS Code 右键的惯例层级（"操作当前内容" → "去到某处" → "委托给外部"）

**约束规则：**
- surface 不包含某段全部 item 时该 separator 不渲染（如纯文本选区菜单无导航类 → 只有 ① ③ 之间的 separator）
- 首项前 / 末项后禁止孤立 separator（menu-items builder 负责 trim）

**风险：** 低。Separator 是已落地的 Phase 1 基础设施。

---

### D-V4: submenu 引入

**候选方案：**
- A) 不用 submenu，直接 flat 列出所有编辑器/终端选项（5-8 项展平）
- B) submenu 二级展开（"在编辑器打开 ›" → VS Code / Cursor / Zed / Sublime）
- C) Settings 预选默认应用 → 菜单只显示默认选项的一项

**选了：** B + C 混合 — 有 Settings 默认值时直接显示"在 {defaultEditor} 打开"单项；仅当 Settings 为"每次选择"时 fall back 到 submenu

**取舍：**
- `PRODUCT.md::Design Principles §1` "审计优先"——多数用户固定一个编辑器/终端，菜单项应一步到位；强制每次从 submenu 选违背"快速定位"
- submenu 保留为 fallback 而非默认路径，降低日常使用的认知开销
- submenu 的 `›` chevron 右对齐（与 D-V2 shortcut hint 位置互斥——有 submenu 的 item 不带 shortcut hint）

**submenu 交互规格（引用 DESIGN.md 已有语言）：**
- 进入延迟: hover `200ms` / ArrowRight 即时打开
- 离开判定: 鼠标移出 parent item + 未进入 submenu 区域 → `150ms` 后关闭（标准 submenu "安全三角" hysteresis）
- 键盘: ArrowRight 打开 + focus 进 submenu 首项 / ArrowLeft 关闭 submenu + focus 回 parent / Esc 关闭整棵菜单树
- 形态: 与父菜单完全相同 bg / border / radius / shadow（`DESIGN.md::Context menu` 段已定义）
- 定位: 默认右侧展开；viewport 右边距不足时翻转到左侧

**风险：** submenu "安全三角" 实现复杂度中等（需 mousemove tracking + 几何判定）。可先用简化版（纯延迟，不做三角），Phase 3 再 polish。

---

### D-V5: active state 对比度

**候选方案：**
- A) 维持现有 `outline: 2px solid rgba(59, 130, 246, 0.15)` + `--tool-item-hover-bg`
- B) 加深 outline 到 `rgba(59, 130, 246, 0.3)` 提升键盘可见性
- C) 替换 outline 为 left indicator（2px inset bar）

**选了：** A — 维持现有实现

**取舍：**
- WCAG 1.4.11 非文本对比要求 ≥ 3:1 针对的是"active state 与 default state 之间的差异"——`--tool-item-hover-bg` 色阶变化是主信号，outline 是补充
- Phase 1 已通过验证（`DESIGN.md::Context menu` 段已固化此视觉语言）：`hover 与 keyboard active 共用 --tool-item-hover-bg；keyboard active 额外加 2px solid rgba(59, 130, 246, 0.15) outline 作为瞬时键盘焦点提示`
- C 方案的 left indicator 违反 `DESIGN.md::§6 Don't` "不要用粗 border-left 装饰列表项"

**风险：** 低。Phase 1 已验证，不改。

---

### D-V6: min-width / max-width 与路径截断

**候选方案：**
- A) 固定宽度 220px（Phase 1 当前）
- B) `min-width: 200px` / `max-width: 320px`，label 截断
- C) 自适应宽度无上限，label 不截断

**选了：** B — 带上限约束 + 路径中段截断

**取舍：**
- Phase 2 引入路径类 item（"在编辑器打开 ~/Rust.../contextMenu.svelte.ts"），加上右侧 shortcut hint，水平内容会超过 220px
- `max-width: 320px` 保证菜单不过宽（桌面窗口最小宽度约 800px 时菜单不超 40% 宽度）——`DESIGN.md::§4 Floating overlay` 定义的 dropdown/popover 是 "8–12px radius、emphasis border"，不应过大
- 路径截断策略：`DESIGN.md::The Machine Information Rule` 要求路径用 mono——中段省略 `~/Rustrove…/contextMenu.svelte.ts`（保留首段 `~/` 识别上下文 + 尾段文件名识别目标），`title` 属性悬浮显示完整路径

**风险：** CSS 原生不支持中段 ellipsis，需 JS 截断（`menu-items.ts` builder 内预处理 label）。实现成本低。

---

### D-V7: 暗色模式 submenu 层级

**候选方案：**
- A) submenu bg 加深一档（`--color-surface-raised` → `--color-surface-overlay`）
- B) 所有层级统一 `--color-surface`（与父菜单相同）
- C) submenu 加强 shadow 区分

**选了：** B — 统一 bg，不加额外 shadow

**取舍：**
- `DESIGN.md::§1 Overview` "系统整体是 flat + tonal layering"——tonal layering 用于语义层级（sidebar < main < overlay），不用于同语义的同级元素之间
- 父菜单与 submenu 是同一语义的"选择浮层"，仅空间位置不同——加深 bg 会暗示 submenu 内容"更重要/更深层"，误导
- `DESIGN.md::The Border Before Shadow Rule` 已允许浮层用 shadow；submenu 本身已有同规格 shadow（`0 4px 16px rgba(0, 0, 0, 0.15)`），空间位移 + 既有 shadow 已提供层次，追加 shadow 是装饰
- 暗色模式下 `--dark-surface: #1e1e1c` + `1px solid --dark-border-emphasis: #4f4e4a` + 同级 shadow 已足够区分浮层与背景

**风险：** submenu 与父菜单视觉重叠时（viewport 边缘翻转导致部分重叠）用户可能短暂困惑哪个是哪层。可通过 submenu 加 `2px` 垂直偏移（submenu top = parent item top - 4px）让空间位置提供 cue。

---

### D1: `open_in_terminal` 跨平台 spawn 策略

**候选方案：**
- A) macOS `osascript -e 'tell application "Terminal" to do script "cd ..."'` + Win `wt.exe -d` + Linux `x-terminal-emulator`
- B) macOS `open -a <App> <path>` + Win `wt.exe -d` fallback `cmd /K cd /d` / `powershell -NoExit -Command Set-Location` + Linux `x-terminal-emulator --working-directory=<path>` fallback DE-specific

**选了：** B — 每平台走最安全的"打开目录"原语，**不拼 shell command 字符串**

**取舍：**
- A 的 `osascript` 有 script 注入风险：path 含 `"` / `$` / `` ` `` 需 escape，escape 不全 = 任意 AppleScript 执行
- B 的 `open -a <App> <path>` 走 macOS Launch Services，仅 cd 到 path 无 shell 执行上下文，**零注入面**
- macOS / Linux / Windows Terminal `wt.exe` 都通过 `std::process::Command::new(exe).arg(path)` 传参（OS-level argv），**不拼字符串**——OS 负责 quoting
- Windows PowerShell / cmd fallback 必须特殊处理：这两条命令本质走 shell parser，path 含 `&` / `|` / `<` / `>` / `^` / `(` / `)` / `%` / `!` / `'` / `"` / 换行等元字符时即使加 `-LiteralPath` 引号也可能被 shell 解释；解决方案是**把 path 用环境变量传**，命令字符串内只引用 `$env:CDT_TARGET_PATH` / `%CDT_TARGET_PATH%`，path 完全不进 shell parser

**安全实现细则（Windows fallback）：**

```rust
// PowerShell：path 走 env var
Command::new("powershell.exe")
    .args(["-NoExit", "-Command", "Set-Location -LiteralPath $env:CDT_TARGET_PATH"])
    .env("CDT_TARGET_PATH", path)
    .spawn()

// cmd：同样走 env var；cmd 内置 cd 支持 %VAR% 展开但不会重新 shell-parse 内容
Command::new("cmd.exe")
    .args(["/K", "cd /d \"%CDT_TARGET_PATH%\""])
    .env("CDT_TARGET_PATH", path)
    .spawn()
```

注意 cmd 的 `%VAR%` 展开发生在 cmd 启动后的内部解析，但展开后的字符串仍由 cmd 重新 tokenize ——这一层风险用 path 含 `&` 实测拦截：path 含 cmd metacharacters 时**直接拒绝**，返回 `ApiError::ValidationError("path contains characters unsafe for Windows shell: ...")` 引导用户重命名。Phase 2 接受此 trade-off（path 含特殊字符的合规目录极少）。后续可改用 PowerShell `-EncodedCommand` UTF-16LE base64 完全消除 cmd parser，phase 3 再做。

**跨平台 spawn 映射表：**

| 平台 | `TerminalApp` variant | 命令构造 |
|---|---|---|
| macOS | `Terminal` | `Command::new("open").args(["-a", "Terminal"]).arg(path)`（OS-argv，零注入） |
| macOS | `ITerm` | `Command::new("open").args(["-a", "iTerm"]).arg(path)` |
| macOS | `Warp` | `Command::new("open").args(["-a", "Warp"]).arg(path)` |
| Win | `WindowsTerminal` | `Command::new("wt.exe").args(["-d"]).arg(path)`（OS-argv） |
| Win | `PowerShell` | path 走 `CDT_TARGET_PATH` env var，命令固定 `Set-Location -LiteralPath $env:CDT_TARGET_PATH` |
| Win | `Cmd` | path 走 `CDT_TARGET_PATH` env var + path 含 cmd metachar 时拒绝 |
| Linux | `XTerminalEmulator` | `Command::new("x-terminal-emulator").arg("--working-directory").arg(path)` |
| Linux | `GnomeTerminal` | `Command::new("gnome-terminal").arg("--working-directory").arg(path)` |
| Linux | `Konsole` | `Command::new("konsole").arg("--workdir").arg(path)` |
| Linux | `Alacritty` | `Command::new("alacritty").arg("--working-directory").arg(path)` |

**IPC 签名：**
```
command: open_in_terminal
入参: { path: String }
返回: Result<(), ApiError>
```

后端从 `ConfigManager` 读当前 `terminal_app` 设置做分流——**command 本身不接受 terminal 参数**，避免前端伪造终端名绕过白名单。path 为文件时自动取 `parent()` 降级到目录。

**风险：**
- macOS `open -a` 的 app 名须严格匹配 bundle name（iTerm 对应 `iTerm`，需在实现中验证）
- Linux `x-terminal-emulator` 在非 Debian 系发行版可能不存在，fallback 逐个 `which`
- Windows `wt.exe` 在 Win 10 旧版可能未预装

---

### D2: `open_in_editor` CLI 跳行号约定 + System fallback 链

**候选方案：**
- A) 前端传 editor 参数 + CLI 名，后端盲 spawn
- B) 后端从 Settings 读 `external_editor`，按白名单 dispatch CLI + 跳行号格式

**选了：** B — 后端封闭 dispatch，不信任前端传入的 editor 标识

**取舍：**
- A 允许前端传任意 CLI 名 = 任意命令执行（RCE），安全不可接受
- B 白名单有限集合，每个 editor 的 CLI 名 + arg 格式在代码里 hardcode

**Editor CLI 映射表：**

| `ExternalEditor` | CLI 可执行文件 | 跳行号格式 | 探测命令 |
|---|---|---|---|
| `VsCode` | `code` | `code --goto <path>:<line>:<col>` | `code --version` |
| `Cursor` | `cursor` | `cursor --goto <path>:<line>:<col>` | `cursor --version` |
| `Zed` | `zed` | `zed <path>:<line>:<col>` | `zed --version` |
| `Sublime` | `subl` | `subl <path>:<line>:<col>` | `subl --version` |
| `System` | macOS `open` / Win `cmd /C start ""` / Linux `xdg-open` | 不支持跳行号 | — |

**Windows drive letter colon 与 `--goto path:line:col` 冲突说明：**

VS Code / Cursor / Zed / Sublime 的 `--goto` parser 在 Windows 上**已知支持 drive letter colon 智能识别**——`code --goto C:\foo\bar.rs:42:8` 内部从字符串末尾向前找最后两个数字 `:` 段（line/col），剩下的 `C:\foo\bar.rs` 作为 path（drive letter colon 后跟非数字 backslash 不被误判为 line 数字）。Phase 2 实现 SHALL 直接走标准 `<path>:<line>:<col>` 拼接 + 在 Windows runner / VM smoke 测试中显式验证 drive letter path（`C:\Program Files\test.rs:42:8`）+ 含 `:` 的非数字目录名（罕见但合规 NTFS 不允许 path 含 `:`，drive letter 是唯一例外）。Phase 2 SHALL 在 IPC contract test 加 `argv` 拼接断言验证 Windows drive letter 边界 case；Phase 3 视真实用户报告再决定是否换 `code --reuse-window <path> --goto-line <line>` 等 alt 格式。

**fallback 链：**
1. 读 `general.externalEditor` 配置
2. `System` → OS 默认打开文件（macOS `open <path>` / Win `cmd /C start "" "<path>"` / Linux `xdg-open <path>`），`line`/`column` 参数忽略
3. 具名 editor → spawn 探测（`<cli> --version` 可执行即认为已装）；不存在 → 返回 `ApiError::external_app("editor CLI '<cli>' not found; install or change Settings")`
4. `line`/`column` 为 `None` 时省略行号后缀
5. spawn 用 `.spawn()`（非阻塞），不等待 editor 进程退出

**IPC 签名：**
```
command: open_in_editor
入参: { path: String, line: Option<u32>, column: Option<u32> }
返回: Result<(), ApiError>
```

同 D1，editor 从 ConfigManager 读取，**不从前端传参**。

**风险：**
- Cursor CLI 可能装成别名，只认 `cursor` 官方名
- Windows 上 `which` 不可用，探测改用 `Command::new(cli).arg("--version").output()` 尝试执行
- `xdg-open` 可能阻塞直到 editor 关闭——用 `.spawn()` 非阻塞

---

### D3: Settings 三字段 enum 设计

**候选方案：**
- A) 自由文本 `String` 字段
- B) 扁平 `#[serde(rename_all = "snake_case")]` enum
- C) internally tagged enum（仅 `SearchEngine` 需要带 `Custom` 额外字段）

**选了：** `ExternalEditor` + `TerminalApp` 用 B（扁平 enum）；`SearchEngine` 用 C（internally tagged）

**三字段均加入 `GeneralConfig`**（与 `theme` / `defaultTab` 同级），`#[serde(default)]` 保证旧配置文件兼容。

#### `ExternalEditor`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExternalEditor {
    #[default]
    System,
    VsCode,
    Cursor,
    Zed,
    Sublime,
}
```

IPC 序列化值：`"system"` / `"vs_code"` / `"cursor"` / `"zed"` / `"sublime"`。
camelCase 字段名：`externalEditor`。

#### `SearchEngine`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SearchEngine {
    #[default]
    Google,
    Bing,
    DuckDuckGo,
    Custom {
        #[serde(rename = "urlTemplate")]
        url_template: String,
    },
}
```

IPC 序列化值：`{ "type": "google" }` 或 `{ "type": "custom", "urlTemplate": "https://example.com/search?q={query}" }`。
camelCase 字段名：`searchEngine`。

**`Custom` 校验**：`url_template` 必须满足两条：(a) 含 `{query}` 占位符；(b) URL scheme ∈ `{http, https}`（拒绝 `javascript:` / `file:` / `data:` / `chrome://` 等危险 scheme，防 XSS-into-opener 路径）。任一不满足 `update_general` 返回 `ApiError::ValidationError`。前端拼接 URL 走 `urlTemplate.replace("{query}", encodeURIComponent(query))` 后调 `plugin:opener|open_url`。

#### `TerminalApp`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TerminalApp {
    // macOS
    #[default]
    Terminal,
    ITerm,
    Warp,
    // Windows
    WindowsTerminal,
    Cmd,
    PowerShell,
    // Linux
    XTerminalEmulator,
    GnomeTerminal,
    Konsole,
    Alacritty,
}
```

IPC 序列化值：`"terminal"` / `"i_term"` / `"warp"` / `"windows_terminal"` / `"cmd"` / `"power_shell"` / `"x_terminal_emulator"` / `"gnome_terminal"` / `"konsole"` / `"alacritty"`。
camelCase 字段名：`terminalApp`。

**设计决策：统一 enum 而非 per-platform enum。**

取舍：
- 统一 enum 让前端 Settings UI 不需 `#[cfg]` 分叉；前端通过 `list_available_terminals` IPC 获取当前平台合法列表过滤 dropdown 选项
- 配置文件跨平台可移植（跨 OS 同步 config 不会 deserialize 失败，运行时不匹配则 fallback 到平台默认）
- `Default` 值固定 `Terminal`（macOS）；运行时 `open_in_terminal` 根据 `cfg!(target_os)` 判断，值与 OS 不匹配时 tracing::warn + fallback 到平台默认

**新增辅助 IPC（前端 Settings dropdown 需要）：**
```
command: list_available_terminals
返回: Vec<String>  // 当前平台支持的 terminal 枚举值列表，如 ["terminal", "i_term", "warp"]
```

**跨平台 mismatch UI 处理（codex 二审 DESIGN_HOLE #12 补）：**

跨平台同步配置（如用户从 Windows 同步到 macOS）时 settings 文件已存的 `terminalApp` 可能不在当前平台 `list_available_terminals` 返回集合中。Settings dropdown SHALL：
1. 调 `list_available_terminals` 拿当前平台合法 options 列表
2. 检查当前 settings `terminalApp` 是否在列表中
3. **不在**列表时：dropdown 显示 disabled "{currentValue} (not available on {os})" 选项 + 默认 selected 当前平台 fallback 值（如 macOS 上 `windows_terminal` mismatch → 显示 disabled "windows_terminal (not available on macOS)" + 默认 select "terminal"）
4. 同时显示一行小字 hint: "Synced from another platform; will fall back to {fallback}. Select to update."
5. 用户主动改 dropdown 后写入新值；不改则保持原值（运行时按 fallback 行为）

这让用户清楚看到 cross-platform sync 状态，**不**默默 silent fallback——避免"我设了 iTerm 怎么没生效"的迷惑。

#### `update_general` 扩展

三个新 match arm：
- `"externalEditor"` → `serde_json::from_value::<ExternalEditor>(v)` 校验
- `"searchEngine"` → `serde_json::from_value::<SearchEngine>(v)` 校验 + Custom urlTemplate 含 `{query}` 断言
- `"terminalApp"` → `serde_json::from_value::<TerminalApp>(v)` 校验 + 跨平台不匹配时 warn 但不拒绝

**风险：**
- `serde(rename_all = "snake_case")` 对 `ITerm` 输出 `"i_term"`（不是 `"iterm"`）—— 前端须对齐此 key
- `SearchEngine` internally tagged enum + default 需要 serde `#[serde(default)]` 在 struct field 级别，unit variant 用 `{}` 或 `{ "type": "google" }` 均可 deserialize

---

### D4: capabilities allow-list 方案 + path 校验防 RCE

**候选方案：**
- A) 使用 `tauri-plugin-shell` 的 sidecar / scope 机制
- B) 自定义 Tauri commands 走 `invoke_handler!`，后端内部 spawn

**选了：** B — 自定义 commands，**不引入 `tauri-plugin-shell`**

**取舍：**
- `tauri-plugin-shell` 的 capabilities scope（`shell:allow-execute` + allowlist pattern）设计复杂且需精确维护；一旦 scope 过宽 = RCE
- 自定义 commands 的攻击面完全封闭在后端代码——只有 D1/D2 表格中的有限 CLI 可被 spawn，前端无法指定任意程序
- `capabilities/default.json` **不需要新增条目**——自定义 commands 注册在 `invoke_handler!` 即对 `default` capability 下所有 windows 可用（Tauri 2 capabilities 仅管控 plugin 权限）

#### Path 校验策略（在 command handler 内执行）

1. **绝对路径校验**：`cdt_discover::looks_like_absolute_path(&path)` → 拒绝相对路径
2. **Canonicalize**：`tokio::fs::canonicalize(&path).await` → 解析 symlink / `..` / 判存在性（一步完成 traversal 防护 + 存在性校验）
3. **目录类型（仅 `open_in_terminal`）**：`metadata.is_dir()` → 文件 path 自动取 `canonicalized.parent()` 降级
4. **Windows cmd metacharacter 拒绝（仅 cmd fallback）**：path 含 `&` / `|` / `<` / `>` / `^` / `(` / `)` / `%` / `!` / `'` / `"` / 换行 时拒绝（详 D1 安全实现细则）
5. **不限制"已发现 project 范围"**（评估后决定不加）：
   - 理由：session cwd 可能不在已发现 project 之内（subagent worktree / 临时目录）
   - 攻击面 = webview XSS；若 XSS 已发生，attacker 可调已有 `opener:allow-open-path`（scope `$HOME/**`），限制 custom command path 无额外安全增益
   - 最坏场景：打开用户机器上已有目录的终端窗口 / 打开已有文件——**无命令执行**

#### Threat model：path 来源可信度（codex 二审 DESIGN_HOLE #3 补）

新 IPC 的 path 入参来自前端 menu factory 构造的 `ContextMenuItem.action` 闭包，回溯到三个数据源：

| 数据源 | 可信度 | 备注 |
|---|---|---|
| **A) 用户主动选择**（项目卡 / worktree chip path）| **高** | 来自后端 `list_projects` 扫描 `~/.claude/projects/`，已经过 `cdt-discover` 路径合法性筛选 |
| **B) chunk JSONL 数据**（Bash cwd / Read 工具 file_path / Edit 工具 file_path）| **中** | 来自 Claude Code 写入的 session JSONL 文件，技术上**用户自有数据**——除非 webview XSS 注入伪造 chunk，否则无 attacker 控制路径 |
| **C) 文本选区**（任意可见字符串）| **低** | 用户选中的任意文本，selection menu 不直接当 path 使用——**仅** "在浏览器搜索"action 消费，URL 拼接走 D3 `searchEngine.urlTemplate` + `encodeURIComponent`，不进 path 层 |

**结论**：path 校验仅做 (1)-(4) 的 OS-level 安全（绝对路径 + canonicalize + 文件类型 + Windows cmd metachar），**不**做来源信任级别区分。理由：
- (A) (B) 都是已落盘的合法 OS path；canonicalize 失败说明路径不存在（OS 自动拒绝）
- (C) 不进 path 接口（design 已显式拆分）
- 假设 webview XSS 已发生，attacker 已能调任意 IPC——限制 path 来源无新增防护
- 真正的 attack vector "fake chunk path 诱导用户打开恶意 path" 的最坏后果仍仅 "在用户已有路径打开终端 / 编辑器"，**无任意命令执行**

**用户确认策略**（不引入）：
- Phase 2 不加 "确认对话框" 二次确认 path——增加摩擦超过收益（高频操作，confirm dialog 会让用户麻木 click through）
- 真要做需 `risky_path_warn` Settings 字段 + 用户首次调时引导，phase 3 评估

**安全不变量（硬约束）：**
- `open_in_terminal` 仅 cd，**不接受 shell command 参数**
- `open_in_editor` 仅打开文件 + 可选行号，**不接受任意 CLI args**
- 两个 command 的可执行文件白名单完全封闭在 D1/D2 表格中
- `searchEngine.custom.urlTemplate` SHALL 校验 scheme ∈ `{http, https}`（D3 决策更新——拒绝 `javascript:` / `file:` / `data:`）

**风险：**
- canonicalize 在 path 不存在时返回 error（正好拦住不存在路径）
- Windows UNC path `\\server\share` 的 canonicalize 可能有 network call 延迟——可接受（用户操作非 hot path）

---

### D5: error variant 体系

**候选方案：**
- A) 为每个新 IPC 定义独立 `thiserror` enum（`TerminalError` / `EditorError`）
- B) 复用 `ApiError` + 新增 `ApiErrorCode::ExternalApp` variant

**选了：** B — 复用 `ApiError`，新增一个 error code variant

**取舍：**
- 前端只需 `code` + `message` 做 toast 提示，不需 exhaustive match 每种失败类型
- A 引入独立 error type 需要在 Tauri command 层做 `Into<ApiError>` 转换，增加无益样板
- `ApiError` 已是所有 IPC command 的统一错误形态，前端 `tauriMock.ts` / error handling 层已对齐

#### 新增 variant

```rust
pub enum ApiErrorCode {
    // ... 现有 ...
    /// 外部应用交互错误（editor/terminal spawn 失败、CLI 不存在等）。
    ExternalApp,
}
```

#### 错误场景映射

| 场景 | code | message 示例 |
|---|---|---|
| path 非绝对路径 | `ValidationError` | `"path must be absolute"` |
| path 不存在（canonicalize 失败） | `NotFound` | `"path does not exist: /foo/bar"` |
| terminal CLI 不可用 | `ExternalApp` | `"terminal 'iTerm' is not installed or not found"` |
| editor CLI 不可用 | `ExternalApp` | `"editor CLI 'code' not found; install VS Code shell command or change Settings"` |
| spawn 失败（权限 / OS 拒绝） | `ExternalApp` | `"failed to launch terminal: Permission denied (os error 13)"` |
| terminal 与当前 OS 不匹配 | — | **不 error**，自动 fallback + `tracing::warn!` |

#### 构造 helper

```rust
impl ApiError {
    pub fn external_app(msg: impl Into<String>) -> Self {
        Self { code: ApiErrorCode::ExternalApp, message: msg.into() }
    }
}
```

#### 模块放置

spawn 逻辑放 `crates/cdt-api/src/ipc/external_app.rs`（新建模块），暴露：
- `pub async fn open_in_terminal(path: &str, config: &ConfigManager) -> Result<(), ApiError>`
- `pub async fn open_in_editor(path: &str, line: Option<u32>, column: Option<u32>, config: &ConfigManager) -> Result<(), ApiError>`
- `pub fn list_available_terminals() -> Vec<TerminalApp>`

Tauri command wrapper 在 `src-tauri/src/lib.rs` 内 thin 调用这些函数。

**风险：** `ApiErrorCode` 新增 variant 需同步 IPC contract test `ipc_contract.rs`（确认 serde 输出 `"external_app"`）。

---

### D6: menu-items 函数库 API 设计

**候选方案：**
- A) 单一 mega-function `buildContextMenuItems(target, ctx)` + type guard 分流
- B) 按 surface 拆独立 factory 函数，每个返回 `ContextMenuItem[]`
- C) 基于 class 的 builder pattern（`new ContextMenuBuilder().addCopy().addNavigate().build()`）

**选了：** B — 按 surface 拆独立 factory

**取舍：**
- B 让每个 surface 的 item 列表在文件内一目了然（grep 可直达），CI 改动范围局部化
- A 的 mega-function 会随 surface 增长膨胀，type guard 分叉多层嵌套降低可读性
- C 的 builder pattern 对纯同步数组构造 over-engineering；item 列表是静态配置非动态组合

**函数签名与模块结构：**

```typescript
// ui/src/lib/contextMenu/menu-items.ts

/** 共享上下文，所有 factory 函数第二参数 */
export interface MenuItemContext {
  sessionId: string;
  projectId: string;
  settings: {
    externalEditor: string;   // ExternalEditor enum IPC 值（"system" / "vs_code" / ...）
    searchEngine: SearchEngineSetting; // { type: "google" } | { type: "custom", urlTemplate: string }
    terminalApp: string;      // TerminalApp enum IPC 值（"terminal" / "i_term" / ...）
  };
  /** 调用方在 oncontextmenu 触发瞬间读 `window.getSelection()?.toString() ?? ""`
   *  传入；factory **不**自己读 DOM，保持纯函数语义并避免 jsdom / SSR 测试问题 */
  selectionText: string;
  /** 封装 IPC 调用，让 factory 产出的 item.action 自包含 */
  dispatch: {
    copyToClipboard: (text: string) => Promise<void>;
    openInEditor: (path: string, line?: number) => Promise<void>;
    openInTerminal: (cwd: string) => Promise<void>;
    revealInDir: (path: string) => Promise<void>;
    openUrl: (url: string) => Promise<void>;
  };
}

export type SearchEngineSetting =
  | { type: "google" }
  | { type: "bing" }
  | { type: "duck_duck_go" }
  | { type: "custom"; urlTemplate: string };

export function buildUserMessageItems(chunk: UserChunk, ctx: MenuItemContext): ContextMenuItem[];
export function buildAssistantMessageItems(chunk: AIChunk, ctx: MenuItemContext): ContextMenuItem[];
export function buildBashToolItems(exec: ToolExecution, ctx: MenuItemContext): ContextMenuItem[];
export function buildFileToolItems(exec: ToolExecution, ctx: MenuItemContext): ContextMenuItem[];
export function buildWorktreeChipItems(worktree: { path: string; name: string }, ctx: MenuItemContext): ContextMenuItem[];
export function buildProjectCardItems(project: { path: string; name: string }, ctx: MenuItemContext): ContextMenuItem[];
export function buildSelectionItems(selectionText: string, ctx: MenuItemContext): ContextMenuItem[];
```

**`ctx.dispatch` 设计理由：**
- factory 返回的 `ContextMenuItem.action` 是闭包，内部需调 IPC（`writeText` / `open_in_editor` / `open_in_terminal` 等）
- 把 IPC 调用封装为 `dispatch` 对象而非让 factory 直接 import api.ts：(1) 便于 vitest mock 不走真 IPC；(2) 让 menu-items.ts 保持纯函数性质（给定输入 → 确定输出），更易测试
- `dispatch.copyToClipboard` 内部调 `navigator.clipboard.writeText()` + 写入 `item.feedback`（"已复制!"600ms 反馈由 AppContextMenu 的 feedback 机制承载，factory 只设 `feedback: { label: "已复制!" }`）

**item 构造约定：**
- separator 语义遵循 D-V3 三段式：① 复制类 → separator → ② 导航类 → separator → ③ 外部类
- 仅 section 实际存在 item 时才插 separator，factory 内部 trim 首尾孤立 separator
- shortcut hint 仅对已注册全局 keyboard shortcut 的 item 附加（如 ⌘C）

**各 surface 的 items 清单（按 D-V3 三段分组）：**

| Surface | ① 复制类 | ② 导航类 | ③ 外部类 |
|---|---|---|---|
| UserMessage | 复制纯文本 / 复制为 Markdown / 复制选区（有选区时） | 复制 deeplink | — |
| AssistantMessage | 复制纯文本 / 复制为 Markdown / 复制选区（有选区时）/ 复制完整对话上下文 | 复制 deeplink | — |
| BashTool | 复制命令 / 复制输出 / 复制 stderr（有 error 时） | — | 在终端打开 cwd / 在浏览器搜索错误 |
| FileTool | 复制路径 / 复制 diff（Edit/Write） | — | 在编辑器打开（含行号）/ 在 Finder 中显示 / 在终端打开父目录 |
| WorktreeChip | 复制路径 | — | 在编辑器打开 / 在终端打开 / 在 Finder 中显示 |
| ProjectCard | 复制路径 / 复制项目名 | — | 在编辑器打开 / 在终端打开 |
| Selection | 复制 / 复制为引用 Markdown | — | 在浏览器搜索 |

**"复制选区"融合规则（D10 的决策产出）：**
- 调用方（surface 组件 / window-level handler）在 `oncontextmenu` 触发瞬间预读 `window.getSelection()?.toString() ?? ""`
- 通过 `ctx.selectionText` 传入 factory；**factory 不直接读 DOM**（保纯函数语义，避免 vitest jsdom 不稳定）
- factory 检查 `ctx.selectionText.length > 0` 时在首段首项前插入"复制选中文本"item（`shortcut: "⌘C"`），action 内 `ctx.dispatch.copyToClipboard(ctx.selectionText)`
- 这避免了用户在有选区时必须从 surface 菜单退出才能用 selection 菜单复制；同时让 factory 单测不需要 polyfill `window.getSelection`

**风险：** `ctx.dispatch` 函数签名可能随后端 IPC 增减变动。缓解：dispatch 接口与后端 D1/D2 IPC 一一对应，变动需同步改。

---

### D7: ContextMenuItem 类型扩展

**候选方案：**
- A) 不扩展，所有新需求用现有 `label` / `action` / `disabled` / `danger` / `separator` / `feedback` 六字段凑
- B) 最小扩展：加 `shortcut` / `submenu` / `kind` 三字段
- C) 完整扩展：加 `shortcut` / `submenu` / `kind` / `pathLabel` / `icon`（icon 预留但 D-V1 决策不渲染）

**选了：** B + pathLabel — 加 `shortcut` / `submenu` / `kind` / `pathLabel` 四字段

**扩展后类型定义：**

```typescript
export interface ContextMenuItem {
  // --- Phase 1 既有 ---
  separator?: boolean;
  label?: string;
  icon?: string;         // 预留（Phase 1 已有字段，D-V1 不渲染）
  disabled?: boolean;
  danger?: boolean;
  action?: () => void;
  feedback?: { label: string; durationMs?: number };

  // --- Phase 2 新增 ---
  /** 右侧快捷键 hint 文本，如 "⌘C"。仅 display，不注册真实快捷键绑定 */
  shortcut?: string;
  /** 二级菜单。有 submenu 时 action 忽略 + shortcut 不渲染（D-V4） */
  submenu?: ContextMenuItem[];
  /** 语义分类，用于 separator 智能插入逻辑（factory 内部标记，AppContextMenu 不消费） */
  kind?: "copy" | "navigate" | "external";
  /** 路径类 label 的中段截断形态（D-V6），渲染层用 short 做显示 + full 做 title tooltip */
  pathLabel?: { short: string; full: string };
}
```

**`kind` 字段设计理由：**
- 不直接参与渲染（AppContextMenu 不读 kind）
- factory 内部用于 separator 自动插入逻辑：相邻 item kind 变化时插 separator
- 让 separator 分组逻辑内聚在 factory 而非 surface 组件

**`pathLabel` 字段设计理由：**
- 中段截断（`~/Rustrove…/contextMenu.svelte.ts`）是 JS 计算产物（CSS 原生无 middle-ellipsis）
- factory 预处理路径生成 `{ short, full }`，渲染层用 `short` 做 label + `full` 做 `title`
- 有 `pathLabel` 时覆盖 `label` 渲染（AppContextMenu 优先读 `pathLabel.short`）
- 截断算法：保留首段（home 前缀 `~/`）+ 尾段（文件名 + 后缀，最多 30 字符）+ 中间 `…`；总长 ≤ 50 字符

**`submenu` 字段影响：**
- AppContextMenu 需扩展渲染逻辑（检测 `item.submenu?.length` → 渲染 `›` + submenu 弹出）
- 交互规格引用 D-V4（200ms entry / ArrowRight / 安全三角简化版）

**兼容性：** 全部新字段 optional，Phase 1 已落地的 Sidebar / Tab 右键菜单无需改动即兼容。

**风险：** `submenu` 嵌套递归深度无限。缓解：Phase 2 只用一层 submenu（编辑器选择 / 终端选择）；AppContextMenu 渲染层加 `maxDepth=2` 防御。

---

### D8: Chunk → Markdown 反序策略

**候选方案：**
- A) 从已渲染 HTML 反向（引入 turndown / 自写反序器），`innerHTML → markdown`
- B) 从 chunk 数据结构的 raw text 字段直接取（前端已持有原始 markdown）
- C) 后端新增 IPC `get_chunk_markdown(sessionId, chunkId)` 返回原始 markdown

**选了：** B — 前端直接取 raw text 字段

**取舍：**
- **UserChunk**：`content` 字段是 `string | ContentBlock[]`。字符串时经 `cleanDisplayText()` 清洗即为用户原始输入 markdown（去 XML noise 标签保留 prose）。ContentBlock[] 时取 `type==="text"` 的 `text` 字段拼接（跳过 `type==="image"` 的 ContentBlock）
- **AIChunk**：`semanticSteps` 中 `kind==="text"` 步骤的 `text` 字段就是原始 markdown（来自 assistant response 的 text block，未经 `marked` 渲染）。多个 text step 用 `\n\n` 拼接
- **Bash 工具块**：ToolExecution.input 的 `command` 字段 → `` ```bash\n$ {command}\n``` ``；output 的 text → `` ```\n{output}\n``` ``
- **File 工具块**：`file_path` → heading；Read output → `` ```{lang}\n{content}\n``` ``；Edit/Write → diff 格式

**A 被否决的理由：**
- 引入 turndown 增加 ~30 KB bundle，对仅"复制文本"功能的 ROI 不合理
- HTML→markdown 是有损转换：highlight.js 注入的 `<span class="hljs-*">` 需特殊处理；mermaid 图表无法逆向；DOMPurify sanitize 后丢失原始结构
- chunk 数据结构本身已持有 raw text，逆向渲染后 HTML 是多余绕路

**C 被否决的理由：**
- 新增 IPC = 后端改动 + contract test + IPC 白名单，仅为"复制纯文本"功能不值得
- 前端已持有全部所需数据（`get_session_detail` 返回的 chunks），无需额外 round trip
- 性能：复制是即时动作，增加 IPC round trip 让"复制"从 0ms 变 50-100ms 违背用户预期

**helper 函数设计：**

```typescript
// ui/src/lib/contextMenu/markdown.ts

/** 从 UserChunk 提取用户输入 markdown（清洗 XML noise，保留 prose 格式） */
export function userChunkToMarkdown(chunk: UserChunk): string;

/** 从 AIChunk 的 semanticSteps 提取 AI 回复 markdown（仅 text 步骤拼接） */
export function aiChunkToMarkdown(chunk: AIChunk): string;

/** 从 ToolExecution 构造工具调用 markdown（含 fenced code block） */
export function toolExecToMarkdown(exec: ToolExecution): string;

/** 提取纯文本（strip markdown formatting：# / ** / ` / fences），用于"复制纯文本" */
export function chunkToPlainText(chunk: Chunk): string;
```

**`chunkToPlainText` vs `*ToMarkdown` 分离理由：**
- "复制纯文本"：strip 所有 markdown 格式（用最小 regex 去 `#` / `**` / `` ` `` / `[text](url)` → `text`），输出用户最终阅读内容
- "复制为 markdown"：保留格式，输出可直接粘贴到 markdown 编辑器的源码
- 纯文本 strip 不引入新依赖——regex 手写（行数 < 30），不值得引 `remove-markdown` 库

**风险：**
- UserChunk `content` 为 `ContentBlock[]` 时需遍历 block 提取 text。线上数据中 user 消息极少用 ContentBlock 数组（通常是纯 string），但 image block 需跳过
- `chunkToPlainText` 的 regex strip 不完美（nested bold/italic / complex link），可接受——用户可选"复制为 markdown"获取精确格式

---

### D9: deeplink hash route 设计

**候选方案：**
- A) 引入 `svelte-spa-router` 库
- B) 自写 hash watcher（`hashchange` + regex 解析）
- C) URL search params（`?session=xxx&chunk=yyy`）

**选了：** B — 自写 hash watcher

**取舍：**
- 本仓不使用 router 库（App.svelte 由 tabStore 管理页面状态，无 route → component 映射），引入 spa-router 仅为一个 deeplink 功能 overkill
- hash route 不影响 Tauri WKWebView 的 navigate 行为（`tauri://localhost/` + `#/...` 不触发页面重载）
- search params 在 dev 模式已被 `?mock=1&fixture=...` 占用，追加 `&session=` 会与 fixture 参数混杂

**格式：**
```
#/session/<sessionId>/chunk/<chunkId>
```
- `sessionId`：与 `SessionDetail.sessionId` 一致
- `chunkId`：与 `Chunk.chunkId` 一致（基于 message uuid 稳定，不因 re-render 变化）

**实现模块：** `ui/src/lib/deeplink.ts`

```typescript
export interface DeeplinkTarget {
  sessionId: string;
  chunkId: string;
}

/** 解析当前 location.hash → DeeplinkTarget | null */
export function parseDeeplink(hash?: string): DeeplinkTarget | null;

/** 生成 deeplink hash 字符串（含 # 前缀） */
export function buildDeeplinkHash(sessionId: string, chunkId: string): string;

/** 注册 hashchange 监听 + 启动时检查；匹配时调 callback。返回 cleanup fn */
export function installDeeplinkWatcher(
  onNavigate: (target: DeeplinkTarget) => void
): () => void;
```

**跨 surface 点击行为（用户流程）：**
1. 用户在某 chunk 右键 → "复制 deeplink" → `navigator.clipboard.writeText(location.origin + location.pathname + buildDeeplinkHash(sessionId, chunkId))`
2. 粘贴到 App 内或从 dev 浏览器 URL 栏访问：
   - `installDeeplinkWatcher` 检测 `hashchange` → 解析 → `onNavigate(target)`
   - `onNavigate` 内调 `openSessionTab(sessionId, projectId, label)` 打开/聚焦 session
   - SessionDetail mount + chunks 加载后 → `scrollToChunk(chunkId)` 滚动 + 高亮
3. `scrollToChunk` 实现：`document.querySelector([data-chunk-id="${chunkId}"])?.scrollIntoView({ behavior: 'smooth', block: 'center' })` + 加 `.chunk-highlight` class → 1.5s fade-out animation

**`data-chunk-id` 属性：** SessionDetail 的 chunk 渲染循环中给每个 chunk 容器 div 加 `data-chunk-id={chunk.chunkId}`（零性能开销的静态属性）

**SessionDetail 未 mount 时的 race 处理（codex 二审 DESIGN_HOLE #11 修订）：**

- watcher 打开 session tab → SessionDetail 异步加载 chunks
- 方案：tabStore 在 tab UI state 里存 `pendingScrollChunkId?: string`
- 消费时机 SHALL 满足三条件：(a) tab 已被用户激活（成为 focused tab）；(b) SessionDetail mount 完成；(c) chunks 加载完成（`chunks.length > 0` 或显式 loaded flag）
- 三条件全满足后 SHALL 检查 `getTabUIState(tabId).pendingScrollChunkId` → 找到 DOM 节点则 scroll + 高亮 + clear；找不到（chunks 加载完但 chunkId 不存在）SHALL 弹 toast "deeplink target not found in this session" + clear
- **不设固定时间超时**——pendingScrollChunkId 绑定到 tab lifecycle：tab 关闭时随 tabUIState 一起清；用户始终未激活 tab 时保持 pending（确保用户后续切到该 tab 时仍触发）
- 用户在 tab 已激活后再次激活同 tab（来回切）SHALL **不**重复 scroll——pendingScrollChunkId 在第一次成功消费后即 clear
- 加载失败（IPC error）SHALL 弹 toast 显示 error + clear pendingScrollChunkId（避免反复重试）

**`openSessionTab` 函数依赖：** 该 export 来自 `ui/src/lib/state/tabStore.ts`（已存在的 tab 管理 API），实现需在 deeplink 模块外部已 import；若该函数 signature 不匹配，frontend-engineer 在 apply 阶段 SHALL 验证 + 必要时小幅调整 tabStore API（视为 design 阶段未尽证）

**风险：**
- 外部浏览器（非 Tauri）访问 deeplink：仅 dev `?mock=1` 可 demo；Tauri 窗口内消费。Phase 3 可注册 `cdt://` custom protocol 支持跨 app deeplink
- `chunkId` 在 session JSONL 追加写入后保持稳定（基于 message uuid），但若用户清空 JSONL 重来则失效——可接受（极端 edge case）

---

### D10: window-level contextmenu 接管 vs surface-level use:contextMenu 优先级

**候选方案：**
- A) window-level handler 统一拦截所有 contextmenu 事件，内部 if/else 判断 target 类型（替代 Phase 1 surface-level）
- B) Phase 1 surface-level `use:contextMenu` 不变（`stopPropagation`），window-level 只处理"漏网"事件
- C) 三层级联：surface-level → window-level selection menu → 全局兜底 `preventDefault`

**选了：** C — 三层级联

**设计理由：**
- Phase 1 已有 `use:contextMenu` action 在 handler 内调 `e.stopPropagation()`——surface 挂了 action 的元素事件不冒泡到 window
- Phase 2 新增"文本选区菜单"本质是 window-level handler，检测 `selection.toString().length > 0` 弹选区菜单
- Phase 1 的 `installGlobalContextMenuFallback()` 已在 bubble 阶段检查 `e.defaultPrevented`

**三层级联机制：**

```
                           contextmenu event
                                │
                    ┌───────────▼───────────┐
                    │ Layer 1: surface-level │  use:contextMenu action
                    │ stopPropagation + show │  → 弹 surface-specific 菜单
                    │   (最高优先级)         │
                    └───────────┬───────────┘
                                │ (仅当无 surface action 时冒泡)
                    ┌───────────▼───────────┐
                    │ Layer 2: window-level  │  selection menu handler
                    │ 检测 selection > 0     │  → 弹选区菜单 + preventDefault
                    └───────────┬───────────┘
                                │ (仅当无选区时继续)
                    ┌───────────▼───────────┐
                    │ Layer 3: 全局兜底      │  installGlobalContextMenuFallback
                    │ 检测 defaultPrevented  │  → 仅 preventDefault（不弹菜单）
                    │ + input/textarea 放行  │
                    └───────────────────────┘
```

**Layer 2 注册策略：**
- bubble 阶段 `window.addEventListener('contextmenu', selectionHandler, false)`
- 在 `installGlobalContextMenuFallback()` **之前** 注册（同阶段先注册先执行）
- handler 内先检查 `e.defaultPrevented`（若被 Layer 1 或 Layer 3 之前的 handler 处理则 skip）
- 再检查 `window.getSelection()?.toString().length > 0`
- 满足则 `e.preventDefault()` + 弹选区菜单 + 不 `stopPropagation`（让后续 handler 可检测 `defaultPrevented`）

**main.ts 注册顺序（硬约束）：**
```typescript
// main.ts 启动序列
installSelectionContextMenu();          // Layer 2
installGlobalContextMenuFallback();     // Layer 3（Phase 1 已有）
```

**冲突场景分析：**

| 场景 | Layer 1 | Layer 2 | Layer 3 | 最终行为 |
|------|---------|---------|---------|---------|
| 右键 AI 消息（已挂 use:contextMenu） | 拦截 + stopProp | 不触发（事件不冒泡） | 不触发 | AI 消息菜单 |
| 先选区再右键 AI 消息 | 拦截 + stopProp | 不触发 | 不触发 | AI 消息菜单（surface 优先） |
| 先选区再右键空白区 | 跳过（无 action） | 检测有选区 → 弹选区菜单 | defaultPrevented → skip | 选区菜单 |
| 右键空白区无选区 | 跳过 | 检测无选区 → 跳过 | preventDefault | 无菜单 |
| 右键 input 框 | 跳过 | 检测 target 是 input → 跳过 | 检测 input → 放行 | 浏览器原生菜单 |

**"先选区再右键 AI 消息"的融合策略：**
- surface-level 菜单优先（用户意图是对"这个 AI 消息"操作）
- 但 `buildAssistantMessageItems` / `buildUserMessageItems` 等 surface factory SHALL 检测 `window.getSelection()?.toString().length > 0`
- 有选区时在首段首项前动态插入"复制选中文本"item（`shortcut: "⌘C"`）
- 这融合了 surface 菜单与 selection 菜单的核心能力，避免用户被迫先清除选区再右键

**`installSelectionContextMenu` 模块位置：** `ui/src/lib/contextMenu/selectionMenu.ts`（新建）

```typescript
export function installSelectionContextMenu(): void;
```

内部使用 `openMenu`（从 `contextMenu.svelte.ts` 导出或重构为共享 util）渲染 `AppContextMenu`。

**Layer 2 的 input/textarea 放行：**
- 与 Layer 3 相同逻辑：`target.closest('input, textarea, [contenteditable], [data-allow-native-context]')` → 跳过
- 保证在 input 内选区右键仍走浏览器原生菜单（粘贴 / 拼写检查）

**HMR 幂等：** 与 Phase 1 `installGlobalContextMenuFallback` 同策略——window sentinel flag `__cdtSelectionMenuInstalled` + `import.meta.hot.dispose` 双保险

**风险：**
- `window.getSelection()?.toString()` 在某些 DOM 结构（shadow DOM / iframe）下可能返回空字符串。缓解：本仓无 shadow DOM 也无 iframe
- Layer 2 的 `openMenu` 调用需要 `trigger` 元素参数（focus 还回用）；selection menu 的 trigger 选 `document.activeElement ?? document.body`

---

## Q1–Q8 决策 mapping

PR #269 catch-up 列出的 8 个开放问题，倾向答案已落到对应 D / D-V 决策。reviewer 用此表逐项追溯：

| # | 问题 | 倾向答案 | 落在哪 |
|---|---|---|---|
| Q1 | chunk deeplink 形态 | hash route（in-app）`#/session/<id>/chunk/<id>` | **D9** deeplink hash route 设计 |
| Q2 | 浏览器搜索默认引擎 + 可配置 | 默认 Google + 可配置（含 Custom URL 模板）| **D3** `SearchEngine` enum + `general.searchEngine` 字段 |
| Q3 | 在终端运行：执行命令 vs 只 cd | **只 cd**（避免 RCE）| **D1** open_in_terminal 仅 cd + **D4** 安全不变量 "open_in_terminal 不接受 shell command 参数" |
| Q4 | 编辑器跳行号 | 支持 | **D2** `code --goto path:line:col` + IPC `line/column: Option<u32>` |
| Q5 | 复制为 Markdown：后端序列化 vs 前端反向 | 前端反向 | **D8** Chunk → Markdown 反序（取 raw text 字段） |
| Q6 | 折叠 chunk 右键菜单项 | **Phase 2 不做**，留 followup | 见下方 `## Followups` 段 |
| Q7 | danger item 视觉首落地 | **Phase 2 不引入**（issue 列表无 destructive） | **D-V** State Coverage 表注 "Phase 2 暂不引入，预留" |
| Q8 | tauriMock stub 新 IPC | mock 返 `Promise.resolve()`，与 `plugin:opener\|open_path` 同模式 | **Verification Plan** 伪覆盖清单 #1/#2（防 mock 反映 = 端到端通过）|

---

## Visual Contract

### Surface Decision

6 个 surface 的入口选择论证：

| # | Surface | 入口机制 | 论证 |
|---|---------|---------|------|
| 1 | 用户消息 chunk | `use:contextMenu` on `.user-bubble` | Phase 1 已落地的 action 模式直接复用；bubble 是独立视觉单元，右键语义清晰 |
| 2 | AI 消息 chunk | `use:contextMenu` on `.ai-msg-container` | 同上；AI 消息容器边界明确，不与子元素（code block、tool item）右键冲突——子元素优先级由 stopPropagation 保证 |
| 3 | Bash 工具块 | `use:contextMenu` on `BashToolViewer` root | 工具块已有 BaseItem disclosure 结构，右键 target 自然落在整个 viewer 上 |
| 4 | Read/Edit/Write 工具块 | `use:contextMenu` on 各 ToolViewer root | 同 Bash；三种 viewer 共享 `buildFileToolItems` factory |
| 5 | Worktree chip + 项目卡 | `use:contextMenu` on `.worktree-chip` / `.project-card` | 独立可点击视觉单元，`PRODUCT.md::Design Principles §2` "采用桌面工具常见的...command palette" 类比——chip/card 右键是桌面工具标准交互 |
| 6 | 任意文本选区 | **window-level** `contextmenu` 接管（非 surface-level action） | 此 surface 无固定 DOM 宿主——选区可能跨多个 DOM 节点；若在每个可能被选中的元素上单独挂 `use:contextMenu` 会导致：(a) 需遍历全部文本容器加 action 维护爆炸、(b) 选区跨容器时无法确定 trigger 归属。window-level handler 检测 `selection.toString().length > 0` → 弹出选区专属菜单（含"复制"、"在浏览器搜索"）；**不与** Phase 1 surface-level `use:contextMenu` 冲突——Phase 1 action 已 `stopPropagation`，selection menu 只在无 surface-specific menu 时 fallback 触发 |

**与 `PRODUCT.md::Anti-references` 对齐**：菜单保持纯文本 + separator 分组，不加装饰性渐变/icon/glass。"信息密度高但不嘈杂"体现为：每 surface 最多 8-10 items，通过 separator 三段分组降低扫描成本。

**与 `The App Owns the Right-Click Rule` 对齐**：所有 6 surface + window-level fallback 构成完整覆盖链，确保应用内任何位置右键都不漏系统菜单。

### Visual Layer

Phase 2 新增视觉元素引用的 Named Rule：

| 元素 | 引用 Named Rule | 说明 |
|------|----------------|------|
| submenu 浮层 | `DESIGN.md::The Border Before Shadow Rule` | submenu 是真浮层（脱离 flow），使用与父菜单同档 shadow 合规 |
| submenu 浮层 | `DESIGN.md::The No Decorative Glass Rule` | 禁 backdrop-filter |
| shortcut hint 文字 | `DESIGN.md::The Machine Information Rule` | 修饰键符号（⌘⇧⌥）用 mono |
| shortcut hint 颜色 | `DESIGN.md::The Status Owns the Color Rule` | hint 用 neutral muted 色，不引入新彩色 |
| 路径类 label | `DESIGN.md::The Machine Information Rule` | 文件路径 mono 显示 |
| item hover/active | `DESIGN.md::The Persistent Selection Is Quiet Rule` 边界说明 | 菜单 item 焦点是 transient ≤ 2s 决策窗，keyboard outline 合规 |
| separator 分组 | `DESIGN.md::The Tool Density Rule` | 不靠字号/字重分组，用 1px hairline + spacing 即可 |
| submenu trigger `›` | `DESIGN.md::The Floating Is Affordance, Not Decoration Rule` | chevron 是结构性 affordance（"有子菜单"），不是装饰 |

### State Coverage

| 状态 | 适用范围 | 实现位置 |
|------|---------|---------|
| **default** | 所有 menu item | `.cm-item` 基态：transparent bg, `--color-text`, cursor pointer |
| **hover** | 非 disabled item 鼠标悬浮 | `.cm-item:hover:not(.cm-item-disabled)` → `--tool-item-hover-bg` |
| **keyboard active** | 键盘 ↑↓ 当前项 | `.cm-item-active` → hover bg + `outline 2px solid rgba(59,130,246,0.15)` |
| **disabled** | 不可用 item（如无选区时"复制选区"） | `.cm-item-disabled` → opacity 0.45, cursor not-allowed, aria-disabled |
| **danger** | destructive item（Phase 2 暂不引入，预留） | `.cm-item-danger` → `--color-danger` text + 淡红 hover bg |
| **feedback** | 复制成功短暂反馈 | feedbackIndex 匹配 → 替换 label 为 feedback text → 600ms 后 onClose |
| **submenu trigger hover** | 有子菜单的 item 悬浮 200ms | 同 hover + 触发 submenu 弹出；trigger item 保持 active bg 直到 submenu 关闭 |
| **submenu open** | submenu 展开中 | 父 item 保持 `.cm-item-active` bg 锁定 + submenu portal 渲染 |
| **loading** | N/A | 菜单是瞬时 UI（items 同步计算），无异步加载态 |
| **empty** | items 为空数组 | 不渲染菜单（`use:contextMenu` provider 返回 `[]` 时 fallback 到全局兜底 → 无菜单无系统菜单） |
| **error** | N/A | 菜单渲染无异步失败路径；外部 action（IPC 调用）的 error 在 action 回调内处理，不反映到菜单状态 |

### DESIGN.md delta plan

Phase 2 完成后，archive 前跑 `/impeccable extract` 提进 `DESIGN.md` 的新增 token / 组件 / Named Rule：

**新增 token（`app.css` 同步浅/深/system 三主题）：**
- `--cm-shortcut-color`: shortcut hint 文字色（alias `--color-text-muted`，独立 token 便于未来微调）
- `--cm-shortcut-font`: shortcut hint 字体（alias `var(--font-mono)` / `11px`）
- `--cm-max-width`: 菜单 max-width（`320px`）

**`DESIGN.md::§5 Context menu` 段扩展：**
- 补充 submenu 交互规格（entry delay / keyboard / positioning / safe triangle）
- 补充 shortcut hint 视觉规格（位置 / 颜色 / 字体）
- 补充 `max-width` 约束 + 路径中段截断策略

**候选 Named Rule（视实现验证后决定是否提升为 Named Rule）：**
- "The Submenu Follows Parent Rule" — submenu SHALL 使用与父菜单完全相同的 bg / border / radius / shadow，不做层级递进
- "The Shortcut Hint Is Whisper Rule" — shortcut hint 使用 muted/mono 最低视觉权重，永远不与 label 抢主注意力

**`ContextMenuItem` 类型扩展（不影响 DESIGN.md，属 spec delta）：**
- 新增 `shortcut?: string` 字段
- 新增 `submenu?: ContextMenuItem[]` 字段
- 新增 `pathLabel?: string` 字段（中段截断后的路径 label）

---


## Verification Plan

### 测试金字塔分层

| Surface | vitest+mockIPC | Playwright e2e | Tauri dev smoke | e2e-http-verify |
|---|---|---|---|---|
| 用户消息 chunk | `buildUserMessageItems` 返回正确 items 数组 + dispatch 回调被调用 + Chunk→MD 反序纯函数 | 右键 `.user-bubble` → `[role="menu"]` 可见 + 包含"复制为 Markdown" + 点击 → clipboard 写入 | N/A（无平台分流 IPC） | `?http=1` 打开真 session → 右键消息 → 验菜单渲染 + "复制 deeplink" hash route 正确 |
| AI 消息 chunk | `buildAssistantMessageItems` 返回 items + stopPropagation 不穿透子工具块 + Chunk→MD 含代码块 | 右键 `.ai-msg-container` → 菜单可见 + 分组正确 + Esc/外点关闭 + 子元素右键不冒泡 | N/A | 同上；AI 消息含 tool result 时验 items 仅属 AI 层 |
| Bash 工具块 | `buildBashToolItems` 含"复制命令"/"复制输出"/"在终端打开" + disabled 态（无 cwd 时） | 右键 `BashToolViewer` root → 菜单含预期 items + 键盘 ↑↓ 导航 | **真测 `open_in_terminal`**：点"在终端打开" → macOS Terminal.app cd 到 cwd | `?http=1` 验 `open_in_terminal` HTTP route 200（或列入 `unsupportedBrowserCommands` 时验 warn） |
| Read/Edit/Write 工具块 | `buildFileToolItems` 含"复制路径"/"在编辑器打开"/"在 Finder 中显示" + `pathLabel` 截断 | 右键 ToolViewer root → 菜单含路径 items + shortcut hint 右对齐 + max-width 截断验 tooltip | **真测 `open_in_editor`**：点"在 VS Code 打开" → VS Code 跳到行号 | `open_in_editor` HTTP route 200 + Settings `external_editor` 字段消费 |
| Worktree chip + 项目卡 | `buildWorktreeChipItems` 含"在终端打开"/"在 Finder 中显示"/"复制路径" + 项目卡含项目级操作 | 右键 `.worktree-chip` / `.project-card` → 各自菜单可见 | **真测**：chip "在终端打开" → Terminal.app cd 到 worktree 目录 | `?http=1` 验 chip 右键菜单渲染（spawn 类 IPC side-effect 视 HTTP route 暴露情况） |
| 任意文本选区 | window-level handler 检测 `selection.toString().length > 0` → 弹选区菜单 + 无选区不弹 + surface `stopPropagation` 不穿透 | 选中文本 → 右键 → 菜单含"复制"/"在浏览器搜索" + 跨 DOM 节点选区 | N/A（"搜索"调 `plugin:opener\|open_url`） | `?http=1` 验 "搜索" → `window.open()` URL 拼接 |

### 跨平台 smoke 计划

#### macOS（本机主跑 ✓）

- `open_in_terminal` → `open -a Terminal <path>` 验 Terminal.app 弹出 cd 到目标目录
- `open_in_editor` → `code --goto path:line:col` 验 VS Code 打开 + 跳行号
- `reveal_item_in_dir` → `open -R path` 验 Finder 聚焦
- submenu 视觉：真鼠标 hover 200ms 延迟 + safe triangle 感知
- 二指触摸板 contextmenu 与合成 `dispatchEvent('contextmenu')` 一致性

#### Windows（⚠️ 本机无法覆盖——需 runner / VM / 用户手测）

- `open_in_terminal` 三级 fallback：
  - `wt.exe -d <path>`：Windows Terminal 已装 → 弹窗口 cd 到目标
  - `powershell.exe -NoExit -Command "Set-Location -LiteralPath '<path>'"` fallback
  - `cmd.exe /K cd /d <path>` 最终兜底
  - **edge case**：Windows Terminal 未装（Win 10 LTSC）→ 验 fallback 到 PowerShell 不 panic
  - 路径含空格 / 中文 / `&` 字符验 quoting
- `open_in_editor` → `code.cmd` PATH 注册 vs 未注册时 error 提示
- Tauri capabilities scope 在 Windows 校验行为

#### Linux（⚠️ 本机无法覆盖——需 runner / VM / 用户手测）

- `open_in_terminal` fallback：
  - `x-terminal-emulator --working-directory=<path>`（Debian 系）
  - headless server 无 DE → graceful error 不 panic
  - Wayland vs X11 差异
- `open_in_editor` → Flatpak/Snap 安装的 VS Code 路径差异
- `xdg-open` 非阻塞验证

#### 未覆盖平台处理策略

| 策略层 | macOS ✓ | Win/Linux CI | Win/Linux 手测 |
|--------|---------|-------------|---------------|
| Rust IPC contract test | ✓ | ✓ GitHub Actions matrix | N/A |
| vitest 单测（menu-items / markdown） | ✓ 无平台依赖 | ✓ CI matrix | N/A |
| Playwright e2e | ✓ | ✓ CI headless Chromium | N/A |
| **`open_in_terminal` 真 spawn** | ✓ Tauri dev smoke | ❌ CI 无 GUI | ⚠️ 人工 |
| **`open_in_editor` 真 spawn** | ✓ Tauri dev smoke | ❌ CI 无 GUI | ⚠️ 人工 |

**标注**：PR 描述 SHALL 写明 "Windows/Linux spawn smoke 需 manual QA"。

### 伪覆盖识别清单

| # | 伪覆盖 pattern | 本质 | 防御手段 |
|---|---|---|---|
| 1 | mockIPC `open_in_terminal` → `Promise.resolve()` | mock void 不验 (a) 真 spawn 是否执行 (b) 路径 quoting (c) 目标 app 存在 | Tauri dev smoke 真跑 + Rust 集成测试验 `Command::new` 参数拼接 |
| 2 | mockIPC `open_in_editor` → `Promise.resolve()` | 同上；特别是 `:line` 拼接 + 路径含空格 | Tauri dev smoke + Rust unit test 验 argv |
| 3 | `plugin:opener\|open_url` mock 走 `window.open` | vitest `window.open` 是 noop stub；Playwright popups 被拦 | Tauri dev smoke 验真打开浏览器；e2e-http-verify 验 URL 拼接 |
| 4 | Playwright `dispatchEvent('contextmenu')` ≠ 桌面端真右键 | macOS WKWebView 二指触摸板 / Ctrl+Click 的 event 属性可能与 JS 合成事件不同 | Tauri dev smoke 物理触摸板验证；Playwright 测试显式设 `{ button: 2 }` |
| 5 | capabilities allow-list 静态 JSON ≠ 运行时拦截 | 静态查不能验遗漏命令是否被 Tauri 报 PermissionDenied | Tauri dev smoke 真调每个新 IPC → console 无 PermissionDenied；contract test 验字段名 |
| 6 | `buildFileToolItems` 路径截断单测 pass ≠ CSS 截断渲染正确 | vitest 验字符串截断，不验 `text-overflow: ellipsis` 真渲染 | Playwright e2e 验 label 未溢出 320px；Tauri dev smoke 人眼验暗色 |
| 7 | window-level selection menu 单测 pass ≠ 与 surface action 互不冲突 | jsdom 不支持真 Selection API，无法验 stopPropagation 真拦 | Playwright e2e：`.user-bubble` 上选中 → 右键 → 验弹出 surface 菜单非 selection 菜单 |
| 8 | submenu hover delay 单测 pass ≠ 真 mouse hysteresis | `vi.advanceTimersByTime(200)` 不验鼠标轨迹几何 | Playwright `page.mouse.move(x, y, { steps: 5 })` 对角穿越；Tauri dev smoke 手动 hover |
| 9 | Settings `external_editor` round-trip 过 ≠ 真消费 | `config_round_trip_reflection` 验存取一致，不验 IPC 是否读了更新后的值 | e2e-http-verify：修改 Settings → 调 `open_in_editor` → 验后端用更新后 editor |

### 验证手段 → tasks.md 标注约定

lead 拆 task 时按以下 tag 标注验证手段：

```
[QA: unit only]           — 纯函数 / store / 类型，vitest 够
[QA: Playwright]          — 浏览器真渲染 + 键鼠 + 跨组件交互
[QA: e2e-http-verify]     — IPC 字段被前端消费的路径
[QA: Tauri smoke]         — 平台分流 spawn / Tauri-only API
[QA: Tauri smoke + xplat] — 平台分流 + 需标注 Win/Linux 手测
[QA: contract test]       — 新 IPC 字段名 / 序列化格式
```

单条 task 可组合多个 tag，如 `[QA: unit only + Playwright + Tauri smoke + xplat]`。

### QA 执行节奏

| 阶段 | QA 动作 |
|------|---------|
| apply 开始 | 确认 IPC contract test 覆盖新字段（后端投递后验） |
| menu-items 函数库完成 | vitest 单测 review + 伪覆盖 #1/#2 防御确认 |
| Playwright e2e 完成 | `just test-e2e` 验 5 surface 全绿 + 伪覆盖 #4/#7/#8 |
| 前端全部 surface 接入 | e2e-http-verify `?http=1` 真数据 smoke |
| 后端 `open_in_terminal` / `open_in_editor` 完成 | Tauri dev smoke 真 spawn（macOS）+ 标注 Win/Linux 待验 |
| PR push 前 | 全量验证报告投递 lead |

## Followups（Phase 3+ 候选）

| # | 项 | 理由 |
|---|---|---|
| F1 | 折叠 chunk 右键菜单项（Q6） | 已有 chunk 内 toggle 入口承担同样能力，右键加项冗余；Phase 3 视用户反馈再评估 |
| F2 | `cdt://` custom protocol 跨 app deeplink | D9 仅支持 in-app hash route；Phase 3 注册 OS-level scheme 让外部链接（IM / 邮件）跳到具体 chunk |
| F3 | submenu 完整 "safe triangle" hysteresis | D-V4 Phase 2 用简化版（纯 200ms entry delay），Phase 3 polish 实现鼠标轨迹几何判定 |
| F4 | "在 Finder/Explorer 中显示" reveal 操作 | menu-items 函数库 ctx.dispatch 已含 `revealInDir`，但底层 IPC 走现有 `plugin:opener\|reveal_item_in_dir`（如已有）或新加 IPC（视检查结果） |
| F5 | 新 token / Named Rule 沉淀进 `DESIGN.md` | archive 前跑 `/impeccable extract` 提进 `DESIGN.md`（D-V `## DESIGN.md delta plan` 子段已列清单） |
| F6 | iOS / mobile 触摸右键映射 | 桌面端独占；Phase 2 不考虑 |

