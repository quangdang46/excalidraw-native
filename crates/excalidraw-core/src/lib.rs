//! Core Excalidraw parsing, validation, and normalization primitives.
//!
//! This crate owns file-format compatibility and scene normalization. Rendering
//! and user-interface crates consume this API instead of parsing independently.

pub mod color;
pub mod defaults;
pub mod normalize;
pub mod parse;
pub mod types;
pub mod validate;

pub use color::{parse_excalidraw_color, Color, ColorParseError};
pub use defaults::font_family_css;
pub use normalize::{normalize_file, NormalizedElement, Point, Rect, Scene, SceneWarning};
pub use parse::{parse_reader, parse_slice, parse_str, ParseError};
pub use types::{
    AppState, ArrowBinding, Arrowhead, BaseElement, BoundElement, Element, ExcalidrawFile,
    FileData, FillStyle, FrameElement, FreedrawElement, ImageCrop, ImageElement, LinearElement,
    Roundness, ShapeElement, StrokeStyle, TextAlign, TextElement, UnsupportedElement,
    VerticalAlign,
};
pub use validate::{
    validate_file, validate_str, ValidationError, ValidationLimits, ValidationReport,
    ValidationWarning,
};

/// Current crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the canonical crate boundary for diagnostics and smoke tests.
#[must_use]
pub fn crate_boundary() -> &'static str {
    "parse-validate-normalize"
}
