## 1. 前端数据层：响应式快照 + 归一化候选 + 去重

- [x] 1.1 CommandPalette 改为响应式读 `projectDataStore` 的 `repositoryGroups`（`$derived`），不再 onMount 一次性复制局部 `projects`（D3 / 修 stale-ghost）
- [x] 1.2 全局 sessionId 候选构建：遍历 `repositoryGroups → worktrees[] → sessions[]`，产出 **normalized row** `{ sessionId, projectId, groupId, projectName, worktreeName, gitBranch, worktreeMostRecent, title?, hits? }`；字段来源明确：groupId=`group.id`、projectId=`worktree.id`、projectName=`worktree.name`、worktreeName=`worktree.name`、gitBranch=`worktree.gitBranch`、worktreeMostRecent=`worktree.mostRecentSession`（均内存现成、零 I/O）；title? 仅当 sessionId 命中**组件已加载的 `sessions` 本地数组**时取，否则留空（D1 / D2 —— 前端无 per-session title/mtime）
- [x] 1.3 去重 identity = `sessionId`（单一 key）；同 id 跨 worktree 取确定性优先级（优先 main/repo-root worktree，否则遍历首条），**不**依赖不存在的 per-session mtime（D4）

## 2. 前端搜索逻辑：合并 / 门槛 / 排序 / debounce

- [x] 2.1 `filteredSessions`：query 长度 ≥ 4 时启用全局 id 子串匹配；< 4 维持现有组内行为（D5）
- [x] 2.2 合并 A（全局 id）+ B（组内正文 `searchGroupSessions`，先转 normalized row）：identity = `sessionId` 合并（与 1.3 同一 key）；双路命中保留 B 的 `projectId/groupId/hits`（B 来自选中组、归属权威），合并 A 的 title?/projectName 兜底（D7）
- [x] 2.3 确定性排序：worktreeMostRecent 倒序 → 项目名 + sessionId 稳定兜底；排序后截断到 `MAX_SESSIONS`；超限显示"仅显示前 N 条"提示（D6 / 无静默 cap）
- [x] 2.4 仅 debounce 后端 `searchGroupSessions` IPC；前端 filter/sort/selectedIndex 走 raw query 即时更新（D8b 反转 D8，修 type-then-Enter 选旧项）；candidate 列表随快照构建一次
- [x] 2.6 B 路 `searchResults` 配 `searchResultsQuery` 标记，`contentMatchRows` 仅当与当前 query 一致才采用；后端 reject seq 守卫清空（D7b，修 stale-as-fresh）
- [x] 2.7 切组 select-effect 起始同步清 `sessions` + catch seq 守卫（D4b，消除身份错配）
- [x] 2.8 短查询提示独立渲染不门控 totalResults；空态用 `getProjectDataError()` 区分加载失败（D5b/D3b）
- [x] 2.5 短查询（1–3 字符）未选项目时显示"输入 ≥4 个字符按 Session ID 全局定位"提示（D5 / 空态）

## 3. 前端打开归属 + UI 行展示

- [x] 3.1 `openSession` 使用结果行自身 `projectId/groupId` 打开，杜绝跨项目归属错误（D7 / codex R3）
- [x] 3.2 会话结果行展示项目名 + title（有则显示）/ sessionId 前缀兜底；空结果 / 截断状态文案（D-V1）
- [x] 3.3 无 title 时回显**完整** sessionId（不截断）+ `matchSegments` 高亮命中子串（`<mark class="cp-match">`，用 `--highlight-bg`）；更新 title-fallback / sort 测试断言为完整 id + 高亮（D2c，用户反馈）

## 4. 测试

- [x] 4.1 vitest 单测：全局 id 匹配跨项目命中 / 去重 / 最小长度门槛 / 确定性排序+截断提示 / 打开用自身 projectId+groupId（mockIPC）
- [x] 4.2 vitest 单测：stale 快照修复——store 刷新后已"打开"面板的会话区同步（新增可见、删除消失）
- [x] 4.3 vitest 单测：A/B 双路合并 winner/tie-break——同 session 不同 project/worktree、含 hits 时保留 B 版本、打开用合并 row 自身归属（D7 / codex T2）
- [x] 4.4 vitest 单测：title 正向场景——命中已加载会话显示 title 且断言**未**调 `listGroupSessions`/`getSessionSummariesByIds`（codex S1）
- [x] 4.5 active-context 边界单测/e2e mock：未连接 host 的 group 不出现，文案不宣称所有 host（D9 / codex T3）
- [x] 4.6 Playwright user story：粘一个其他项目的 sessionId → 跨项目定位 → Enter 打开正确会话
- [x] 4.7 确认无新增后端 IPC、无 metadata scan 触发（grep / 断言不调补 title 路径）
- [x] 4.8 vitest：确定性排序顺序 / 截断到 20 + 提示 / title 未加载兜底(id 前缀+项目名,不调 IPC) / 短查询提示有 actions 时仍显示（codex+test-analyzer 二审补）
- [x] 4.9 e2e 强化：用 getPaneLayout 断言打开的 tab 归属 mock-rich-ts 的 sess-ts-* 会话（验跨项目归属，非仅面板关闭）

## 5. 验证 + 流程

- [x] 5.1 `pnpm --dir ui run check` + `just test-ui-unit` + 受影响 e2e 通过
- [x] 5.2 `openspec validate cmdk-global-session-locate --strict` 通过
- [x] 5.3 CHANGELOG `## [Unreleased]` → `### Added` 追加用户可感知条目（英文）
- [ ] 5.4 e2e-http-verify 真数据验证"粘 id 跨项目定位"（用户可感知 + HTTP transport 路径）

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex + pr-review-toolkit 二审通过（如发现 bug：修 → push → 回 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
