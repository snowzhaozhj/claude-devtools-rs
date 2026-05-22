# scripts/repro/

Reproduction scripts for SSH-related issues. These are **diagnostic tools**, not
CI tests — they require a real Docker container and your local SSH keys. CI
won't run them; use them locally when investigating an SSH bug or validating a
fix.

## Setup once

```bash
# Pre-build cdt-cli so each script's `cargo run` starts in seconds
cargo build -p cdt-cli --bin cdt
```

## Scripts

### `setup-ssh-fixture-container.sh`

Spin up an **isolated** SSH test container (`cdt-ssh-fixture-test`, port 2223)
with an RW-mounted independent fixture home. Coexists with the original
`cdt-ssh-test` container (port 2222, ro-mount of your real `~/.claude`) — does
not touch it.

The fixture data uses `/srv/ssh-fixture-NNN` paths (encoded as
`-srv-ssh-fixture-NNN` project IDs) that **never appear in your local
`~/.claude/projects`**. If you see these in the desktop sidebar, SSH data
really loaded; if you don't, you're looking at Local data.

```bash
# default: 3 projects × 1 session × 4 messages
bash scripts/repro/setup-ssh-fixture-container.sh up

# large fixture for stress testing (e.g. reproduce list_repository_groups slowness)
bash scripts/repro/setup-ssh-fixture-container.sh up --scale 50 10 20

# refresh fixture only, leave container running
bash scripts/repro/setup-ssh-fixture-container.sh refresh --scale 100 5 10

# inspect state
bash scripts/repro/setup-ssh-fixture-container.sh status

# tear down
bash scripts/repro/setup-ssh-fixture-container.sh down
```

Desktop connection config after `up`:

| Field | Value |
|---|---|
| Host | `localhost` |
| Port | `2223` (override `CDT_SSH_FIXTURE_PORT`) |
| Username | `devuser` |
| Auth | SSH config (uses `~/.ssh/id_*.pub`) |

### `repro-ssh-dead-channel.sh`

Reproduce **A1**: SFTP "dead channel" via `pkill -STOP sshd`. This proves that
`is_permanent_sftp_failure` does not match `timeout` — the polling watcher
counts `Transient("timeout")` as recoverable forever, so `dead_signal` never
fires. Currently expected to "fail": after 60s with sshd suspended, active
context is still `ssh`, not `local`.

See `openspec/followups.md::ssh-remote-context::SFTP 失效检测不完整`.

```bash
bash scripts/repro/repro-ssh-dead-channel.sh
```

Prereqs: `cdt-ssh-test` container running on port 2222.

### `repro-disconnect-perf.sh`

Measure **A2**: `ssh_disconnect` IPC latency + post-disconnect list call timing.
Acts as a regression baseline for the disconnect path. In current measurements:
disconnect IPC = ~165 ms, post-disconnect list_repository_groups = ~67 ms — if a
future change makes any of these significantly worse, run this and compare.

```bash
bash scripts/repro/repro-disconnect-perf.sh
```

Prereqs: `cdt-ssh-test` container running on port 2222.

## Why not in CI?

These scripts depend on:
- A long-running Docker container
- Your local `~/.ssh/id_*.pub` for authentication
- Stable network access to `localhost:2222` / `:2223`

CI runners don't have any of that. They're for **local diagnosis** when a
SSH-related bug needs to be reproduced under controlled conditions, or when a
SSH-touching PR needs regression validation.
