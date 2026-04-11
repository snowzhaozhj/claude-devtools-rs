//! claude-devtools-rs binary entrypoint.
//!
//! Wires the data API facade and runs either the HTTP server or a headless
//! inspection command. Currently a bootstrap stub: prints a readiness line
//! and exits, to confirm the workspace compiles end-to-end.

use anyhow::Result;

#[allow(clippy::unnecessary_wraps)] // main will grow to propagate service errors
fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    println!("claude-devtools-rs bootstrap OK");
    Ok(())
}
