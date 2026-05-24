<!--
PR template — 关联 issue 时务必在 body 写 Closes/Fixes/Resolves #N（英文关键字），
否则 merge 后 issue 不会自动关闭。
GitHub 仅识别 close/closes/closed、fix/fixes/fixed、resolve/resolves/resolved，
中文「修 #N」/「修复 #N」/标题里的 (#N) 均不触发；标题里写 (closes #N) 也算。
详见 https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/linking-a-pull-request-to-an-issue
-->

## Summary

<!-- 改了什么 + 为什么。1-3 句话；细节展开放下面的 Why / What / Decisions。 -->

## Test plan

- [ ] `just preflight`（fmt + clippy + nextest + svelte-check + vitest + spec-validate）
- [ ] CI 全绿
- [ ] codex 二审通过（豁免：bump version / 纯 docs / typo / CI 配置微调，跳过须写明理由）

<!--
关联 issue：解开下方注释并填 issue 号。多个 issue 各占一行（Closes #1\nCloses #2）。
本 PR 不关联任何 issue 时直接整段删掉。

Closes #
-->
