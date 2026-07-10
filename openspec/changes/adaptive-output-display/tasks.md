# adaptive-output-display 实施任务

> 范围：纯前端展示层。工具输出会话内搜索 / 超大 output 分段加载 / 前端 output cache byte cap 均**不在本 change**，见 `## 8. Followup issue`。

## 1. 自适应输出框架（共享基础）

- [x] 1.1 定义规模判定纯函数：字节按 UTF-8 长度（对齐 `outputBytes`）、行按换行符（末尾单换行不计空行）、`>=` 即升档；限高档 80 行/16 KiB，超大档 1000 行/256 KiB；返回档位（inline / bounded / oversized）
- [x] 1.2 定义工具输出分档状态机（codex C1/C2）：信号优先级 已加载真实内容 > `outputBytes` > 未知；裁剪空值不判 0；未知 fetch-first 用稳定占位（限高档高度）；加载后校正只改内层不改外层 viewport 几何
- [x] 1.3 为 1.1/1.2 写 Vitest 单测：三档边界（恰好 80/16384）、极长单行按字节升档、多字节字节度量、omitted+outputBytes 缺失 fetch-first、裁剪空值不判短、预估短档→实际超大档外层几何不变（24 用例通过）
- [x] 1.4 实现信息气味 header（总行数·总字节数·"预览"）与省略接缝（省略量=总−首尾实渲量），中性色 + mono metadata（`AdaptiveOutputFrame.svelte`）
- [x] 1.5 引入响应式限高 block-size（`clamp(12rem, 42dvh, 30rem)`）与省略接缝间距/边界（颜色 alias 现有 neutral）

## 2. 内部滚动 a11y 与滚动稳定

- [x] 2.1 限高内部滚动 viewport 沿任一轴（横/纵）实际溢出时才 `tabindex="0"` + 可访问名（含来源/规模）；未溢出不加该 viewport 的 Tab 停靠点；header 动作控件独立键盘可达（codex W11）
- [x] 2.2 focus-visible 落 viewport 用现有焦点环；保留边界滚动链（不加 `overscroll-behavior: contain`）；键盘滚动走浏览器原生
- [x] 2.3 竖向滚动区加 `scrollbar-gutter: stable`；`OutputBlock` 重写后移除旧 gutter 豁免注释（ReadToolViewer 豁免注释随 4.3 处理）
- [ ] 2.4 限高 viewport 作为 lazy markdown 节点的**外层稳定容器**（codex W6）：min-height 占位在内层、`max-block-size`+滚动在外层；确认占位清除只改内层不改外层几何，符合 `session-display::长会话滚动高度保持稳定`（随 3.x prose 接入落地）

## 3. AI prose 输出路径接入（核心痛点，仅两档不切片）

- [ ] 3.1 `SessionDetail.svelte` 的 output / user_message prose 接入自适应框架：**完整内联 / 限高预览两档，不做 top/tail 切片**（限高下完整内容留 DOM 保 Cmd+F 全文命中）；限高用 CSS block-size 不改 DOM 结构
- [ ] 3.2 AIChunk 末尾 lastOutput prose 接入两档限高
- [ ] 3.3 嵌套 ExecutionTrace（SubagentCard/WorkflowCard）内 prose 同受限高约束（codex W10：该路径同步 renderMarkdown 不经顶层 lazy observer，须显式覆盖）
- [ ] 3.4 验证搜索 hydrate（`ui-search` flushAll）后限高仍生效、长 prose 唯一关键词在中段仍命中（Playwright）

## 4. 工具查看器接入（三档含 top/tail）

- [ ] 4.1 内容源矩阵（codex C4）：Read/Bash/Default 按 output、Write 按 input.content、Edit/diff 按 old/new、error 附 errorMessage
- [x] 4.2 `OutputBlock.svelte`（Bash/Default/Edit-result）接入三档 + 常驻复制全文，替代 hover-only overlay
- [ ] 4.3 ReadToolViewer / WriteToolViewer 接入三档，与既有轻量高亮降级叠加
- [ ] 4.4 DiffViewer（`edit-diff-view`）接入三档限高 + 信息气味（diff 行不做重高亮的既有约束保留）
- [x] 4.5 超大档 top/tail 切片预算（codex C5）：每侧上限（400 行/128 KiB）、重叠规避（总行 ≤ 两侧和则退限高预览）、Unicode 码点+行边界切分、省略量精确；**仅行导向纯文本/代码/diff，markdown 不切片**（codex W7）（`sliceHeadTail` + 单测）

## 5. 复制全文常驻可发现

- [ ] 5.1 自适应框架 header 用常驻"复制全文"，复制完整原文；各输出面全文来源落表（prose→全文/Read·Bash·Default→output/Write→input.content/Edit·diff→差异）；更新 `copy-to-clipboard` 相关组件与测试（`CopyButton` 已扩展 label/disabled；OutputBlock 已接入，其余面随 3.x/4.3/4.4）
- [ ] 5.2 完整原文未就绪（懒加载中/失败/Missing/空）时复制入口禁用 + 原因标签，SHALL NOT 降级复制可见切片；不引入新失败反馈（沿用既有静默降级，codex W9）

## 6. 测试与视觉验收

- [ ] 6.1 Vitest：分级判定 / 复制全文指向原文 / 信息气味文案（对应 spec Scenario）
- [ ] 6.2 Playwright：长输出限高滚动、键盘进入内部滚动、超大 top/tail + 省略接缝、搜索 hydrate 后限高保持、滚动位置稳定
- [ ] 6.3 真实长输出 fixture 浏览器视觉验收：短 / 中长 / 超大三档 × 浅 / 深 / 窄 pane 各截图自查（无逐字折行 / 列对齐 / 文案统一 / 对比度达标）
- [ ] 6.4 阈值体感校准：按 6.3 结果必要时调整阈值并同步 spec NFR 数字

## 7. 设计契约沉淀

- [ ] 7.1 `/impeccable extract`：把 Adaptive Output Frame / Output Omission Seam 及候选 token 提进 `DESIGN.md`；更新 `DESIGN.md::Code, diff, and output` 章节
- [ ] 7.2 按 design `DESIGN.md delta plan` 提取门槛核对（三类 viewer 实装 + 三主题验收为证据）

## 8. Followup issue（本 change 前置产物）

- [x] 8.1 开 GitHub issue 记录 deferred 工作：工具输出会话内 Cmd+F 搜索（`search_tool_outputs` IPC）+ 超大 output 分段加载 / 完整中段查看 + 前端 output cache byte cap 化 + HTTP/Tauri detail omission 对齐；引用本 change design `## 显式排除与 followup` → #599

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex + pr-review-toolkit 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
