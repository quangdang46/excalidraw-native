//! Mermaid → Excalidraw conversion built on `merman-core` / `merman-render`.
//!
//! The crate parses Mermaid source text with `merman-core`, computes geometry
//! with `merman-render`, and then maps the resulting `LayoutedDiagram` to
//! Excalidraw elements that the v0.1 renderer in this workspace can draw.
//!
//! See `excalidraw-native-PLAN.md` §21 for the v0.2 contract.

pub mod builder;
pub mod convert;
pub mod engine;
pub mod error;
pub mod fallback;
pub mod options;
pub mod style;

pub use convert::{convert_layouted, convert_layouted_to_file};
pub use error::MermaidConvertError;
pub use options::{FlowchartCurve, MermaidConvertOptions, OnUnsupported};

use excalidraw_core::{parse_str, Element, ExcalidrawFile};
use serde_json::Value;

/// Parse a Mermaid source string and return the resulting Excalidraw elements.
///
/// Element ordering follows the conversion order (background frames first,
/// nodes next, edges + labels last) so that callers can preserve z-order when
/// merging into a larger scene.
pub fn parse_to_excalidraw(
    mermaid_text: &str,
    options: &MermaidConvertOptions,
) -> Result<Vec<Element>, MermaidConvertError> {
    let file = parse_to_excalidraw_file(mermaid_text, options)?;
    Ok(file.elements)
}

/// Parse a Mermaid source string and return a full `.excalidraw` document.
pub fn parse_to_excalidraw_file(
    mermaid_text: &str,
    options: &MermaidConvertOptions,
) -> Result<ExcalidrawFile, MermaidConvertError> {
    let value = parse_to_excalidraw_value(mermaid_text, options)?;
    let raw = serde_json::to_string(&value)?;
    parse_str(&raw).map_err(MermaidConvertError::from)
}

/// Parse a Mermaid source string and return the raw `.excalidraw` JSON `Value`.
///
/// Use this when you want to round-trip the document through serde without
/// going through the typed `ExcalidrawFile` (e.g. CLI/MCP writing to disk).
pub fn parse_to_excalidraw_value(
    mermaid_text: &str,
    options: &MermaidConvertOptions,
) -> Result<Value, MermaidConvertError> {
    let layouted = engine::layout_mermaid(mermaid_text, options)?;
    convert::convert_layouted_to_file(&layouted, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_flowchart_produces_elements() {
        let mermaid =
            "flowchart TD\n  A[Start] --> B{Decision}\n  B -->|Yes| C[OK]\n  B -->|No| D[Stop]\n";
        let opts = MermaidConvertOptions::default();
        let elements = parse_to_excalidraw(mermaid, &opts).expect("conversion should succeed");
        assert!(
            !elements.is_empty(),
            "expected at least one element from flowchart"
        );
    }

    #[test]
    fn parse_to_file_round_trips_through_parser() {
        let mermaid = "flowchart LR\n  A --> B\n";
        let opts = MermaidConvertOptions::default();
        let file = parse_to_excalidraw_file(mermaid, &opts).expect("file should parse");
        assert_eq!(file.file_type, "excalidraw");
        assert!(!file.elements.is_empty());
    }
}
