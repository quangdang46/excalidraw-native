//! Default Excalidraw styling for Mermaid-derived elements.
//!
//! Values mirror the defaults used by the JS port of `mermaid-to-excalidraw`
//! so that converted scenes look familiar in the Excalidraw UI.

use crate::options::MermaidConvertOptions;

/// Default stroke color (Excalidraw "ink" black).
pub const STROKE: &str = "#1e1e1e";

/// Default node background (light grey, matches Mermaid `flowchart.nodeFill`).
pub const NODE_FILL: &str = "#e7f5ff";

/// Cluster / subgraph background.
pub const CLUSTER_FILL: &str = "#ffeccf";

/// Class-diagram / ER table header fill.
pub const HEADER_FILL: &str = "#e9ecef";

/// Sequence-diagram actor (participant) box fill.
pub const ACTOR_FILL: &str = "#dde9f7";

/// Color used for state-diagram start / end pseudostates.
pub const STATE_PSEUDO_FILL: &str = "#1e1e1e";

/// Stroke width applied to nodes / edges.
pub const STROKE_WIDTH: f64 = 2.0;

/// Roughness preset matching the Excalidraw default (`1` = hand-drawn).
pub const ROUGHNESS: f64 = 1.0;

/// Default text line-height multiplier (Excalidraw "default" value).
pub const LINE_HEIGHT: f64 = 1.25;

/// Returns the fill style string used for filled shapes given the options.
#[must_use]
pub fn fill_style(opts: &MermaidConvertOptions) -> &'static str {
    if opts.hachure_fill {
        "hachure"
    } else {
        "solid"
    }
}

/// Round-rect roundness type. `3` selects the proportional variant used by
/// Excalidraw for shapes with rounded corners.
pub const ROUND_RECT: u32 = 3;
