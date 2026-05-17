//! ER diagram → Excalidraw mapping.
//!
//! Each entity becomes a rectangle with the entity name + attribute list as
//! the bound text. Relationships become arrows whose start/end arrowheads
//! reflect the Mermaid cardinality markers (`one`, `many`, `optional`, ...).

use std::collections::HashMap;

use merman_render::model::ErDiagramLayout;
use serde_json::Value;

use crate::builder;
use crate::convert::common::{
    edge_with_label, node_with_text, truncate, EdgeOutput, IdGen, NodeOutput, NodeShape,
};
use crate::error::MermaidConvertError;
use crate::options::MermaidConvertOptions;
use crate::style;

pub fn convert(
    layout: &ErDiagramLayout,
    semantic: &Value,
    options: &MermaidConvertOptions,
) -> Result<Vec<Value>, MermaidConvertError> {
    let mut ids = IdGen::new("mm-er");
    let mut elements: Vec<Value> = Vec::new();
    let mut shape_ids: HashMap<String, String> = HashMap::new();
    let entities = index_entities(semantic);

    for node in &layout.nodes {
        if node.is_cluster {
            continue;
        }
        let label = entities
            .get(node.id.as_str())
            .map(|entity| render_entity_label(entity, options))
            .unwrap_or_else(|| node.id.replace('_', " "));
        let NodeOutput {
            shape_id,
            shape,
            text,
        } = node_with_text(
            node,
            NodeShape::Rectangle { rounded: false },
            &label,
            Some(style::HEADER_FILL),
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
        let stroke_style = edge
            .stroke_dasharray
            .as_deref()
            .map(|_| "dashed")
            .unwrap_or("solid");
        let start_arrowhead = edge.start_marker.as_deref().map(map_marker_to_arrowhead);
        let end_arrowhead = edge.end_marker.as_deref().map(map_marker_to_arrowhead);
        let EdgeOutput {
            arrow_id, arrow, ..
        } = edge_with_label(
            edge,
            start_id,
            end_id,
            start_arrowhead,
            end_arrowhead.or(Some("arrow")),
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
        elements.push(arrow);
    }

    Ok(elements)
}

fn map_marker_to_arrowhead(marker: &str) -> &'static str {
    let lower = marker.to_ascii_lowercase();
    if lower.contains("zero_or_one") || lower.contains("one_or_zero") {
        "circle"
    } else if lower.contains("zero_or_many") || lower.contains("many_or_zero") {
        "circle"
    } else if lower.contains("only_one") || lower.ends_with("_one") || lower == "one" {
        "bar"
    } else if lower.contains("many") {
        "crowfoot"
    } else {
        "arrow"
    }
}

#[derive(Debug, Default, Clone)]
struct EntityInfo {
    name: String,
    attributes: Vec<String>,
}

fn render_entity_label(info: &EntityInfo, options: &MermaidConvertOptions) -> String {
    let mut out = info.name.clone();
    for attr in &info.attributes {
        out.push('\n');
        out.push_str(attr);
    }
    truncate(&out, options.max_text_size)
}

fn index_entities(semantic: &Value) -> HashMap<String, EntityInfo> {
    let mut map = HashMap::new();
    let entries = semantic
        .get("entities")
        .or_else(|| semantic.get("nodes"))
        .and_then(Value::as_array);
    if let Some(entries) = entries {
        for entity in entries {
            let id = entity
                .get("id")
                .or_else(|| entity.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let name = entity
                .get("name")
                .or_else(|| entity.get("label"))
                .or_else(|| entity.get("id"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let attributes = entity
                .get("attributes")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| {
                            if let Value::String(s) = item {
                                Some(s.clone())
                            } else if let Value::Object(map) = item {
                                let name = map
                                    .get("name")
                                    .or_else(|| map.get("attributeName"))
                                    .and_then(Value::as_str)
                                    .unwrap_or("");
                                let ty = map
                                    .get("type")
                                    .or_else(|| map.get("attributeType"))
                                    .and_then(Value::as_str)
                                    .unwrap_or("");
                                if name.is_empty() && ty.is_empty() {
                                    None
                                } else {
                                    Some(format!("{ty} {name}").trim().to_string())
                                }
                            } else {
                                None
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            map.insert(id, EntityInfo { name, attributes });
        }
    }
    map
}
