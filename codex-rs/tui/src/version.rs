/// The current Codex CLI version as embedded at compile time.
#[cfg(not(test))]
pub const CODEX_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

// Display snapshots use a stable identity so package version bumps do not change
// border spacing or require accepting unrelated UI snapshots.
#[cfg(test)]
pub const CODEX_CLI_VERSION: &str = "0.0.0";
