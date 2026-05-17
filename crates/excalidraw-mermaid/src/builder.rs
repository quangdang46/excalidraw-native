//! Small helpers for constructing Excalidraw element JSON values without
//! depending on serialisable types from `excalidraw-core`.
//!
//! Each builder returns a `serde_json::Value` carrying a minimal-but-complete
//! element shape so the document round-trips through
//! `excalidraw_core::parse_str`.

use serde_json::{json, Value};

use crate::options::MermaidConvertOptions;
use crate::style;

/// Stable seed so that repeated conversions produce identical output. The
/// renderer hashes element ids, so we don't need per-element randomness.
const STABLE_SEED: u64 = 1_337;

/// Build a base element JSON object with the common Excalidraw fields filled
/// in. Callers extend the resulting object with type-specific fields.
fn base(
    element_type: &str,
    id: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    opts: &MermaidConvertOptions,
) -> Value {
    let _ = opts;
    json!({
        "type": element_type,
        "id": id,
        "x": x,
        "y": y,
        "width": width,
        "height": height,
        "angle": 0.0,
        "strokeColor": style::STROKE,
        "backgroundColor": "transparent",
        "fillStyle": "solid",
        "strokeWidth": style::STROKE_WIDTH,
        "strokeStyle": "solid",
        "roughness": style::ROUGHNESS,
        "opacity": 100.0,
        "seed": STABLE_SEED,
        "version": 1,
        "versionNonce": 0,
        "isDeleted": false,
        "groupIds": [],
        "frameId": Value::Null,
        "boundElements": [],
        "updated": 0,
        "link": Value::Null,
        "locked": false,
    })
}

fn ensure_filled(value: &mut Value, fill: &str, fill_style: &str) {
    if let Value::Object(map) = value {
        map.insert("backgroundColor".to_string(), Value::String(fill.into()));
        map.insert("fillStyle".to_string(), Value::String(fill_style.into()));
    }
}

fn ensure_round(value: &mut Value, kind: u32) {
    if let Value::Object(map) = value {
        map.insert(
            "roundness".to_string(),
            json!({
                "type": kind
            }),
        );
    }
}

fn ensure_frame_parent(value: &mut Value, frame_id: Option<&str>) {
    if let (Value::Object(map), Some(parent)) = (value, frame_id) {
        map.insert("frameId".to_string(), Value::String(parent.into()));
    }
}

/// A rectangular Excalidraw shape (rectangle / ellipse / diamond).
pub struct Rect<'a> {
    pub id: &'a str,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub fill: Option<&'a str>,
    pub rounded: bool,
    pub frame_id: Option<&'a str>,
}

/// Build a rectangle element.
#[must_use]
pub fn rectangle(spec: &Rect, opts: &MermaidConvertOptions) -> Value {
    let mut value = base(
        "rectangle",
        spec.id,
        spec.x,
        spec.y,
        spec.width,
        spec.height,
        opts,
    );
    if let Some(fill) = spec.fill {
        ensure_filled(&mut value, fill, style::fill_style(opts));
    }
    if spec.rounded {
        ensure_round(&mut value, style::ROUND_RECT);
    }
    ensure_frame_parent(&mut value, spec.frame_id);
    value
}

/// Build an ellipse element using the same spec as [`rectangle`].
#[must_use]
pub fn ellipse(spec: &Rect, opts: &MermaidConvertOptions) -> Value {
    let mut value = base(
        "ellipse",
        spec.id,
        spec.x,
        spec.y,
        spec.width,
        spec.height,
        opts,
    );
    if let Some(fill) = spec.fill {
        ensure_filled(&mut value, fill, style::fill_style(opts));
    }
    ensure_frame_parent(&mut value, spec.frame_id);
    value
}

/// Build a diamond element using the same spec as [`rectangle`].
#[must_use]
pub fn diamond(spec: &Rect, opts: &MermaidConvertOptions) -> Value {
    let mut value = base(
        "diamond",
        spec.id,
        spec.x,
        spec.y,
        spec.width,
        spec.height,
        opts,
    );
    if let Some(fill) = spec.fill {
        ensure_filled(&mut value, fill, style::fill_style(opts));
    }
    ensure_frame_parent(&mut value, spec.frame_id);
    value
}

/// Build a frame element used for subgraphs / clusters.
#[must_use]
pub fn frame(
    id: &str,
    name: Option<&str>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    opts: &MermaidConvertOptions,
) -> Value {
    let mut value = base("frame", id, x, y, width, height, opts);
    if let Value::Object(map) = &mut value {
        if let Some(name) = name {
            map.insert("name".to_string(), Value::String(name.to_string()));
        } else {
            map.insert("name".to_string(), Value::Null);
        }
        map.insert("clip".to_string(), Value::Bool(false));
        map.insert("isCollapsed".to_string(), Value::Bool(false));
    }
    value
}

/// Spec for a text element.
pub struct Text<'a> {
    pub id: &'a str,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub text: &'a str,
    pub font_size: f64,
    pub align: &'a str,
    pub container_id: Option<&'a str>,
    pub frame_id: Option<&'a str>,
}

