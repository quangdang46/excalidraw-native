//! Core Excalidraw parsing, validation, and normalization primitives.
//!
//! This crate owns file-format compatibility and scene normalization. Rendering
//! and user-interface crates consume this API instead of parsing independently.

/// Current crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the canonical crate boundary for diagnostics and smoke tests.
#[must_use]
pub fn crate_boundary() -> &'static str {
    "parse-validate-normalize"
}
