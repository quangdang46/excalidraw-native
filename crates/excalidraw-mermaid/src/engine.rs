//! Thin wrapper around `merman-core` / `merman-render` that handles the
//! synchronous Mermaid parse + layout pipeline.

use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutedDiagram;
use merman_render::LayoutOptions;

use crate::error::MermaidConvertError;
use crate::options::MermaidConvertOptions;

/// Run the synchronous Mermaid pipeline and return a fully-laid-out diagram.
///
/// `merman` performs its work synchronously when `parse_diagram_sync` /
/// `layout_parsed` are used, so this function is safe to call from the CLI,
/// the MCP server (within `tokio::task::spawn_blocking`), and library tests.
pub fn layout_mermaid(
    mermaid_text: &str,
    _options: &MermaidConvertOptions,
) -> Result<LayoutedDiagram, MermaidConvertError> {
    let engine = Engine::new();
    let parse_options = ParseOptions::default();
    let parsed = engine
        .parse_diagram_sync(mermaid_text, parse_options)
        .map_err(|err| MermaidConvertError::Parse(err.to_string()))?
        .ok_or(MermaidConvertError::Empty)?;
    let layout_options = LayoutOptions::headless_svg_defaults();
    merman_render::layout_parsed(&parsed, &layout_options)
        .map_err(|err| MermaidConvertError::Layout(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flowchart_lays_out() {
        let mermaid = "flowchart TD\n  A[Start] --> B[End]\n";
        let opts = MermaidConvertOptions::default();
        let layouted = layout_mermaid(mermaid, &opts).expect("flowchart should lay out");
        assert_eq!(layouted.meta.diagram_type, "flowchart-v2");
    }

    #[test]
    fn empty_input_returns_empty_error() {
        let opts = MermaidConvertOptions::default();
        let err = layout_mermaid("", &opts).expect_err("empty input should error");
        assert!(matches!(
            err,
            MermaidConvertError::Empty | MermaidConvertError::Parse(_)
        ));
    }
}
