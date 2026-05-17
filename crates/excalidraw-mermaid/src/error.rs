//! Error type returned by the public conversion API.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MermaidConvertError {
    /// `merman-core` could not parse the input text (syntax error / unsupported
    /// directive / etc.).
    #[error("failed to parse Mermaid source: {0}")]
    Parse(String),

    /// The diagram parsed successfully but `merman-render` does not know how to
    /// lay it out yet.
    #[error("failed to lay out Mermaid diagram: {0}")]
    Layout(String),

    /// `merman-core::parse_diagram_sync` returned `None`, meaning the source
    /// did not contain a recognisable diagram (empty input, only directives,
    /// etc.).
    #[error("input does not contain a parseable Mermaid diagram")]
    Empty,

    /// The diagram type is not supported by this crate yet *and*
    /// [`crate::OnUnsupported::Error`] was requested.
    #[error("unsupported Mermaid diagram type: {diagram_type}")]
    Unsupported { diagram_type: String },

    /// Limit guard tripped (max edges, max text size, etc.).
    #[error("Mermaid input exceeds conversion limit: {message}")]
    LimitExceeded { message: String },

    /// Output JSON failed to round-trip through `excalidraw-core::parse_str`.
    /// This indicates a bug in the converter rather than user input.
    #[error("converted Excalidraw document failed to parse: {0}")]
    OutputParse(#[from] excalidraw_core::ParseError),

    /// Generic JSON serialisation error during output building.
    #[error("JSON serialisation error: {0}")]
    Json(#[from] serde_json::Error),
}
