## 1. file-watching 主 spec 重写（spec delta 落地）

- [x] 1.1 把 9 个 Requirement 的英文 Scenario 标题改简体中文（24 处，按 brief 第 2.2 节清单）
- [x] 1.2 重写 Purpose 段为用户价值视角（删 broadcast / debounce / pipeline / 通道 / in-process 等实现机制术语）
- [x] 1.3 NFR 数字独立成 Requirement「事件投递时延、远端 polling 频率与停止时延」（合并 100ms debounce / 30s catch-up / 3s polling / 1s 停止四块数字契约）
- [x] 1.4 删原 FR Body 内嵌的 NFR 数字描述行（仅在新 NFR Requirement 内保留）
- [x] 1.5 `openspec validate spec-overhaul-file-watching-pilot --strict` 通过

## 2. spec-guide-reviewer subagent 沉淀

- [x] 2.1 写 `.claude/agents/spec-guide-reviewer.md`：frontmatter（name / description / tools: Read,Grep,Glob,Bash）+ body（角色 / 输入 / 检查清单 / 输出格式）
- [x] 2.2 引用 `openspec/SPEC_GUIDE.md` 的 4 层骨架 / 该写 vs 不该写对照表 / reviewer checklist 6 条
- [x] 2.3 输出格式参考 `windows-compat-reviewer` 的分级 finding（hard / warn / info）+ 不 hard-fail

## 3. push-events 决策草案落 design.md

- [x] 3.1 D-1：push-events cap 范围（候选 a/b/c + 推荐 b 的 4 条理由）
- [x] 3.2 D-2：现有 6 引用 spec 替换策略（PR 2 落地表格）
- [x] 3.3 D-3：Purpose 段直 edit 主 spec 的架构例外说明（OpenSpec spec delta 不解析 Purpose）
- [x] 3.4 D-4：archive 顺序坑预防论证（PR 1 单 cap 改动无 archive 互覆盖风险）

## 4. 验收 grep + baseline 同步

- [x] 4.1 brief 第 5 节 6 条验收 grep 全过（Scenario 全中文 / Purpose 不含实现术语 / subagent 文件存在 / D 决策 ≥ 3 / validate / NFR Requirement ≥ 1）；本 PR 内 grep 1/6 跑 delta 而非主 spec（主 spec post-archive 才生效），其余跑主 spec
- [x] 4.2 `bash scripts/check-spec-purity.sh --baseline > scripts/spec-purity-baseline.txt` 同 commit 刷新；当前加 `change/spec-overhaul-file-watching-pilot/file-watching 12`；archive 时 N.4 二次刷新（删 change 行 + 更新 spec/file-watching 行）
- [x] 4.3 `git diff scripts/spec-purity-baseline.txt` 验证 file-watching 行被合理更新

## N. 发布

- [ ] N.1 push 分支 + 开 PR（用 brief B 模板，含 Refs #303 PR 1 / Closes #304）
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过 + spec-guide-reviewer 0 hard finding（首次实战；如发现 prompt bug 同 PR 修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
