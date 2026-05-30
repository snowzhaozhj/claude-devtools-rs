<h1 align="center">claude-devtools-rs</h1>

<p align="center">
  <strong>See everything Claude Code did — locally, instantly.</strong>
</p>

<p align="center">
  <sub>A native desktop viewer and analyzer for Claude Code sessions. Reads the
  logs already on your machine and reconstructs the full trace: file paths,
  diffs, thinking, subagents, token usage. No account, no API key, no upload.</sub>
</p>

<p align="center">
  <a href="https://github.com/snowzhaozhj/claude-devtools-rs/releases/latest"><img src="https://img.shields.io/github/v/release/snowzhaozhj/claude-devtools-rs?style=flat-square&label=release&color=blue" alt="Latest release" /></a>&nbsp;
  <a href="https://github.com/snowzhaozhj/claude-devtools-rs/releases"><img src="https://img.shields.io/github/downloads/snowzhaozhj/claude-devtools-rs/total?style=flat-square&color=green" alt="Downloads" /></a>&nbsp;
  <a href="https://github.com/snowzhaozhj/claude-devtools-rs/stargazers"><img src="https://img.shields.io/github/stars/snowzhaozhj/claude-devtools-rs?style=flat-square&color=yellow&label=stars" alt="Stars" /></a>&nbsp;
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey?style=flat-square" alt="Platform" />&nbsp;
  <img src="https://img.shields.io/badge/built%20with-Rust%20%2B%20Tauri-orange?style=flat-square" alt="Rust + Tauri" />&nbsp;
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue?style=flat-square" alt="MIT" /></a>
</p>

<p align="center">
  <a href="https://github.com/snowzhaozhj/claude-devtools-rs/releases/latest"><img src="https://img.shields.io/badge/macOS-Download-black?logo=apple&logoColor=white&style=flat" alt="Download for macOS" height="30" /></a>&nbsp;&nbsp;
  <a href="https://github.com/snowzhaozhj/claude-devtools-rs/releases/latest"><img src="https://img.shields.io/badge/Linux-Download-FCC624?logo=linux&logoColor=black&style=flat" alt="Download for Linux" height="30" /></a>&nbsp;&nbsp;
  <a href="https://github.com/snowzhaozhj/claude-devtools-rs/releases/latest"><img src="https://img.shields.io/badge/Windows-Download-0078D4?logo=windows&logoColor=white&style=flat" alt="Download for Windows" height="30" /></a>
</p>

<p align="center">
  <strong>English</strong> · <a href="./README.zh-CN.md">简体中文</a>
</p>

<p align="center">
  <img src="docs/assets/demo.gif" alt="claude-devtools-rs demo: discover projects, browse sessions, open a session, search" width="100%" />
</p>

---

## Why

**Claude Code hides what it does.** The terminal collapses each step into a
one-line summary — `Read 3 files`, `Searched for 1 pattern`, `Edited 2 files` —
with no paths, no content, no diffs. Thinking is invisible, subagent activity is
buried, and the context window is a three-segment bar with no breakdown. The
only escape hatch is `--verbose`, which dumps raw JSON and thousands of lines of
noise. There is no middle ground.

**claude-devtools-rs reads the logs already saved under `~/.claude/` and
reconstructs everything** — in a native desktop app, not a browser tab or an
Electron shell.

| What the terminal hides | What claude-devtools-rs shows |
|---|---|
| `Read 3 files` | Exact paths, syntax-highlighted content with line numbers |
| `Searched for 1 pattern` | The pattern, every matching file, the matched lines |
| `Edited 2 files` | Inline diffs with added / removed highlighting |
| Three-segment context bar | Per-category token attribution (CLAUDE.md, skills, @-files, tool I/O, thinking, …) |
| Collapsed subagent output | Full execution trees per agent with token / model / error metrics |
| Nothing about thinking | Extended thinking content, fully rendered |
| `--verbose` JSON dump | A structured, searchable, navigable interface |

