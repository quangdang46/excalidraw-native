//! Flowchart → Excalidraw mapping.

use std::collections::HashMap;

use merman_render::model::{FlowchartV2Layout, LayoutCluster};
use serde_json::Value;

use crate::builder;
use crate::convert::common::{
    self, edge_with_label, node_with_text, update_text_box, EdgeOutput, IdGen, NodeOutput,
    NodeShape,
};
use crate::error::MermaidConvertError;
use crate::options::MermaidConvertOptions;
use crate::style;

pub fn convert(
    layout: &FlowchartV2Layout,
    semantic: &Value,
    options: &MermaidConvertOptions,
) -> Result<Vec<Value>, MermaidConvertError> {
    let mut ids = IdGen::new("mm-flow");
    let mut elements: Vec<Value> = Vec::new();
    let mut shape_ids: HashMap<String, String> = HashMap::new();
    let semantic_nodes = index_semantic_nodes(semantic);
    let semantic_edges = index_semantic_edges(semantic);
    let cluster_ids = emit_clusters(&layout.clusters, &mut ids, options, &mut elements);

    for node in &layout.nodes {
        if node.is_cluster {
            continue;
        }
        let info = semantic_nodes
            .get(node.id.as_str())
            .copied()
            .map(parse_node_info)
            .unwrap_or_default();
        let label = info.label.as_deref().unwrap_or(node.id.as_str());
        let shape_kind = info.shape;
        let fill = if matches!(shape_kind, NodeShape::Diamond) {
            Some("#fff4e6")
        } else {
            Some(common::node_fill())
        };
        let frame_id = cluster_ids.get(node.id.as_str()).map(String::as_str);
        let NodeOutput {
            shape_id,
            shape,
            text,
        } = node_with_text(node, shape_kind, label, fill, frame_id, &mut ids, options);
        shape_ids.insert(node.id.clone(), shape_id);
        elements.push(shape);
        elements.push(text);
    }

    for edge in &layout.edges {
        let start_id = shape_ids.get(&edge.from).map(String::as_str);
        let end_id = shape_ids.get(&edge.to).map(String::as_str);
        let edge_info = semantic_edges.get(&edge.id);
        let (end_arrowhead, stroke_style) = edge_info
            .map(arrowhead_for_edge)
            .unwrap_or(("arrow", "solid"));

        let EdgeOutput {
            arrow_id,
            arrow,
            label,
        } = edge_with_label(
            edge,
            start_id,
            end_id,
            None,
            Some(end_arrowhead),
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

        // Edge label content from semantic JSON.
        if let Some(label) = label {
            if let Some(text_value) = edge_info.and_then(|info| info.label.clone()) {
                let mut label_text = label.text;
                update_text_box(
                    &mut label_text,
                    label.x,
                    label.y,
                    label.width,
                    label.height,
                    &text_value,
                );
                elements.push(label_text);
            }
        }
        let _ = options;
        elements.push(arrow);
    }

    let _ = style::CLUSTER_FILL;
    let _ = options;
    Ok(elements)
}

fn emit_clusters(
    clusters: &[LayoutCluster],
    ids: &mut IdGen,
    options: &MermaidConvertOptions,
    elements: &mut Vec<Value>,
) -> HashMap<String, String> {
    let mut mapping = HashMap::new();
    for cluster in clusters {
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
        mapping.insert(cluster.id.clone(), frame_id);
    }
    mapping
}

#[derive(Debug, Default, Clone)]
struct NodeInfo {
    label: Option<String>,
    shape: NodeShape,
}

impl Default for NodeShape {
    fn default() -> Self {
        NodeShape::Rectangle { rounded: false }
    }
}

fn parse_node_info(value: &Value) -> NodeInfo {
    let label = value
        .get("label")
        .or_else(|| value.get("text"))
        .or_else(|| value.get("name"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let shape_name = value.get("shape").and_then(Value::as_str).unwrap_or("rect");
    let shape = match shape_name {
        // Diamond / rhombus
        "diam" | "diamond" | "rhombus" | "decision" | "question" | "fork" => NodeShape::Diamond,
        // Ellipses and circles
        "circle" | "circ" | "doublecircle" | "double_circle" | "stadium" | "stad" | "rounded" => {
            NodeShape::Ellipse
        }
        "ellipse" | "lin-cyl" | "cyl" | "cylinder" => NodeShape::Ellipse,
        // Rounded rectangle family
        "round" | "rounded_rect" | "pill" | "stadium-rounded" | "subroutine" | "subproc" => {
            NodeShape::Rectangle { rounded: true }
        }
        // Default: plain rectangle
        _ => NodeShape::Rectangle { rounded: false },
    };
    NodeInfo { label, shape }
}

fn arrowhead_for_edge(info: &EdgeInfo) -> (&'static str, &'static str) {
    let head = match info.arrowhead.as_deref() {
        Some("arrow_circle" | "circle") => "circle",
        Some("arrow_cross" | "cross") => "bar",
        _ => "arrow",
    };
    let stroke_style = match info.stroke.as_deref() {
        Some("dotted") => "dotted",
        Some("dashed") => "dashed",
        _ => "solid",
    };
    (head, stroke_style)
}

#[derive(Debug, Default, Clone)]
struct EdgeInfo {
    label: Option<String>,
    arrowhead: Option<String>,
    stroke: Option<String>,
}

fn index_semantic_nodes(semantic: &Value) -> HashMap<&str, &Value> {
    let mut map = HashMap::new();
    let arr = semantic.get("nodes").and_then(Value::as_array);
    if let Some(arr) = arr {
        for node in arr {
            if let Some(id) = node.get("id").and_then(Value::as_str) {
                map.insert(id, node);
            }
        }
    }
    map
}

fn index_semantic_edges(semantic: &Value) -> HashMap<String, EdgeInfo> {
    let mut map = HashMap::new();
    let arr = semantic.get("edges").and_then(Value::as_array);
    if let Some(arr) = arr {
        for (idx, edge) in arr.iter().enumerate() {
            let info = EdgeInfo {
                label: edge
                    .get("text")
                    .or_else(|| edge.get("label"))
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                arrowhead: edge
                    .get("arrowhead")
                    .or_else(|| edge.get("arrowTypeEnd"))
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                stroke: edge
                    .get("stroke")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            };
            // Merman's render layout uses synthetic edge ids of the form
            // `L_<from>_<to>_<index>` so we map both ways.
            let from = edge
                .get("start")
                .or_else(|| edge.get("from"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let to = edge
                .get("end")
                .or_else(|| edge.get("to"))
                .and_then(Value::as_str)
                .unwrap_or("");
            map.insert(format!("L_{from}_{to}_{idx}"), info.clone());
            map.insert(format!("{from}-{to}-{idx}"), info.clone());
            if let Some(id) = edge.get("id").and_then(Value::as_str) {
                map.insert(id.to_string(), info.clone());
            }
        }
    }
    map
}
