## Context

keyboard-shortcuts capability 的 spec 已定义 `mod` token 作为跨平台主修饰键抽象（macOS → `Meta`、Windows / Linux → `Control`），并由 `normalizeBinding(binding: string)` 在 dispatcher 入口展开。`registerShortcut` 的 `defaultBinding` 已统一使用 `mod+x` 字面量（如 `defaults.ts` 中 `command-palette.toggle = "mod+K"`）。

但**用户自定义 override 的录入与持久化路径**没沿用 mod token：

- `KeyRecorderInput.svelte::handleKeyDown` 把 `KeyboardEvent` 透传给 `normalize(event)`，产出平台特化字面量 `meta+k`（mac）/ `ctrl+k`（win/linux）后调 `onCommit(binding)`，最终由 KeyboardShortcutsPanel 的 Save 写入 cdt-config
- `bootstrapOverrides` 通过 `getConfig()` 读出存量字面量直接喂给 `mergeOverrides`，不做任何归一化

后果：
1. mac 录的 `meta+shift+p` 持久化后同步到 Windows，`normalize(event)` 在 non-mac 不识 `metaKey`，dispatcher 永不命中；但 `formatShortcut("meta+shift+p")` 在 Windows 渲染为 `Win+Shift+P` 误导用户
2. Windows 用户按 `Win+B` 录键，`normalize(event)` 忽略 `metaKey` 后产出字面量 `b`，`KeyRecorderInput` 静默 commit 单字符 binding——既数据污染也 UX 断裂

约束：dispatcher 行为零回归是硬约束。`normalize(event)` 与 `normalizeBinding(binding)` 已在生产环境跑，spec 既有 D2"non-mac 不识 metaKey"是防 Win 键误触发的关键决策，本次改动 SHALL NOT 触动 dispatcher 入口。

## Goals / Non-Goals

**Goals:**
- 录键产出与 cdt-config 持久化的 binding 字面量统一为跨平台 `mod+x` 表达，与 `defaults.ts` defaultBinding 表达对齐
- 存量 `meta+x` / `ctrl+x` 字面量在 bootstrap 阶段透明迁移为 `mod+x`，无需用户操作、无需 schema 版本号
- `KeyRecorderInput` 在 non-mac + `event.metaKey === true` 时给出明确错误反馈（不静默 commit 错误 binding）
- 新视觉状态（Win 键守卫 warning）复用 `DESIGN.md` 既有 token，不引入新 token
- dispatcher 行为零回归

**Non-Goals:**
- `tab.close` advisoryHints Windows + Tauri WebView2 提示（涉及 src-tauri accelerator 配置，独立 followup）
- 删除 `WIN_TEXT.meta = "Win"`（防御性 fallback，正常路径不触发；删了反而暴露未知二级路径）
- `formatShortcut` 在 Windows 遇到 `meta` 显示"(不可用)"标注（mod 归一后 binding 不再含字面 `meta`，自然规避）
- 修改 dispatcher 的 `normalize(event)` 平台分支语义
- cdt-config schema 版本号 / migration version 字段（字符串迁移幂等且无破坏性，无需版本管控）

## Decisions

### D1：录键产出 mod 字面量（不是产出 meta/ctrl 后由消费者展开）

**选择**：`KeyRecorderInput::handleKeyDown` 在调 `onCommit(binding)` 前把当前平台主修饰键反写为 `mod`：mac 把 `meta+x` 字面量替换为 `mod+x`，win/linux 把 `ctrl+x` 字面量替换为 `mod+x`。新工厂函数 `recordBindingFromEvent(event: KeyboardEvent): string | null` 在 `platform.ts` 内封装"normalize(event) → 平台主修饰键反写"两步。

**为什么不让消费者持有原始 `meta+x` 由 Save handler 在写 cdt-config 前展开**：那样需要在每个 commit / Save / format 边界都重新归一一次，而录键 widget 是这条数据的唯一源头。**source 即归一**让下游全程只见 mod 字面量，符合"single source of truth"。

