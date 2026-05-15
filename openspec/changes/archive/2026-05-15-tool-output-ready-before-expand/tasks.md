## 1. ui (`ui/src/`)

- [x] 1.1 在 `ui/src/lib/toolHelpers.ts` 新增并 export `viewerUsesOutput(exec): boolean` —— Edit 与 Write（非 isError）返回 `false`，其余路径（Read / Bash / DefaultToolViewer 含 Write isError 与 Read isError）返回 `true`（D1b：抽到共享 helper 以让 vitest 覆盖）
- [x] 1.1b 在 `ui/src/lib/toolHelpers.ts` 新增并 export `shouldPrefetchOnChunkExpand(exec): boolean` —— 仅 Read（非 isError 且 `outputOmitted=true`）返回 `true`；让 spec "展开 AIChunk 不主动 prefetch Bash 与 Default" SHALL NOT 契约靠 vitest 守护（codex 二审 Bug 2）
- [x] 1.2 在 `ui/src/components/ExecutionTrace.svelte::toggle` `import { viewerUsesOutput }`，把 `if (exec && isReadTool(exec) && !isOutputReady(exec))` 替换为 `if (exec && viewerUsesOutput(exec) && !isOutputReady(exec))`，移除后面 `if (exec && !isReadTool(exec)) { void ensureToolOutput(exec); }` 兜底（已被前面 await 覆盖；Edit/Write 不需要 output）
- [x] 1.3 在 `ui/src/routes/SessionDetail.svelte` `import { viewerUsesOutput, shouldPrefetchOnChunkExpand }`，把 `toggle` 中 `isReadTool(exec)` gate 替换为 `viewerUsesOutput(exec)`，移除原 `if (exec && !isReadTool(exec)) { void ensureToolOutput(exec); }` 兜底；`prefetchReadOutputs` 把 `isReadTool && outputOmitted` 替换为 `shouldPrefetchOnChunkExpand(exec)`
- [x] 1.4 保持 `prefetchReadOutputs` 仅 prefetch Read 工具，不扩展过滤条件（design D2）

## 2. tests (`ui/src/lib/__tests__/`)

- [x] 2.1 vitest 覆盖 `viewerUsesOutput(Bash) === true`、`viewerUsesOutput(Read) === true`，间接保证 toggle 在 Bash / Read 上走 await 分支
- [x] 2.2 vitest 覆盖 `viewerUsesOutput(Grep) === true`、`viewerUsesOutput(WebFetch) === true`（DefaultToolViewer 路径）
- [x] 2.3 vitest 覆盖 `viewerUsesOutput(Edit) === false`、`viewerUsesOutput(Write) === false`、`Edit/isError === false`，间接保证 toggle 不为这两类 await
- [x] 2.4 vitest 覆盖 `viewerUsesOutput(Write isError) === true`、`viewerUsesOutput(Read isError) === true`（这两类走 DefaultToolViewer 渲染错误详情，仍需要 output）
- [x] 2.5 vitest 覆盖 `shouldPrefetchOnChunkExpand` —— Read+outputOmitted=true 命中；Read isError / Read outputOmitted=false / Bash / Grep / WebFetch / Write / Edit 全不命中（守护 codex 二审 Bug 2 的 SHALL NOT 契约）

## 3. validation

- [x] 3.1 `npm run check --prefix ui`（svelte-check）
- [x] 3.2 `just test-ui-unit`（vitest）
- [x] 3.3 `cargo test -p cdt-api --test ipc_contract`（IPC 协议未改，回归确认）
- [x] 3.4 `openspec validate tool-output-ready-before-expand --strict`
