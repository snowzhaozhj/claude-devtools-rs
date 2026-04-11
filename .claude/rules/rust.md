# Rust Conventions

## Naming

- Types / traits / enum variants: `CamelCase`
- Functions / modules / files: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`
- Crate names: `cdt-<domain>` (dashes in Cargo manifest, underscores in `use` paths)
- Type guards / predicates: `is_<x>` returning `bool`
- Builders: `<Noun>Builder` with `.build() -> Result<Noun, Error>`

## Error handling

- **Library crates**: define a `pub enum <Crate>Error` with `thiserror::Error`; never panic, never `unwrap()` on non-test code paths.
- **Binary (`cdt-cli`)**: use `anyhow::Result<()>` at `main` and propagate with `?`; use `.context("...")` on the important boundaries.
- Validation at system boundaries only (external input, filesystem, IPC, HTTP, SSH). Internal code trusts type-level guarantees.
- Prefer `Result::map_err` over `From` conversions when the context matters.

## Async

- `tokio` lives **only** in crates that actually do I/O (`cdt-parse`, `cdt-watch`, `cdt-ssh`, `cdt-config`, `cdt-discover`, `cdt-api`, `cdt-cli`).
- `cdt-core` and `cdt-analyze` stay **sync**. Pure data transforms are easier to test without a runtime.
- Parsing functions come in two flavors when it makes sense: `parse_entry(line) -> Result<ParsedMessage, _>` (sync, per-line) and `parse_file(path) -> impl Stream<Item = ParsedMessage>` (async, per-file).

## Logging

- Use `tracing` crate everywhere. `tracing::debug!`, `tracing::info!`, `tracing::warn!`, `tracing::error!`.
- Init `tracing_subscriber` once in `cdt-cli::main`. Libraries must never install a global subscriber.
- Attach structured fields: `tracing::info!(session_id = %id, chunk_count = chunks.len(), "built chunks")`.

## Testing

- Unit tests in `#[cfg(test)] mod tests` at the bottom of the file they test.
- Integration tests in `tests/` directory of the crate, when they cross multiple modules.
- For snapshot-heavy tests (chunk building, context stats), introduce `insta` in the port change that needs it — not earlier.
- Every capability spec scenario is a testable behavior. A rule of thumb: each `#### Scenario:` should become at least one test, named in prose (`#[test] fn user_question_then_ai_response_emits_two_chunks()`).

## Module boundaries

- Cross-crate imports go through each crate's **public** API (`pub use` from `lib.rs`), never reaching into `crate::internal::...`.
- Shared types that two or more crates need belong in `cdt-core`. Don't re-export types across leaf crates.
- `cdt-core` MUST NOT depend on `tokio`, `axum`, `notify`, `ssh2`, or any runtime infrastructure.

## Dependencies

- All versions live in the workspace root `[workspace.dependencies]`. Crate-level `Cargo.toml` uses `dep = { workspace = true }`.
- New deps require justification (what specifically we'd write ourselves, what tradeoff). Prefer std / well-known crates over niche ones.
- `unsafe` is forbidden workspace-wide (`#![forbid(unsafe_code)]` via lints). If you genuinely need it, open an issue first.

## Comments

- Default: no comments. Rust names and types carry most of the meaning.
- Write a doc-comment (`///`) on every `pub` item that isn't self-explanatory — especially trait contracts and type invariants.
- Module headers (`//!`) should state what capability the module owns and link to the spec (`openspec/specs/<cap>/spec.md`).
- Do NOT write comments that describe WHAT the code does; write them when WHY is non-obvious (spec reference, TS impl-bug being deliberately fixed, subtle invariant).

## Formatting

- `cargo fmt --all` before every commit. No exceptions.
- Line length: rustfmt default (100). Don't fight the formatter.

## Spec fidelity

- When implementing a capability, the spec under `openspec/specs/<cap>/spec.md` is the source of truth.
- Each `SHALL` in the spec should correspond to at least one test.
- When the TS behavior conflicts with the spec (see `openspec/followups.md`), follow the **spec**, not the TS code. Document the deliberate divergence in the port change's tasks.md.
