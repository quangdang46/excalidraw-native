//! Shared helpers used by every diagram-type converter.
//!
//! These helpers translate `merman-render` `LayoutNode` / `LayoutEdge`
//! geometry into Excalidraw shapes, manage element ids so the output is
//! deterministic, and provide convenience routines for label placement and
//! text truncation.

use merman_render::model::{LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint};
use serde_json::Value;

use crate::builder::{self, Arrow, Rect, Text};
use crate::options::MermaidConvertOptions;
use crate::style;

/// Allocator that produces stable, prefixed ids (`mm-flow-node-A`, etc.).
pub struct IdGen {
    prefix: String,
}

impl IdGen {
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
        }
    }

    /// Produce a stable id for a known string (e.g. layout node id).
    pub fn for_node(&self, sub: &str, node_id: &str) -> String {
        format!("{}-{}-{}", self.prefix, sub, sanitize_id(node_id))
    }
}

fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Truncate `value` to at most `max_bytes` bytes (UTF-8), suffixed with `…`.
pub fn truncate(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes.saturating_sub(3);
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
        if end == 0 {
            break;
        }
    }
    let mut out = String::with_capacity(end + 3);
    out.push_str(&value[..end]);
    out.push('…');
    out
}

/// Heuristic font-size for label text. Mermaid stores its own font metrics on
/// the layout label; we honour them when present and fall back to the user's
/// configured size otherwise.
pub fn label_font_size(label: &LayoutLabel, opts: &MermaidConvertOptions) -> f64 {
    // Mermaid bbox-derived heights tend to round to ~20px for the default
    // theme; treat the user-requested font_size as authoritative if it's
    // smaller (so callers can shrink labels for dense diagrams).
    let from_label = (label.height * 0.7).max(opts.font_size);
    from_label.min(opts.font_size * 1.5)
}

/// Convert a `LayoutNode` into the Excalidraw shape implied by `shape_kind`,
/// returning the shape JSON value plus its bound text element.
pub fn node_with_text(
    layout: &LayoutNode,
    shape_kind: NodeShape,
    text_value: &str,
    fill: Option<&str>,
    frame_id: Option<&str>,
    ids: &mut IdGen,
    opts: &MermaidConvertOptions,
) -> NodeOutput {
    let shape_id = ids.for_node("node", &layout.id);
    let text_id = ids.for_node("nodetext", &layout.id);

    // Layout x/y are centre coordinates in merman; Excalidraw uses top-left.
    let x = layout.x - layout.width / 2.0;
    let y = layout.y - layout.height / 2.0;

    let rect = Rect {
        id: &shape_id,
        x,
        y,
        width: layout.width.max(40.0),
        height: layout.height.max(40.0),
        fill,
        rounded: matches!(shape_kind, NodeShape::Rectangle { rounded: true }),
        frame_id,
    };

    let mut shape = match shape_kind {
        NodeShape::Rectangle { .. } => builder::rectangle(&rect, opts),
        NodeShape::Ellipse => builder::ellipse(&rect, opts),
        NodeShape::Diamond => builder::diamond(&rect, opts),
    };

    let truncated = truncate(text_value, opts.max_text_size);
    let text = builder::text(
        &Text {
            id: &text_id,
            x,
            y,
            width: rect.width,
            height: rect.height,
            text: &truncated,
            font_size: opts.font_size,
            align: "center",
            container_id: Some(&shape_id),
            frame_id,
        },
        opts,
    );
    builder::bind_text(&mut shape, &text_id);

    NodeOutput {
        shape_id,
        shape,
        text,
    }
}

/// Excalidraw shape produced for a Mermaid node.
#[derive(Debug, Clone, Copy)]
pub enum NodeShape {
    Rectangle { rounded: bool },
    Ellipse,
    Diamond,
}

pub struct NodeOutput {
    pub shape_id: String,
    pub shape: Value,
    pub text: Value,
}

/// Convert a `LayoutEdge` into an Excalidraw arrow with optional edge label.
#[allow(clippy::too_many_arguments)]
pub fn edge_with_label(
    layout: &LayoutEdge,
    start_id: Option<&str>,
    end_id: Option<&str>,
    start_arrowhead: Option<&str>,
    end_arrowhead: Option<&str>,
    stroke_style: &str,
    ids: &mut IdGen,
    opts: &MermaidConvertOptions,
) -> EdgeOutput {
    let arrow_id = ids.for_node("edge", &layout.id);
    let points = sample_points(&layout.points);
    let arrow = builder::arrow(
        &Arrow {
            id: &arrow_id,
            points,
            start_arrowhead,
            end_arrowhead,
            start_binding: start_id,
            end_binding: end_id,
            stroke_style,
            frame_id: None,
        },
        opts,
    );
    let label = layout.label.as_ref().map(|label| {
        let label_id = ids.for_node("edgelabel", &layout.id);
        let text_value = "".to_string();
        let text = builder::text(
            &Text {
                id: &label_id,
                x: label.x,
                y: label.y,
                width: label.width.max(20.0),
                height: label.height.max(16.0),
                text: &text_value,
                font_size: label_font_size(label, opts),
                align: "center",
                container_id: None,
                frame_id: None,
            },
            opts,
        );
        EdgeLabel {
            x: label.x,
            y: label.y,
            width: label.width.max(20.0),
            height: label.height.max(16.0),
            text,
        }
    });

    EdgeOutput {
        arrow_id,
        arrow,
        label,
    }
}

fn sample_points(points: &[LayoutPoint]) -> Vec<(f64, f64)> {
    if points.is_empty() {
        return vec![(0.0, 0.0), (1.0, 0.0)];
    }
    if points.len() == 1 {
        let p = &points[0];
        return vec![(p.x, p.y), (p.x + 1.0, p.y)];
    }
    points.iter().map(|p| (p.x, p.y)).collect()
}

/// Re-position a previously-built text element so it sits centred over
/// `(x, y, width, height)`. Used for edge labels whose final text we discover
/// after the layout step.
pub fn update_text_box(value: &mut Value, x: f64, y: f64, width: f64, height: f64, text: &str) {
    if let Value::Object(map) = value {
        map.insert("x".to_string(), serde_json::json!(x));
        map.insert("y".to_string(), serde_json::json!(y));
        map.insert("width".to_string(), serde_json::json!(width));
        map.insert("height".to_string(), serde_json::json!(height));
        map.insert("text".to_string(), Value::String(text.to_string()));
        map.insert("originalText".to_string(), Value::String(text.to_string()));
    }
}

pub struct EdgeOutput {
    pub arrow_id: String,
    pub arrow: Value,
    pub label: Option<EdgeLabel>,
}

pub struct EdgeLabel {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub text: Value,
}

/// Default node fill applied to most diagram types.
pub fn node_fill() -> &'static str {
    style::NODE_FILL
}
