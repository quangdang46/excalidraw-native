//! MCP server consumer for excalidraw-native.
//!
//! MCP tools expose parsing, validation, description, and rendering through the
//! shared core/render crates.

/// Current crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the canonical crate boundary for diagnostics and smoke tests.
#[must_use]
pub fn crate_boundary() -> &'static str {
    "mcp-tools"
}
