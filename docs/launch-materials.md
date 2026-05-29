# Launch materials

发布素材草稿（issue #392）。社交平台真人发帖前请按当下数据/口吻微调。

## Repo metadata（已通过 `gh repo edit` 设置）

- **Description**: `Desktop viewer and analyzer for Claude Code sessions — Rust + Tauri + Svelte`
- **Topics**: `claude-code` · `devtools` · `rust` · `tauri` · `svelte` · `ai-coding` · `session-viewer`
- **Homepage**: https://github.com/snowzhaozhj/claude-devtools-rs/releases

## Hacker News — Show HN

**Title (≤ 80 chars):**

```
Show HN: Desktop viewer for Claude Code sessions, built in Rust
```

**Alternates:**

```
Show HN: See everything Claude Code did — a native session viewer in Rust/Tauri
Show HN: Claude Code hides what it does; this Rust app shows the full trace
```

**Body:**

> Claude Code keeps detailed session logs in `~/.claude/projects/`, but the
> terminal collapses them into one-line summaries (`Read 3 files`,
> `Edited 2 files`) — no paths, no diffs, no thinking, no subagent traces. The
> only escape hatch is `--verbose`, which dumps raw JSON.
>
> This is a native desktop app (Rust + Tauri, not Electron) that reads those
> logs and reconstructs the full picture: exact file paths with syntax
> highlighting, inline edit diffs, per-turn token attribution, subagent
> execution trees, and extended thinking. It watches the log directory and
> patches the view in place, so a running session updates live.
>
> It's a Rust rewrite of an existing Electron tool — the motivation was
> performance: a 1000-message session opens in well under a second and idle CPU
> stays near zero (it shouldn't spin your fan while it sits in the background).
>
> Everything is local. No API keys, no account, no network calls — it only
> reads files already on your disk. There's also a CLI (`cdt`) that registers
> as a Claude Code MCP server / skill so Claude can query its own past sessions.
>
> Binaries for macOS / Linux / Windows on the releases page; builds from source
> with `just dev`. Feedback welcome.

**Posting tips:**
- 周二–周四 美西早上 8–10 点（PT）发，避开周末。
- 发完别拉票，前几条评论作者亲自回复体验问题。
- 准备好 demo GIF 直链（GitHub user-attachments）放在第一条评论。

## Reddit — r/ClaudeAI

**Title:**

```
I built a native desktop viewer for Claude Code sessions (Rust + Tauri, fully local)
```

**Body:**

> Claude Code stores every session under `~/.claude/`, but the terminal only
> shows collapsed summaries — you can't see the file paths it read, the diffs it
> applied, the subagents it spawned, or where the context window filled up.
>
> So I built **claude-devtools-rs**: a desktop app that reads those logs and
> reconstructs the whole session.
>
> - **Execution trace** — user / assistant / tool-call cards (Read, Edit, Write,
>   Bash, custom agents), with syntax highlighting and inline diffs
> - **Subagent trees** — token / model / error metrics per agent
> - **Context panel** — what's injected into the window (CLAUDE.md, slash
>   commands, @-file references), categorized
> - **Live refresh** — watches the log dir and patches the view in place, no
>   "loading…" flicker
> - **Search** — `Cmd+F` in a session, `Cmd+K` across sessions
> - **Local-only** — no API keys, no account, no network; it just reads files on
>   your machine
>
> It's a Rust + Tauri rewrite of an Electron tool — native, low memory, opens
> 1000-message sessions instantly. There's also a `cdt` CLI that plugs into
> Claude Code as an MCP server / skill.
>
> Downloads (macOS / Linux / Windows) + source:
> https://github.com/snowzhaozhj/claude-devtools-rs
>
> Happy to answer questions — what would you want to see in a tool like this?

**Posting tips:**
- 读一遍 r/ClaudeAI 的 self-promotion / show-off 规则，必要时打 flair。
- 标题强调 "fully local / no data leaves your machine" —— 该社区对隐私敏感。
- 配 1 张高信息密度截图（Session Detail）比 GIF 更适合 Reddit feed。

## Demo GIF 规格（录屏脚本）

30 秒，1440×900 窗口，浅色或深色主题二选一（建议深色更出片）：

1. (0–6s) 启动 → Dashboard 项目卡片网格，鼠标扫过几个项目
2. (6–14s) 点进一个项目 → sidebar session 列表，标题/消息数 fade-in
3. (14–22s) 打开一个 session → 滚动执行轨迹，展开一个 Edit 工具卡看 diff
4. (22–28s) `Cmd+K` 跨 session 搜索，输入关键词，结果高亮
5. (28–30s) 收尾停在 Session Detail

压缩：`gifski`（macOS：`brew install gifski`）。`ffmpeg -i demo.mov -vf scale=1280:-1 -r 15 frames/%04d.png && gifski -o demo.gif --fps 15 frames/*.png`，目标 < 8 MB（GitHub README inline 上限）。或直接传 `.mov` 到 GitHub release/comment 用 `<video>` 标签内嵌。
