## MODIFIED Requirements

### Requirement: Metadata 占位字段视觉渐显

骨架行 SHALL 用条件 CSS class `.metadata-pending` 标识占位状态，class 上 SHALL 应用**静态** opacity 占位样式（不含 `infinite` 动画 / `background-position` 等 paint-only 周期重绘）；元数据 patch 到达后 SHALL 移除 class，触发 CSS `transition: opacity 150ms ease-out` 让真值 fade-in。

为避免 metadata 字段（`title` / `messageCount` / `isOngoing` / `gitBranch`）从骨架占位（`null` / `0` / `false`）到真值的瞬变带来视觉断层，骨架态用静态 opacity（如 `0.55`）+ 静态背景（如 `linear-gradient` 占位渐变）让"未加载"在视觉上与真值有层次差，但**不**通过周期动画提示"加载中"——遵循 `PRODUCT.md::Design Principle 5`「实时但不闪烁，避免 loading 中间态打断阅读」与 `DESIGN.md::The One Live Signal Rule` 边界条款「Skeleton placeholder 必须**静态** opacity 占位，**禁用** shimmer」。

实现 SHALL 满足：

- 每条 session 渲染时 SHALL 通过 `class:metadata-pending={!session.title && session.messageCount === 0 && !session.isOngoing}` 判定（与既有占位回退路径同条件）
- `.metadata-pending` 的 CSS SHALL **不**含 `animation` / `@keyframes` 任何 `infinite` 或周期性 `background-position` / `background-color` / `opacity` 抖动；`transform` / `opacity` 的一次性短动画（≤ 250 ms）允许，但不在本 Requirement 范围内
- `transition` SHALL 用 CSS 而**非** Svelte `transition:fade`——metadata patch 是字段 mutate 不重建 DOM 节点，Svelte transition 指令绑定 mount/unmount 不触发
- 渐显时长 SHALL 在 `100 ms ≤ X ≤ 200 ms` 区间（取 `150 ms` 作为默认值）；过短等同瞬变无渐显感，过长让用户感到"卡顿等待"
- 骨架占位视觉 SHALL NOT 依赖 metadata 请求等待时长（"已请求 N ms"）—— 占位视觉由占位条件本身（骨架字段 = `null` / `0` / `false`）决定，与到达时间阈值无关；具体实现选型（是否需要 `requestedAt` 跟踪用于非视觉用途如 telemetry）不在本视觉契约范围内

#### Scenario: 骨架行渲染时显示静态占位

- **WHEN** Sidebar 渲染一条骨架 session（`title=null`，`messageCount=0`，`isOngoing=false`）
- **THEN** 该行 SHALL 携带 `.metadata-pending` class 应用静态 opacity 占位样式
- **AND** 该行的 `.session-title-text` / `.session-meta` 元素 `getComputedStyle().animation` SHALL 为 `none` 或等价空值
- **AND** title 区显示既有占位回退（**完整 sessionId**，由 CSS `text-overflow: ellipsis` 自然截断；与 `Requirement: 会话项展示::Scenario: 无标题的会话` 一致，**禁止**在 JS 侧再手动 `slice(0, 8) + "…"`）

#### Scenario: Metadata patch 到达后字段渐显

- **WHEN** `session-metadata-update` listener 收到 sessionId 为 `S` 的更新，更新该 session 的 `title` 为 `"My Session"`
- **THEN** 该行 SHALL 在 patch 同帧移除 `.metadata-pending` class
- **AND** title 文本 SHALL 通过 CSS `transition: opacity 150ms ease-out` 从骨架占位的 `opacity: 0.55` 渐升到正常的 `opacity: 1`（不是 `0 → 1`——骨架态本身就用 `0.55` 半透明而非完全透明，避免内容彻底消失再重绘的视觉断层）
- **AND** 整个过程中 SHALL 不出现 shimmer / 周期重绘 / `background-position` 平移等动画

#### Scenario: Metadata 长时间未到达仍保持静态

- **WHEN** 某条骨架 session 的 metadata 在 `> 1500 ms` 后仍未通过 `session-metadata-update` 推送到达
- **THEN** 该行 SHALL 仍保持与 `< 1500 ms` 时**完全一致**的静态 opacity 占位，**不**升级为任何形式的 shimmer / 周期动画 / "加载更慢了" 视觉提示
- **AND** `.metadata-pending` class 的 CSS 样式 SHALL 不引用任何与等待时长相关的 CSS 自定义属性 / `:hover` 之外的状态选择器