**Built in Rust + Tauri** — it's a port of the Electron
[claude-devtools](https://github.com/matt1398/claude-devtools), rewritten for
performance: a thousand-message session opens in well under a second, idle CPU
stays near zero, and it won't spin your fan while it sits in the background.

**Everything stays local.** No account, no API key, no network calls — it only
reads files already on your disk, and updates the view live as sessions run.

---

## Screenshots

<table>
  <tr>
    <td width="50%"><img src="docs/assets/session-detail.png" alt="Session Detail — execution trace with tool-call cards" /></td>
    <td width="50%"><img src="docs/assets/sidebar-search.png" alt="Sidebar and cross-session search" /></td>
  </tr>
  <tr>
    <td align="center"><sub><b>Session Detail</b> — execution trace, tool-call cards, inline diffs</sub></td>
    <td align="center"><sub><b>Sidebar + Search</b> — projects, live session list, <code>Cmd+K</code></sub></td>
  </tr>
  <tr>
    <td colspan="2"><img src="docs/assets/tool-viewer.png" alt="Tool viewer — Read/Edit/Write/Bash with syntax highlighting" /></td>
  </tr>
  <tr>
    <td colspan="2" align="center"><sub><b>Tool Viewer</b> — specialised viewers for Read / Edit / Write / Bash with syntax highlighting</sub></td>
  </tr>
</table>

---

## Features

- **Session browsing** — scans `~/.claude/projects/`, groups history by project,
  and follows running sessions live.
- **Execution trace** — UserChunk / AIChunk / SemanticStep segmentation plus
  tool-call cards (Read / Edit / Write / Bash / custom agents).
- **Subagent view** — embedded execution traces with token / model / error metrics.
- **Context panel** — categorised breakdown of what's injected into the window
  (CLAUDE.md, slash commands, @-file references).
- **Global search + command palette** — `Cmd+F` within a session, `Cmd+K` across sessions.
- **Live refresh** — file watcher → debounce → in-place patch, no "loading…" flicker.
- **Desktop notifications + system tray** — custom triggers, Dock unread badge (macOS).
- **SSH remote sessions** — inspect sessions on a remote machine over SSH.
- **Browser access** — opt-in local HTTP server to open the same UI in a browser.
- **Themes** — light / dark / follow system.
- **CLI + MCP/Skills** — query session data from the terminal or let Claude Code
  query its own sessions (see [Claude Code integration](#claude-code-integration)).
- **Fast** — multi-round IPC payload slimming (lazy markdown, `asset://` images,
  lazy subagent / tool output) keeps thousand-message sessions snappy.

---

## Install

### Desktop app

Download the installer for your platform from
[Releases](https://github.com/snowzhaozhj/claude-devtools-rs/releases):

- **macOS**: `.dmg` (Apple Silicon / Intel)
- **Linux**: `.deb` / `.AppImage`
- **Windows**: `.msi` / `.exe`

> The app is **not** signed with an Apple Developer ID (ad-hoc signature only) or
> a Windows code-signing certificate.
>
> **macOS first launch**: after dragging the app from the `.dmg` to
> `/Applications`, **right-click → Open** (not double-click) and confirm. If it's
> still blocked, *System Settings → Privacy & Security* has an "Open anyway"
> button. If you see *"…is damaged and can't be opened"* (browser downloads carry
> a quarantine attribute), run:
> ```bash
> sudo xattr -rd com.apple.quarantine "/Applications/Claude DevTools.app"
> ```
>
> **Windows**: SmartScreen → "More info" → "Run anyway".

### CLI (`cdt`)

The CLI queries session data from the terminal and integrates with Claude Code
via MCP / Skills.

**One-line install** (macOS / Linux):

```bash
curl -fsSL https://raw.githubusercontent.com/snowzhaozhj/claude-devtools-rs/main/install.sh | sh
```

**Other methods:**

| Method | Command |
|---|---|
| Manual download | Grab `cdt-{platform}.tar.gz` from [Releases](https://github.com/snowzhaozhj/claude-devtools-rs/releases) |
| Build from source | `cargo install --git https://github.com/snowzhaozhj/claude-devtools-rs cdt-cli` |

After installing, run `cdt setup mcp --apply` to register the MCP server, or
`cdt setup skills` to install the session-analysis skill. Update with
`cdt self-update` (or re-run the install script).

**Environment variables:**

| Variable | Purpose | Default |
|---|---|---|
| `CDT_INSTALL_DIR` | Install directory | `~/.local/bin` |
| `CDT_VERSION` | Pin a version (e.g. `v0.5.14`) | latest |

---

## Claude Code integration

The `cdt` CLI plugs into Claude Code two ways: as an **MCP server** and as a **skill**.

### MCP server

Register `cdt` so Claude can call session-query tools directly:

```bash
cdt setup mcp --apply              # automatic
# or manually:
claude mcp add cdt-devtools -- cdt mcp serve
```

Claude Code then has `list_projects`, `list_sessions`, `search_sessions`,
`get_session_detail`, `get_session_stats`, and more.

### Skills (recommended)

```bash
cdt setup skills                   # install to .claude/skills/
cdt setup skills --force           # overwrite existing
```

Installs the `session-insights` skill (error analysis, token accounting,
full-text search, single-session diagnostics). Trigger it with
`/session-insights` in Claude Code, or just describe what you want. The skill
shells out to `cdt` directly — no MCP config required.

---

## Browser access

Enable a local HTTP server under *Settings → General → Browser Access*. The app
shows a `http://localhost:<port>` URL (default `3456`); open it in any browser
to get the same UI.

**Security model**: the server binds `127.0.0.1` only, CORS allows just
`localhost` / `127.0.0.1` origins, and there is no token or password auth. It's
meant for the local browser, iframe embedding, or local scripts — it is not
exposed to the LAN. For remote access, put your own reverse proxy, TLS, and auth
in front of it.

Desktop-only capabilities (system tray, Dock badge, native notifications,
in-app updates, Rosetta detection) are hidden or disabled in the browser runtime.

---

## Build from source

**Prerequisites:** Rust stable (`rust-toolchain.toml` pins 1.85+), Node.js 20+,
[pnpm](https://pnpm.io/) 8+, [just](https://github.com/casey/just).

```bash
brew install just pnpm      # if you don't have them
just bootstrap              # install frontend deps (pnpm install)
just dev                    # launch the desktop app in dev mode
```

> This repo uses **pnpm** (not npm) for frontend deps; the lockfile is
> `ui/pnpm-lock.yaml`. After a worktree switch / rebase, run
> `pnpm --dir ui install` to sync.

Common recipes (full list: `just` or `just -l`):

| Command | Purpose |
|---|---|
| `just build` | Compile the workspace |
| `just build-tauri` | Build the desktop app |
| `just test` | Rust + frontend tests |
| `just lint` | clippy (strict) |
| `just fmt` | rustfmt |
| `just check-ui` | svelte-check + tsc |
| `just test-e2e` | Playwright user-story tests |
| `just preflight` | fmt + lint + test + spec-validate, all at once |

### Debug the UI in a browser (no Tauri window)

```bash
pnpm --dir ui run dev
# open http://127.0.0.1:5173/?mock=1&fixture=multi-project-rich
```

`?mock=1` enables dev-only mockIPC backed by fixture data (`empty` /
`single-project` / `multi-project-rich`). The production bundle contains no
mockIPC (verified by vite DCE).

---

## Project structure

```
crates/
├── cdt-core       # shared types (no runtime deps)
├── cdt-parse      # session-parsing
├── cdt-analyze    # chunk-building / tool-linking / context-tracking / team-metadata
├── cdt-discover   # project-discovery / session-search
├── cdt-watch      # file-watching
├── cdt-config     # configuration-management / notification-triggers
├── cdt-ssh        # ssh-remote-context
├── cdt-api        # ipc-data-api / http-data-api
└── cdt-cli        # binary entrypoint (`cdt`)
ui/                # Svelte 5 + Vite frontend
src-tauri/         # Tauri 2 Rust backend (excluded from workspace)
openspec/
├── specs/                       # behaviour contract (authoritative source of truth)
└── TS_BASELINE_DEVIATIONS.md    # TS-port deviation notes
```

---

## Contributing

`main` is the release branch — **don't commit to it directly**. Use a feature
branch + PR:

```bash
git checkout -b feat/xxx
# ...changes
just preflight
git commit -m "..."
git push -u origin feat/xxx
gh pr create --base main
```

CI (`.github/workflows/ci.yml`) runs fmt / clippy / test and must be green before
merge. Project conventions and architecture live in [`CLAUDE.md`](./CLAUDE.md);
behaviour contracts in `openspec/specs/<capability>/spec.md`.

## Release

Versions are kept in sync across three files: `Cargo.toml` (workspace),
`src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.

```bash
git checkout main && git pull
just release-check          # verify versions + clean tree + preflight
git tag v0.2.0
git push origin v0.2.0      # triggers .github/workflows/release.yml
```

The tag build (`tauri-apps/tauri-action`) produces macOS arm64/x64 + Linux +
Windows bundles into a Draft Release. The app integrates `tauri-plugin-updater`
for in-app updates (macOS / Windows / Linux AppImage; `.deb` excluded). See
[CHANGELOG.md](CHANGELOG.md) for release history.

## Documentation

- **Conventions / architecture**: [`CLAUDE.md`](./CLAUDE.md)
- **Behaviour contracts**: `openspec/specs/<capability>/spec.md`
- **OpenSpec workflow**: [`openspec/README.md`](./openspec/README.md)

## License

[MIT](LICENSE)
