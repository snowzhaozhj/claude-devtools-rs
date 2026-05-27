# Proposal: cleanup-purity-ipc-data-api

## Problem

`ipc-data-api` spec 有 57 处 purity 违规（mod-path 18 / src-path 14 / commit 5 / metric 4 / impl-flag 2 / lib/framework 14），是全仓最高。这些实现细节（内部模块路径、源码文件路径、commit/PR 引用、实测数值、回滚开关 const 名、第三方库名）混入行为契约，导致 spec 与实现耦合，重构时行为不变但 spec 也得改。

## Goal

把 57 处 purity hit 清零（或降到与外部协议字段名等"合法引用"平齐），同时行为契约零变更。

## Scope

仅改 spec 文字，不改代码。受影响 Requirements 约 12 个。

## Non-goals

- 不改行为契约
- 不调整 Requirement 拆分/合并
- 不处理其他 spec 的 purity 问题