/// Build a text element.
#[must_use]
pub fn text(spec: &Text, opts: &MermaidConvertOptions) -> Value {
    let mut value = base(
        "text",
        spec.id,
        spec.x,
        spec.y,
        spec.width,
        spec.height,
        opts,
    );
    if let Value::Object(map) = &mut value {
        map.insert("text".to_string(), Value::String(spec.text.into()));
        map.insert("originalText".to_string(), Value::String(spec.text.into()));
        map.insert("fontSize".to_string(), json!(spec.font_size.max(8.0)));
        map.insert("fontFamily".to_string(), json!(5));
        map.insert("textAlign".to_string(), Value::String(spec.align.into()));
        map.insert("verticalAlign".to_string(), Value::String("middle".into()));
        map.insert("lineHeight".to_string(), json!(style::LINE_HEIGHT));
        map.insert("autoResize".to_string(), Value::Bool(true));
        if let Some(cid) = spec.container_id {
            map.insert("containerId".to_string(), Value::String(cid.into()));
        } else {
            map.insert("containerId".to_string(), Value::Null);
        }
    }
    ensure_frame_parent(&mut value, spec.frame_id);
    value
}

/// Spec for an arrow / line element.
pub struct Arrow<'a> {
    pub id: &'a str,
    pub points: Vec<(f64, f64)>,
    pub start_arrowhead: Option<&'a str>,
    pub end_arrowhead: Option<&'a str>,
    pub start_binding: Option<&'a str>,
    pub end_binding: Option<&'a str>,
    pub stroke_style: &'a str,
    pub frame_id: Option<&'a str>,
}

/// Build an arrow element. Points are absolute (x, y) coordinates; they are
/// translated into the Excalidraw point space (origin = start of arrow) here.
#[must_use]
pub fn arrow(spec: &Arrow, opts: &MermaidConvertOptions) -> Value {
    let (origin_x, origin_y) = spec.points.first().copied().unwrap_or((0.0, 0.0));
    let mut points: Vec<Value> = Vec::with_capacity(spec.points.len());
    for (x, y) in &spec.points {
        points.push(json!([x - origin_x, y - origin_y]));
    }
    let xs = spec.points.iter().map(|(x, _)| *x);
    let ys = spec.points.iter().map(|(_, y)| *y);
    let min_x = xs.clone().fold(origin_x, f64::min);
    let max_x = xs.fold(origin_x, f64::max);
    let min_y = ys.clone().fold(origin_y, f64::min);
    let max_y = ys.fold(origin_y, f64::max);
    let width = (max_x - min_x).max(0.0);
    let height = (max_y - min_y).max(0.0);

    let mut value = base("arrow", spec.id, origin_x, origin_y, width, height, opts);
    if let Value::Object(map) = &mut value {
        map.insert("points".to_string(), Value::Array(points));
        map.insert(
            "strokeStyle".to_string(),
            Value::String(spec.stroke_style.into()),
        );
        map.insert(
            "startArrowhead".to_string(),
            spec.start_arrowhead
                .map(|s| Value::String(s.into()))
                .unwrap_or(Value::Null),
        );
        map.insert(
            "endArrowhead".to_string(),
            spec.end_arrowhead
                .map(|s| Value::String(s.into()))
                .unwrap_or(Value::Null),
        );
        map.insert(
            "startBinding".to_string(),
            spec.start_binding
                .map(|id| {
                    json!({
                        "elementId": id,
                        "focus": 0.0,
                        "gap": 4.0,
                    })
                })
                .unwrap_or(Value::Null),
        );
        map.insert(
            "endBinding".to_string(),
            spec.end_binding
                .map(|id| {
                    json!({
                        "elementId": id,
                        "focus": 0.0,
                        "gap": 4.0,
                    })
                })
                .unwrap_or(Value::Null),
        );
        map.insert("elbowed".to_string(), Value::Bool(false));
        map.insert("lastCommittedPoint".to_string(), Value::Null);
    }
    ensure_frame_parent(&mut value, spec.frame_id);
    value
}

/// Append a `boundElements` entry to a shape so the renderer knows to draw
/// the text inside the container.
pub fn bind_text(container: &mut Value, text_id: &str) {
    let Value::Object(map) = container else {
        return;
    };
    let entry = json!({
        "id": text_id,
        "type": "text",
    });
    match map.entry("boundElements".to_string()) {
        serde_json::map::Entry::Occupied(mut occ) => {
            if let Value::Array(list) = occ.get_mut() {
                list.push(entry);
            } else {
                occ.insert(Value::Array(vec![entry]));
            }
        }
        serde_json::map::Entry::Vacant(vac) => {
            vac.insert(Value::Array(vec![entry]));
        }
    }
}

/// Append a bound arrow entry to a shape.
pub fn bind_arrow(container: &mut Value, arrow_id: &str) {
    let Value::Object(map) = container else {
        return;
    };
    let entry = json!({
        "id": arrow_id,
        "type": "arrow",
    });
    match map.entry("boundElements".to_string()) {
        serde_json::map::Entry::Occupied(mut occ) => {
            if let Value::Array(list) = occ.get_mut() {
                list.push(entry);
            } else {
                occ.insert(Value::Array(vec![entry]));
            }
        }
        serde_json::map::Entry::Vacant(vac) => {
            vac.insert(Value::Array(vec![entry]));
        }
    }
}

/// Build a full `.excalidraw` file document around a vector of element values.
#[must_use]
pub fn build_document(elements: Vec<Value>, diagram_type: &str) -> Value {
    json!({
        "type": "excalidraw",
        "version": 2,
        "source": format!("excalidraw-mermaid:{diagram_type}"),
        "elements": elements,
        "appState": {
            "viewBackgroundColor": "#ffffff",
            "gridSize": 20,
            "theme": "light",
        },
        "files": {},
    })
}