**alt+ctrl+x 在 win 平台的处理**：只把"主修饰键"`ctrl` 反写为 `mod`，得 `alt+mod+x`（`normalizeBinding` 展开为 `alt+ctrl+x` 命中正确）。`alt+x`（无主修饰键）不变（`alt+x` 不是常见快捷键，但若用户录入也合法）。

**meta + ctrl 同时按下的边缘情况**：
- mac：`meta+ctrl+x` 反写主修饰键 `meta` → `mod`，得 `ctrl+mod+x`，dispatcher 展开 `ctrl+meta+x` 命中
- win/linux：理论上不会发生（`metaKey` 已被 `normalize(event)` 在 non-mac 忽略），无需特化

### D2：bootstrap 字面量迁移走 token-level 替换 + 幂等

**选择**：`mergeOverrides(defaults, overrides)` 在合并前对每个 override binding 调 `normalizeBindingToMod(binding)`：

```ts
function normalizeBindingToMod(binding: string): string {
  // token-level 算法（不是字符串前缀替换）：
  // 1. binding.split("+") 得 token 数组
  // 2. 若数组中**已含** "mod" token：保留所有 token 顺序，移除主键之外位置的 "meta" / "ctrl" token（防御异常字面量 "meta+mod+x"），不重排
  // 3. 否则按"主修饰键优先级 meta > ctrl"在 modifier 位置（除主键外）找替换目标：
  //    a. 优先找第一个 "meta" token 替换为 "mod"
  //    b. 若数组无 "meta"，再找第一个 "ctrl" token 替换为 "mod"
  // 4. 不再调 normalize 重排——dispatcher 入口的 normalizeBinding(binding) 会在 register 时统一展开 + sort，本函数仅负责 token 替换
  // 5. 不含 meta / ctrl 主修饰键的 binding 原样返回（alt+x / shift+x / 单字符 / F1 / Numpad 系列）
}
```

**为什么 token-level 而非字符串前缀替换**：用户手工编辑 cdt-config 可能产出非 sorted 字面量（如 `shift+meta+p`），按字母排序后 `meta` 不在头部，前缀 `meta+` 替换会漏掉。token-level 按 `split("+") → 找主修饰键 token → 替换`鲁棒覆盖任意位置。

**为什么 meta 优先于 ctrl**：mac 平台 `Cmd+Ctrl+X` event 经 `normalize` 输出 `ctrl+meta+x`（按内部排序 ctrl < meta < shift），同时含 meta 和 ctrl。该 binding 的用户意图是"Command + Control 加 X"，mac 主修饰键是 meta、辅助是 ctrl，应反写 meta 保留 ctrl 得 `ctrl+mod+x`。win 平台 `normalize` 不会产出 meta（spec 既有决策"non-mac 不识 metaKey"），所以 binding 中含 meta 的肯定来自 mac 录入；按"优先替换 meta"规则：含 meta 时一定优先替换 meta（mac 视角主修饰键）；不含 meta 时才替换 ctrl（win 视角主修饰键）。该规则在 mac / win / 跨平台同步三种场景下都给出正确归一。

**为什么不在迁移函数内重排 sort**：`normalizeBinding` 在 dispatcher 入口与 `findConflict` 内部都会调，是 spec 既有规约的"sort 入口"。本迁移函数只负责"token 替换不重排"，sort 留给 dispatcher 流水线，避免 sort 算法在两处实现漂移。

**异常字面量 `meta+mod+x` 的处理**：用户手工编辑 cdt-config 可能产出该字面量。归一化为 `mod+x`（移除多余 `meta`），与"用户意图是跨平台 mod"的预期一致。spec 用一条 Scenario 显式覆盖。

**为什么不写 schema 版本号 + 一次性 migration**：(1) 字符串归一化幂等（已是 `mod+x` 不变，是 `meta+x`/`ctrl+x` 转 `mod+x`、是 `meta+mod+x` 这种异常归一为 `mod+x`）；(2) 替换无信息丢失（不会把"用户故意只在 mac 用的快捷键"误解为"跨平台"——dispatcher 行为本来就是按平台展开 `mod`，与原 `meta+x` / `ctrl+x` 在本平台等价）；(3) 不引入 cdt-config schema 字段污染。每次启动都跑一遍开销可忽略（字符串 split + 数组遍历，N≤几十条 override）。

