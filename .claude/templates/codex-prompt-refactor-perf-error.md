# codex 重构影响面 / perf 根因 / error 边界 prompt 模板

三个触发点（#6 #7 #8）共用一个简短模板——它们的 prompt 结构相同：给 diff + 问特定维度。

## #6 重构影响面

```
改动：[列出 rename/move/split 的文件]
调用方：[列出 grep 到的 caller 文件 + 行号]

请查：哪些调用方的隐含假设（返回值排序、初始化顺序、副作用依赖、错误语义）被这次移动打破？
编译通过不代表行为正确。每个问题给：caller 文件 + 行号 + 假设 + 为什么会断。
中文，≤ 400 字。
```

## #7 Perf 回归根因

```
回归数据：
- before: wall [X]ms, user [Y]s, sys [Z]s, RSS [N]MB, user/real=[R]
- after:  wall [X']ms, user [Y']s, sys [Z']s, RSS [N']MB, user/real=[R']

diff：[贴精简 git diff]

请定位：哪行改动导致哪个指标变化？提出验证实验（revert 哪几行 re-bench 确认）。
每个归因给：文件 + 行号 + 机制解释。
中文，≤ 400 字。
```

## #8 Error 边界完备性

```
新增/变更的 error variant：[列出]
传播链变化：[哪些函数的 ? 路径变了]
Tauri command boundary：[哪些 IPC command 会暴露这些 error]

请查：哪些路径把新 variant 传到前端时变成 opaque `__TAURI_ERROR__`？
每个问题给：路径（fn A → fn B → command C）+ 为什么前端收不到有意义的错误信息 + 修法。
中文，≤ 400 字。
```
