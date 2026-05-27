# 合法 spec 内容（应 0 违规）

## Purpose

系统 SHALL 在 200ms 内完成冷启动扫描。

## Requirements

系统 MUST 保持 wall time < 500ms baseline 预算。

系统 SHALL 渲染 `UpdateBanner.svelte` 组件。

系统 SHALL 通过 `tauri-plugin-updater` 实现更新。

系统 SHALL 调用 `tool_linking::filter_resolved_tasks` 过滤。

系统 MUST 维持 max RSS < 200 MB SLA。

inline suppress 示例：参考 crates/cdt-parse/src/parser.rs 的实现 <!-- spec-purity: ok -->
