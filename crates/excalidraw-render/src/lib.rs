//! Rendering backend for normalized Excalidraw scenes.
//!
//! This crate will own SVG/PNG generation, rough-rs integration, text layout,
//! image handling, frames, and render warnings.

/// Current crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the canonical crate boundary for diagnostics and smoke tests.
#[must_use]
pub fn crate_boundary() -> &'static str {
    "svg-png-render"
}
