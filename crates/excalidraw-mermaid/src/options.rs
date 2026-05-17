//! Public options for Mermaid → Excalidraw conversion.
//!
//! Mirrors the shape documented in `excalidraw-native-PLAN.md` §21.3 so that
//! upstream callers (CLI, MCP, library embedders) have a stable surface.

/// Configuration controlling the conversion pipeline.
#[derive(Debug, Clone)]
pub struct MermaidConvertOptions {
    /// Base Excalidraw font size used for node and label text.
    pub font_size: f64,
    /// Curve style applied to flowchart edges. Sequence/class/state/ER
    /// converters use their own routing and ignore this field.
    pub flowchart_curve: FlowchartCurve,
    /// Maximum number of edges (arrows) accepted before [`crate::MermaidConvertError::LimitExceeded`]
    /// is returned. Set to `usize::MAX` to disable.
    pub max_edges: usize,
    /// Maximum length of node / label text accepted before truncation. Set to
    /// `usize::MAX` to disable. Values are measured in bytes of the UTF-8
    /// string and the truncated text is suffixed with `…` when shortened.
    pub max_text_size: usize,
    /// What to do when a diagram type is not handled by a dedicated converter.
    pub on_unsupported: OnUnsupported,
    /// When `true`, generated elements use the Excalidraw default hachure fill
    /// style instead of `solid`. The default mirrors the official
    /// "@excalidraw/mermaid-to-excalidraw" output (`solid` for nodes,
    /// `transparent` for backgrounds, no hachure).
    pub hachure_fill: bool,
}

impl Default for MermaidConvertOptions {
    fn default() -> Self {
        Self {
            font_size: 16.0,
            flowchart_curve: FlowchartCurve::default(),
            max_edges: 1_000,
            max_text_size: 4_000,
            on_unsupported: OnUnsupported::default(),
            hachure_fill: false,
        }
    }
}

/// Curve style applied to flowchart edges. Maps to Mermaid's
/// `flowchart.curve` config field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlowchartCurve {
    /// Polyline routing (default in Mermaid Live for the rough/handDrawn theme).
    #[default]
    Linear,
    /// Smoother basis spline routing (Mermaid default for the classic theme).
    Basis,
}

/// What the converter should do when it encounters a Mermaid diagram type it
/// does not know how to map to Excalidraw shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OnUnsupported {
    /// Return [`crate::MermaidConvertError::Unsupported`] immediately.
    Error,
    /// Emit a single Excalidraw rectangle + text element describing the
    /// unsupported diagram type. Useful for the CLI/MCP "best effort" mode.
    #[default]
    Placeholder,
}
