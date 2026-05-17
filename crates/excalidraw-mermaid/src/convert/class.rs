//! Class diagram → Excalidraw mapping.
//!
//! Class layouts give us node geometry but the member/method lists are stored
//! in the semantic JSON. We render each class as a single Excalidraw rectangle
//! with the class name + member text inside.

use std::collections::HashMap;

use merman_render::model::ClassDiagramV2Layout;
use serde_json::Value;

use crate::builder;
use crate::convert::common::{
    edge_with_label, node_with_text, truncate, EdgeOutput, IdGen, NodeOutput, NodeShape,
};
use crate::error::MermaidConvertError;
use crate::options::MermaidConvertOptions;
use crate::style;

pub fn convert(
    layout: &ClassDiagramV2Layout,
    semantic: &Value,
    options: &MermaidConvertOptions,
) -> Result<Vec<Value>, MermaidConvertError> {
    let mut ids = IdGen::new("mm-class");
    let mut elements: Vec<Value> = Vec::new();
    let mut shape_ids: HashMap<String, String> = HashMap::new();
    let class_info = index_classes(semantic);

    for node in &layout.nodes {
        if node.is_cluster {
            continue;
        }
        let info = class_info
            .get(node.id.as_str())
            .cloned()
            .unwrap_or_default();
        let label = render_class_label(&info, options);
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
        let EdgeOutput {
            arrow_id, arrow, ..
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
        bind_shape_arrow(&mut elements, start_id, &arrow_id);
        bind_shape_arrow(&mut elements, end_id, &arrow_id);
        elements.push(arrow);
    }

    Ok(elements)
}

fn bind_shape_arrow(elements: &mut [Value], shape_id: Option<&str>, arrow_id: &str) {
    let Some(shape_id) = shape_id else {
        return;
    };
    if let Some(value) = elements
        .iter_mut()
        .find(|el| el.get("id").and_then(Value::as_str) == Some(shape_id))
    {
        builder::bind_arrow(value, arrow_id);
    }
}

#[derive(Debug, Default, Clone)]
struct ClassInfo {
    name: String,
    members: Vec<String>,
    methods: Vec<String>,
}

fn render_class_label(info: &ClassInfo, options: &MermaidConvertOptions) -> String {
    let mut out = if info.name.is_empty() {
        String::new()
    } else {
        info.name.clone()
    };
    for member in &info.members {
        out.push('\n');
        out.push_str(member);
    }
    if !info.methods.is_empty() {
        out.push_str("\n---");
    }
    for method in &info.methods {
        out.push('\n');
        out.push_str(method);
    }
    truncate(&out, options.max_text_size)
}

fn index_classes(semantic: &Value) -> HashMap<String, ClassInfo> {
    let mut map = HashMap::new();
    let arr = semantic
        .get("classes")
        .or_else(|| semantic.get("nodes"))
        .and_then(Value::as_array);
    if let Some(arr) = arr {
        for class in arr {
            let id = class
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let name = class
                .get("name")
                .or_else(|| class.get("label"))
                .or_else(|| class.get("id"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let members = extract_string_list(class, "members");
            let methods = extract_string_list(class, "methods");
            map.insert(
                id,
                ClassInfo {
                    name,
                    members,
                    methods,
                },
            );
        }
    }
    map
}

fn extract_string_list(value: &Value, key: &str) -> Vec<String> {
    let Some(arr) = value.get(key).and_then(Value::as_array) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|item| match item {
            Value::String(s) => Some(s.clone()),
            Value::Object(map) => map
                .get("text")
                .or_else(|| map.get("signature"))
                .or_else(|| map.get("name"))
                .and_then(Value::as_str)
                .map(str::to_owned),
            _ => None,
        })
        .collect()
}
