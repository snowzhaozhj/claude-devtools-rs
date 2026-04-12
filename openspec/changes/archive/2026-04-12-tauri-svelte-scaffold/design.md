## Context

数据层全部就位。需要最小可用 UI 验证 Tauri + Svelte + `LocalDataApi` 链路。

## Goals / Non-Goals

**Goals:**
- Tauri 2 项目初始化（使用 `create-tauri-app` 脚手架）
- Svelte 5 + Vite 前端骨架
- Tauri IPC commands：`list_projects`、`list_sessions`、`get_session_detail`
- 两个页面：项目列表 → 点击进入会话列表
- 基础样式（暗色主题，类 IDE 风格）

**Non-Goals:**
- 完整 UI 功能（搜索、配置、通知、SSH）→ 后续迭代
- 虚拟滚动 → 骨架验证后再加
- 打包发布 → 先 `cargo tauri dev` 跑通

## Decisions

### D1: 目录结构

```
claude-devtools-rs/
├── ui/                    # Svelte 前端
│   ├── src/
│   │   ├── App.svelte
│   │   ├── lib/
│   │   │   └── api.ts     # Tauri invoke 封装
│   │   └── routes/
│   │       ├── ProjectList.svelte
│   │       └── SessionList.svelte
│   ├── package.json
│   └── vite.config.ts
├── src-tauri/             # Tauri Rust 后端
│   ├── src/
│   │   └── lib.rs         # IPC commands
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── build.rs
└── crates/                # 已有数据层
```

### D2: IPC 而非 HTTP

Tauri 的 `invoke` 直接调 Rust 函数，不走 HTTP server。比 HTTP 快（无序列化/网络开销），且天然类型安全。

### D3: 状态管理

`LocalDataApi` 放在 Tauri 的 `manage()` state 中，IPC handler 通过 `State<Arc<LocalDataApi>>` 访问。

## Risks / Trade-offs

- **[Risk] Tauri 2 + Svelte 5 都较新** → 社区支持已足够成熟
- **[Trade-off] `src-tauri` 独立于 workspace** → Tauri 有自己的 Cargo.toml，不加入 workspace members（避免干扰 `cargo test --workspace`）
