---
name: mark-tasks-done
description: 在 openspec/changes/<name>/tasks.md 里把指定 section 范围内的 `- [ ]` 批量改成 `- [x]`，跳过 "下次 port 同步位点" / "future notes" 这类备忘节。用于 /opsx:apply 结束时的批量勾选，防止误勾选 future-port 备忘。
disable-model-invocation: true
---

# mark-tasks-done

用户明确要求时（`/mark-tasks-done <change> [--sections N-M]` 或 "把 port-foo 的任务勾上"）才调用。不允许模型在 apply 流程里自动触发——勾选是有副作用的修改动作。

## 输入

1. **change name**（必需）：openspec 变更名，形如 `port-tool-execution-linking`。
2. **sections**（可选）：section 区间，默认 `1-10`，即第 1 到第 10 章。备忘节（通常是最后一节，例如 "下次 port 同步位点" / "future notes"）永远被排除。

## 工作步骤

1. **定位文件**：`openspec/changes/<name>/tasks.md`。若不存在则报错退出。
2. **解析 section 头**：识别所有 `^## N\. ` 形式的标题，按数字排序。记录每段 section 的行号区间。
3. **识别备忘节**：
   - 标题文本匹配正则 `(下次|future|notes|同步位点|备忘|carry[- ]over)` 之一的 section
   - 或者紧跟 section 头后面有一行 blockquote `^> ` 以 "以下" 或 "these" 开头
   → 这些 section **不要**勾选。
4. **计算要勾选的行**：用户传入 `--sections N-M`（默认 `1-<最大活动节号>`），跳过备忘节；对范围内的每条 `- [ ] ` 行替换为 `- [x] `。
5. **使用 Edit 工具修改文件**，不要用 Bash 的 `sed`/`perl`——模型友好的精准替换减少误伤。
6. **输出摘要**：
   ```
   marked N tasks done in openspec/changes/<name>/tasks.md
     sections: 1..M (excluded: <备忘节编号与标题>)
     preserved: K unchecked items in 备忘节
   ```

## 硬性约束

- **只操作 `- [ ] `**（精确前缀 2 空格 + 括号 + 空格），不要匹配 `-[ ]` 或 `* [ ]` 等变体。
- 备忘节里的任何 checkbox **必须**保持原状，即使它是 `- [ ]`。
- 不跑 `cargo`、不跑 `openspec`——这是纯文本编辑。
- 修改前先 Read 一次 tasks.md 确认格式，然后用 Edit 工具单行或多行替换。
- 如果 section 头用错了格式（例如 `##1.` 没空格），报错提示用户先修格式，不要尝试纠错。

## 示例

用户：`/mark-tasks-done port-foo`

→ 读 `openspec/changes/port-foo/tasks.md` → 识别 section 1..10 为活动、section 11 "下次 port 同步位点" 为备忘 → 对 1..10 里的 `- [ ] ` 改成 `- [x] ` → 输出：
```
marked 55 tasks done in openspec/changes/port-foo/tasks.md
  sections: 1..10 (excluded: 11 "下次 port 同步位点")
  preserved: 3 unchecked items in 备忘节
```