**保存路径无需特化**：录键已产出 `mod+x`（D1），Save handler 直接把 pendingOverrides 的 `mod+x` 写入 cdt-config，无需在 Save 前再归一。

**触发时机**：`mergeOverrides` 在两个调用点跑——`bootstrapOverrides` 启动时、`registry.update(id, newBinding)` 运行期单次更新（虽然录键已产出 mod，但 `update` 是 public API，可能被未来调用方传 `meta+x`，归一化作为护栏）。

**跨设备同步语境澄清**：本 change 不引入"自动 cdt-config 跨设备同步"机制——用户在 mac 录键后通过手动复制 / iCloud / 第三方同步工具把 cdt-config 文件搬到 Windows 设备，是 issue 描述的"同步到 Windows"语义。本 PR 修的是"该文件在异平台启动时能正确归一化"，不增删任何同步路径。

### D3：Win 键守卫——recording 态的 self-emit warning，与 conflict prop 隔离

**选择**：`KeyRecorderInput::handleKeyDown` 在 `event.metaKey === true && !isMac()` 时**先于** `recordBindingFromEvent(event)` 守卫：不调 `onCommit`、不调 `containerEl?.blur()`，set 内部 `winKeyWarning = $state(true)`。aria-live 区域文本切换为"Windows 不支持 Win 键作为修饰键，按目标组合键重新录入"。

**warning 子态清除时机**（明确状态机）：
- 用户按下不含 `metaKey` 的下一次 keydown 时 reset 为 false——无论该次按键是否触发 commit（如仅按 Shift 单键 normalize 返回 null，warning 也清除）
- 用户按 Esc 退出 recording → `stopRecording()` 中显式 reset
- 用户按 Tab / 失焦让 recorder blur → `stopRecording()` 路径同样 reset
- recording 态内连续按 `Win+B` → `Win+X` 多次都触发 warning，但任何一次按下不含 metaKey 的键都 reset

**为什么用内部 state 而非新加 prop**：父组件 `KeyboardShortcutsPanel` 不需要知道这个错误（它不影响 conflict 检测、不影响 Save 启用态）；warning 是 widget 内部 transient state，commit 成功后自动清除。

**视觉**：复用 `DESIGN.md::The Conflict Is Warning Not Error Rule` 的 `--surface-conflict-bg` / `--border-conflict` token——Win 键守卫与 conflict 同属"用户输入未通过校验，可解决"语义，按规则"用 warning 暖色而非 error red"。`KeyRecorderInput` 已有 `class:conflict` 选择器引用这两个 token，新增 `class:warning` 选择器走相同 token + 同一 transient hint 行（不新增 DOM 结构）。

**视觉等同与文本可区分性**：warning 与 conflict 视觉相同（同 token），靠 hint 区文本区分（"Windows 不支持 Win 键..." vs "冲突：与「X」重叠"）——按 `DESIGN.md::The Conflict Is Warning Not Error Rule` 的预期行为，"两种 actionable warning 共用暖色 token"是规则鼓励的复用，不引入新 token。

**与 conflict prop 的优先级**：父组件传入的 `conflict` prop（findConflict 命中）与 widget 内部 `winKeyWarning` 互斥——Win 键守卫直接 return 不进 normalize 路径，不会触发 conflict 检测；conflict prop 由父组件在 commit 后才计算，时序上 Win 键守卫先发生。hint 文本优先级：`winKeyWarning > conflict > recording > idle`。

### D4：Space mac 显示 ␣（独立 polish，与 mod 主线无耦合）

**选择**：`formatMainKey(key, platform)` 对 `Space` 在 mac 返回 `"␣"`（U+2423 OPEN BOX），其他平台保持 `"Space"`。对齐 macOS HIG 推荐符号（空格在系统中以 `␣` / `space` 文本表示）。

