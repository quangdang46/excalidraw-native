//! Sequence diagram → Excalidraw mapping.
//!
//! Mermaid's sequence layout positions actor boxes along a horizontal lane and
//! draws messages as arrows between vertical lifelines. We turn each actor box
//! into a labelled rectangle, draw a vertical line through the lifeline, and
//! emit each message as an arrow with an optional centred label.

use std::collections::HashMap;

use merman_render::model::SequenceDiagramLayout;
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
    semantic: &Value,
    options: &MermaidConvertOptions,
) -> Result<Vec<Value>, MermaidConvertError> {
    let mut ids = IdGen::new("mm-seq");
    let mut elements: Vec<Value> = Vec::new();
    let mut actor_ids: HashMap<String, String> = HashMap::new();
    // Build lookup from semantic data for actors and messages.
    let actor_labels = index_actors(semantic);
    let messages = index_messages(semantic);
    // Track which actor names we've already emitted (skip "actor-bottom-*" duplicates).
    let mut emitted_actors: HashMap<String, bool> = HashMap::new();

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
        // Extract the real actor name from the layout node id.
        let actor_name = extract_actor_name(&node.id);
        // Skip "actor-bottom-*" duplicates — only emit one box per actor.
        let is_bottom = node.id.starts_with("actor-bottom-");
        if is_bottom && emitted_actors.contains_key(&actor_name) {
            continue;
        }
        if emitted_actors.contains_key(&actor_name) && !is_bottom {
            continue;
        }
        emitted_actors.insert(actor_name.clone(), true);
        // Derive label from semantic actor data, falling back to the extracted name.
        let label = actor_labels.get(&actor_name).cloned().unwrap_or(actor_name);
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

    for (idx, edge) in layout.edges.iter().enumerate() {
        // Skip lifeline edges — they're rendered above as part of actor emission.
        if edge.id.starts_with("lifeline-") {
            continue;
        }
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
            // Look up the real message text from semantic data.
            let label_text = messages.get(idx).cloned().unwrap_or_default();
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

/// Extract the real actor name from a layout node id like "actor-top-Alice"
/// or "actor-bottom-Bob".
fn extract_actor_name(node_id: &str) -> String {
    let stripped = node_id
        .strip_prefix("actor-top-")
        .or_else(|| node_id.strip_prefix("actor-bottom-"))
        .unwrap_or(node_id);
    stripped.to_string()
}

/// Build a lookup from actor name → display label using the semantic JSON.
/// Mermaid semantic stores actors as an object keyed by name with
/// `name` and `description` fields.
fn index_actors(semantic: &Value) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Some(obj) = semantic.get("actors").and_then(Value::as_object) {
        for (key, actor) in obj {
            let label = actor
                .get("description")
                .or_else(|| actor.get("name"))
                .and_then(Value::as_str)
                .unwrap_or(key.as_str());
            map.insert(key.clone(), label.to_string());
        }
    }
    map
}

/// Build an ordered list of message texts from the semantic JSON.
/// Messages are stored as an array under `semantic.messages`.
fn index_messages(semantic: &Value) -> Vec<String> {
    let Some(arr) = semantic.get("messages").and_then(Value::as_array) else {
        return Vec::new();
    };
    arr.iter()
        .map(|msg| {
            msg.get("message")
                .or_else(|| msg.get("text"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        })
        .collect()
}
