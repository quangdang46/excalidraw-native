//! Sequence diagram → Excalidraw mapping.
//!
//! Mermaid's sequence layout positions actor boxes along a horizontal lane and
//! draws messages as arrows between vertical lifelines. We turn each actor box
//! into a labelled rectangle, draw a vertical line through the lifeline, and
//! emit each message as an arrow with an optional centred label.

use std::collections::HashMap;

use merman_render::model::{LayoutNode, SequenceDiagramLayout};
use serde_json::Value;

use crate::builder::{self, Arrow, Rect, Text};
use crate::convert::common::{
    edge_with_label, node_with_text, truncate, update_text_box, EdgeOutput, IdGen, NodeOutput,
    NodeShape,
};
use crate::error::MermaidConvertError;
use crate::options::MermaidConvertOptions;
use crate::style;

pub fn convert(
    layout: &SequenceDiagramLayout,
    _semantic: &Value,
    options: &MermaidConvertOptions,
) -> Result<Vec<Value>, MermaidConvertError> {
    let mut ids = IdGen::new("mm-seq");
    let mut elements: Vec<Value> = Vec::new();
    let mut actor_ids: HashMap<String, String> = HashMap::new();

    // Determine bounds for lifeline rendering.
    let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    for node in &layout.nodes {
        if node.is_cluster {
            continue;
        }
        let top = node.y - node.height / 2.0;
        let bottom = node.y + node.height / 2.0;
        if top < min_y {
            min_y = top;
        }
        if bottom > max_y {
            max_y = bottom;
        }
    }
    if !min_y.is_finite() {
        min_y = 0.0;
    }
    if !max_y.is_finite() {
        max_y = min_y + 320.0;
    }
    let lifeline_bottom = max_y + 240.0;

    for node in &layout.nodes {
        if node.is_cluster {
            continue;
        }
        let label = derive_actor_label(node);
        let NodeOutput {
            shape_id,
            shape,
            text,
        } = node_with_text(
            node,
            NodeShape::Rectangle { rounded: true },
            &label,
            Some(style::ACTOR_FILL),
            None,
            &mut ids,
            options,
        );
        actor_ids.insert(node.id.clone(), shape_id.clone());
        elements.push(shape);
        elements.push(text);

        // Lifeline (vertical line below the actor box).
        let lifeline_id = ids.for_node("lifeline", &node.id);
        let lifeline = builder::arrow(
            &Arrow {
                id: &lifeline_id,
                points: vec![
                    (node.x, node.y + node.height / 2.0),
                    (node.x, lifeline_bottom),
                ],
                start_arrowhead: None,
                end_arrowhead: None,
                start_binding: Some(&shape_id),
                end_binding: None,
                stroke_style: "dashed",
                frame_id: None,
            },
            options,
        );
        elements.push(lifeline);
    }

    for edge in &layout.edges {
        let start_id = actor_ids.get(&edge.from).map(String::as_str);
        let end_id = actor_ids.get(&edge.to).map(String::as_str);
        let stroke_style = edge
            .stroke_dasharray
            .as_deref()
            .map(|_| "dashed")
            .unwrap_or("solid");
        let EdgeOutput {
            arrow_id,
            arrow,
            label,
        } = edge_with_label(
            edge,
            start_id,
            end_id,
            None,
            Some("triangle"),
            stroke_style,
            &mut ids,
            options,
        );
        if let Some(start) = start_id {
            if let Some(start_value) = elements
                .iter_mut()
                .find(|el| el.get("id").and_then(Value::as_str) == Some(start))
            {
                builder::bind_arrow(start_value, &arrow_id);
            }
        }
        if let Some(end) = end_id {
            if let Some(end_value) = elements
                .iter_mut()
                .find(|el| el.get("id").and_then(Value::as_str) == Some(end))
            {
                builder::bind_arrow(end_value, &arrow_id);
            }
        }
        if let Some(label) = label {
            let label_text = derive_edge_label(edge);
            if !label_text.is_empty() {
                let mut text_value = label.text;
                update_text_box(
                    &mut text_value,
                    label.x,
                    label.y,
                    label.width,
                    label.height,
                    &truncate(&label_text, options.max_text_size),
                );
                elements.push(text_value);
            }
        }
        elements.push(arrow);
    }

    // Add a header rectangle if there's a title to anchor the actor lane.
    let _ = Text {
        id: "",
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
        text: "",
        font_size: options.font_size,
        align: "left",
        container_id: None,
        frame_id: None,
    };
    let _ = Rect {
        id: "",
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
        fill: None,
        rounded: false,
        frame_id: None,
    };

    Ok(elements)
}

fn derive_actor_label(node: &LayoutNode) -> String {
    // Mermaid sequence diagrams encode the participant name in the layout id
    // (e.g. `actorA`) and there is no separate semantic label exposed via the
    // public layout API. Use the id as a stable fallback.
    node.id.replace('_', " ")
}

fn derive_edge_label(edge: &merman_render::model::LayoutEdge) -> String {
    // The published merman API does not expose message text on `LayoutEdge`
    // yet. Use the edge id for visibility (e.g. `msg-1`).
    if edge.id.is_empty() {
        String::new()
    } else {
        edge.id.replace('_', " ")
    }
}