**为什么纳入本 change**：单点改动 + 与 `formatMainKey` 同函数 + 同 PR codex 二审一遍即可，独立开 PR 收益不抵成本。

### D-V1：Win 键守卫态视觉契约

- **Surface**：守卫态发生在 Settings → Keyboard Shortcuts → KeyRecorderInput recording 中，无新 surface 引入
- **视觉规约**：复用 `DESIGN.md::The Conflict Is Warning Not Error Rule` 的 `--surface-conflict-bg` / `--border-conflict` token；与既有 conflict 态视觉等同（暖色 border + bg），由 `class:warning`（widget 内部 self-emit）控制
- **a11y**：sets `aria-describedby` 指向同一 hint 区域（已有），文本随 `winKeyWarning` 切换。当前 `aria-live="polite"` 落在 recorder 容器 div 上（line 134），hint span 自身没 aria-live；本 PR SHALL 把 `aria-live="polite"` 显式声明到 hint `<span>`（移到该处而非保留 recorder div 上的副本，避免 SR 双宣告 noise——focus / pressed / class 变化时 recorder div 会触发宣告，但仅 hint 文本切换才是用户关心的语义）。spec 与 task 2.7 同步显式说明该改动
- **交互**：守卫触发时 widget 保持 recording 态（不 blur）；用户按下不含 `metaKey` 的下一组组合键即正常 commit，warning 自动消除
- **DESIGN.md delta**：无新增 token / 新 Named Rule；`The Recorder Idle State Rule` 已说明"`KeyRecorderInput.svelte` 是首个引用案例"，本 PR 是首次扩展该 widget 的 warning 子态，确认在既有规约内

## Risks / Trade-offs

- [字符串归一化误处理任意位置主修饰键] → `normalizeBindingToMod` 走 token-level 算法（`split("+")` 后按 token 分类，不依赖 token 位置或前缀）；vitest 单测覆盖 `meta+x` / `ctrl+x` / `alt+ctrl+x`（中间位置）/ `shift+meta+p`（用户手编非 sorted）/ `meta+mod+x`（异常字面量幂等到 `mod+x`）/ `mod+x`（幂等）/ `alt+x`（无主修饰键不变）/ `F1`（无修饰键不变）至少 8 种字面量
- [bootstrap 每次启动都跑归一化的开销] → N≤几十条 override，纯字符串操作 O(N)，启动路径预估 < 1ms，spec 性能预算无影响
- [用户在 mac 录的 `mod+x` 同步到只有 `Ctrl` 的旧版本会失效] → 旧版本不识 `mod` 字面量，但本仓所有线上版本都已使用 `mod` token（`defaults.ts` 早已用），无版本前向兼容包袱
- [Win 键守卫的 false positive：用户按 `Ctrl+Win+X` 本意是 `Ctrl+X` 误触] → 守卫直接 return + warning，用户重录即可；与"静默吃掉 Win 键产出错误 binding"相比，明确反馈是更优 UX
- [warning 与 conflict 视觉相同导致用户误以为是冲突] → hint 区域文本明确写"Windows 不支持 Win 键作为修饰键"，与冲突文本"冲突：与「X」重叠"语义可区分；ergonomic UX 上"两种 warning 共用暖色"是 `DESIGN.md::The Conflict Is Warning Not Error Rule` 的预期行为

- [Cmd+Ctrl+X 在 mac 平台 sort 后 token 顺序导致 D1 反写出歧义] → mac `event.metaKey + event.ctrlKey + KeyX` 经 `normalize` sort 输出 `ctrl+meta+x`（按字母 `c < m`），D1 反写主修饰键 `meta` → `mod` 得 `ctrl+mod+x`；dispatcher 入口的 `normalizeBinding("ctrl+mod+x")` 在 mac 展开为 `ctrl+meta+x`（mod → meta，再 sort 一次得 `ctrl+meta+x`），与原 binding 等价。**mac 平台仅 `meta` 是主修饰键反写目标**——即使按下了 `ctrlKey`，也仅替换 `meta` token，`ctrl` 保留为辅助修饰键。spec 与 vitest 单测显式覆盖该 case
