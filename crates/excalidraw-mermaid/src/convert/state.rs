//! State diagram → Excalidraw mapping.

use std::collections::HashMap;

use merman_render::model::StateDiagramV2Layout;
use serde_json::Value;

use crate::builder::{self, Rect};
use crate::convert::common::{
    edge_with_label, node_with_text, EdgeOutput, IdGen, NodeOutput, NodeShape,
};
use crate::error::MermaidConvertError;
use crate::options::MermaidConvertOptions;
use crate::style;

pub fn convert(
    layout: &StateDiagramV2Layout,
    _semantic: &Value,
    options: &MermaidConvertOptions,
) -> Result<Vec<Value>, MermaidConvertError> {
    let mut ids = IdGen::new("mm-state");
    let mut elements: Vec<Value> = Vec::new();
    let mut shape_ids: HashMap<String, String> = HashMap::new();

    // Emit clusters first as background frames.
    for cluster in &layout.clusters {
        let frame_id = ids.for_node("cluster", &cluster.id);
        let label = if cluster.title.trim().is_empty() {
            None
        } else {
            Some(cluster.title.as_str())
        };
        elements.push(builder::frame(
            &frame_id,
            label,
            cluster.x,
            cluster.y,
            cluster.width.max(40.0),
            cluster.height.max(40.0),
            options,
        ));
    }

    for node in &layout.nodes {
        if node.is_cluster {
            continue;
        }
        // Mermaid uses pseudostates `[*]` for start/end. Render them as filled
        // black circles using a small ellipse.
        let is_pseudo =
            node.id == "[*]" || node.id.starts_with("__start__") || node.id.starts_with("__end__");
        if is_pseudo {
            let id = ids.for_node("pseudo", &node.id);
            let x = node.x - node.width.max(24.0) / 2.0;
            let y = node.y - node.height.max(24.0) / 2.0;
            elements.push(builder::ellipse(
                &Rect {
                    id: &id,
                    x,
                    y,
                    width: node.width.max(24.0),
                    height: node.height.max(24.0),
                    fill: Some(style::STATE_PSEUDO_FILL),
                    rounded: false,
                    frame_id: None,
                },
                options,
            ));
            shape_ids.insert(node.id.clone(), id);
            continue;
        }
        let label = node.id.replace('_', " ");
        let NodeOutput {
            shape_id,
            shape,
            text,
        } = node_with_text(
            node,
            NodeShape::Rectangle { rounded: true },
            &label,
            Some(style::NODE_FILL),
            None,
            &mut ids,
            options,
        );
        shape_ids.insert(node.id.clone(), shape_id);
        elements.push(shape);
        elements.push(text);
    }

    for edge in &layout.edges {
        let start_id = shape_ids.get(&edge.from).map(String::as_str);
        let end_id = shape_ids.get(&edge.to).map(String::as_str);
        let EdgeOutput {
            arrow_id, arrow, ..
        } = edge_with_label(
            edge,
            start_id,
            end_id,
            None,
            Some("arrow"),
            "solid",
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
        elements.push(arrow);
    }

    Ok(elements)
}
