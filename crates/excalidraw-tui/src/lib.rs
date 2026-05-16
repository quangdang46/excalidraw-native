//! Terminal preview consumer for excalidraw-native.
//!
//! The TUI displays render outputs from `excalidraw-render`; it does not own
//! renderer semantics.

/// Current crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the canonical crate boundary for diagnostics and smoke tests.
#[must_use]
pub fn crate_boundary() -> &'static str {
    "terminal-viewer"
}
