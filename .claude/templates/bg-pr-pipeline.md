# Background PR 流水线模板（通用）

> 用法：把 `{{...}}` 占位符填空后传给 `just bg-pr <name> <this-file>`。**不要省略硬约束段**——`.claude/rules/bg-task-dispatch.md` 的 6 个踩坑全部源于"图省事砍 prompt"。

---

你是被主 session 派来跑独立 PR 的 background claude。自治从实施到 push → codex 二审 → wait-ci 全绿 → archive（如适用），**不 merge**（留主 session 用户决定）。

仓库根：`/Users/zhaohejie/RustroverProjects/Project/claude-devtools-rs`

## 总体规则

按 `CLAUDE.md` + `.claude/rules/{rust,perf,opsx-apply-cadence,codex-usage,bg-task-dispatch}.md`：

- 不在 main 直接编辑；先 EnterWorktree + `git checkout -b {{branch-prefix}}/{{slug}}`
- 全程 `cargo clippy --workspace --all-targets -- -D warnings` 汇总校验
- 每轮 tool 后自检"发下批工具 or 发最终文本"
- **禁止** `git push --force` / `gh pr merge` / `rm -rf` / `--no-verify` 等 destructive shared state 操作
- 命令用绝对路径或 `--manifest-path` 避免 cwd 漂移到 src-tauri/

## 准备

1. 用 EnterWorktree 工具：`name="{{worktree-name}}"`
2. 在 worktree 内：
   ```bash
   git checkout -b {{branch-prefix}}/{{slug}}
   ```
3. 跑前置验证（如有 bench）：
   ```bash
   {{pre-validation-command}}
   ```

## 任务描述

### 改动 1: {{标题}}
- 位置：`{{file:line}}`
- 现状：{{现状}}
- 修法：{{修法}}
- 影响：{{量级 / 行为变化}}

### 改动 N: {{...}}
...

## 是否走 openspec？

- **行为契约级改动**（IPC 字段 / 后端算法 / 状态判定 / 数据流语义 / Tauri command 协议） → **走** `Skill({ skill: "opsx:propose", args: "{{slug}}" })` 先 propose（design.md 含 D1/D2/D3，spec delta 含 SHALL/MUST），再 apply，最后 archive
- **纯视觉对齐 / 单点样式 / 实现优化** 不动语义 → 直接 PR

填到下方：本 PR **{{走 / 不走}}** openspec，slug `{{slug}}`。

## 业务推进段（按 .claude/rules/opsx-apply-cadence.md 节拍）

每改一项：
1. Edit 源文件
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo fmt --all`
4. `cargo test -p {{affected-crate}}`（涉及多 crate 跑 `--workspace`）
5. （如改 `ui/`）`npm run check --prefix ui`
6. （如走 openspec）`openspec validate {{slug}} --strict`

完成后跑后置验证：
```bash
{{post-validation-command}}
```

## 性能相关 PR（如适用）

涉及 `cdt-discover/` / `cdt-api/src/ipc/` / `cdt-analyze/` / hot loop file I/O / 引入子进程 spawn 的 PR SHALL 在 PR 描述里加四维 Perf impact 段（见 `.claude/rules/perf.md`）：

```markdown
## Perf impact
- 关键路径：[xxx]
- wall：基线 a ms → 本 PR b ms（±%）
- user：c.cc s → d.dd s（±%）
- sys：e.ee s → f.ff s（±%）
- max RSS：N MB → M MB（±%）
- user/real：0.xx → 0.yy
- 数据：`/usr/bin/time -lp <bench-cmd>` 输出
```

非性能 PR 跳过本段。

## 发布尾段（**不 merge**）

```bash
git add -A
git commit -m "{{commit-msg}}"
git push -u origin {{branch-prefix}}/{{slug}}

gh pr create --title "{{pr-title}}" --body "$(cat <<'EOF'
{{pr-body}}
EOF
)"
```

记下 PR_NUM。

## codex 二审

按 `.claude/rules/codex-usage.md` 调 `Agent({ subagent_type: "codex:codex-rescue", prompt: ... })`：

```
背景：{{branch}} 分支，{{改动主题}}

改动范围：{{文件列表}}

我已经验证的事：
- preflight 全绿
- {{其他验证：测试 / bench / round-trip 等}}

我希望你重点查的问题：
1. {{怀疑点 1}}
2. {{怀疑点 2}}
3. {{怀疑点 3}}

约束：只报 bug；每个问题给文件 + 行号 + 现状 + 为什么 + 修法；中文 500 字以内
仓库根：{{worktree-absolute-path}}
分支：{{branch}}
```

报 bug 就修 → push → 用 `SendMessage` 接续同一 codex agent 验证修法。可多轮。

## wait-ci

```bash
gh pr checks {{PR_NUM}} --watch
```

红了 `gh run view --log-failed` 自查自修 + push + 再 wait。

## archive（如走 openspec，**原子操作**）

CI 全绿 + codex 通过：
```bash
openspec archive {{slug}} -y
git add -A
git commit -m "chore(opsx): archive {{slug}}"
git push
```

archive commit 推完**再次** wait-ci 全绿。**不要**先单独 commit "勾 N.4 tasks" 再 archive（CI 拦截窗口，详见 `.claude/rules/opsx-apply-cadence.md` "循环依赖如何避免"段）。

## 完成标准

CI 全绿 + codex 通过后，**不 merge**。最终回复（含 `result:` 触发主 session 收尾）：

```
result: PR {{name}} 完成 — PR #{{NUM}}，CI 全绿（含 archive commit，如适用），codex 通过 N 轮，等待用户 merge
- {{核心数据 / 收益 / 影响摘要}}
```

## 注意事项

- 不确定的问题先用 grep 调研，调研不出再写 `needs input:` 求助
- 走 openspec 时 design.md 必含 D1/D2/D3 决策记录（候选方案对比 + 风险）
- 如本 PR 与并行其他 PR 改同文件，rebase main 时如有冲突解决之
