use std::collections::HashMap;

use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::defaults::{
    default_background_color, default_file_type, default_fill_style, default_font_family,
    default_font_size, default_line_height, default_opacity, default_roughness,
    default_stroke_color, default_stroke_style, default_stroke_width, default_text_align,
    default_version, default_vertical_align,
};

/// Top-level `.excalidraw` payload.
#[derive(Debug, Clone, Deserialize)]
pub struct ExcalidrawFile {
    #[serde(default = "default_file_type", rename = "type")]
    pub file_type: String,

    #[serde(default = "default_version")]
    pub version: u32,

    #[serde(default)]
    pub source: Option<String>,

    #[serde(default)]
    pub elements: Vec<Element>,

    #[serde(default, rename = "appState")]
    pub app_state: AppState,

    #[serde(default)]
    pub files: HashMap<String, FileData>,

    /// Unknown top-level fields are preserved for forward compatibility.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Excalidraw application state fields relevant to rendering.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppState {
    #[serde(default, rename = "viewBackgroundColor")]
    pub view_background_color: Option<String>,

    #[serde(default, rename = "gridSize")]
    pub grid_size: Option<u32>,

    #[serde(default)]
    pub theme: Option<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Embedded file data referenced by image elements.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileData {
    #[serde(default, rename = "mimeType")]
    pub mime_type: String,

    #[serde(default)]
    pub id: String,

    #[serde(default, rename = "dataURL")]
    pub data_url: String,

    #[serde(default)]
    pub created: Option<u64>,

    #[serde(default, rename = "lastRetrieved", alias = "last_retrieved")]
    pub last_retrieved: Option<u64>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Shared fields present on Excalidraw elements.
#[derive(Debug, Clone, Deserialize)]
pub struct BaseElement {
    #[serde(default)]
    pub id: String,

    #[serde(default)]
    pub x: f64,

    #[serde(default)]
    pub y: f64,

    #[serde(default)]
    pub width: f64,

    #[serde(default)]
    pub height: f64,

    #[serde(default)]
    pub angle: f64,

    #[serde(default = "default_stroke_color", rename = "strokeColor")]
    pub stroke_color: String,

    #[serde(default = "default_background_color", rename = "backgroundColor")]
    pub background_color: String,

    #[serde(default = "default_fill_style", rename = "fillStyle")]
    pub fill_style: FillStyle,

    #[serde(default = "default_stroke_width", rename = "strokeWidth")]
    pub stroke_width: f64,

    #[serde(default = "default_stroke_style", rename = "strokeStyle")]
    pub stroke_style: StrokeStyle,

    #[serde(default = "default_roughness")]
    pub roughness: f64,

    #[serde(default = "default_opacity")]
    pub opacity: f64,

    #[serde(default)]
    pub seed: u64,

    #[serde(default, rename = "isDeleted")]
    pub is_deleted: bool,

    #[serde(default, rename = "groupIds")]
    pub group_ids: Vec<String>,

    #[serde(default, rename = "frameId")]
    pub frame_id: Option<String>,

    #[serde(default, rename = "boundElements")]
    pub bound_elements: Vec<BoundElement>,

    #[serde(default)]
    pub roundness: Option<Roundness>,

    #[serde(default)]
    pub version: u64,

    #[serde(default)]
    pub link: Option<String>,

    #[serde(default)]
    pub locked: bool,

    #[serde(default)]
    pub index: Option<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FillStyle {
    Hachure,
    Solid,
    #[serde(rename = "cross-hatch")]
    CrossHatch,
    Dots,
    Dashed,
    #[serde(rename = "zigzag-line")]
    ZigzagLine,
    None,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StrokeStyle {
    Solid,
    Dashed,
    Dotted,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Roundness {
    #[serde(default, rename = "type")]
    pub roundness_type: u32,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BoundElement {
    #[serde(default)]
    pub id: String,

    #[serde(default, rename = "type")]
    pub element_type: String,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShapeElement {
    #[serde(flatten)]
    pub base: BaseElement,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LinearElement {
    #[serde(flatten)]
    pub base: BaseElement,

    #[serde(default)]
    pub points: Vec<[f64; 2]>,

    #[serde(default, rename = "startArrowhead")]
    pub start_arrowhead: Option<Arrowhead>,

    #[serde(default, rename = "endArrowhead")]
    pub end_arrowhead: Option<Arrowhead>,

    #[serde(default, rename = "startBinding")]
    pub start_binding: Option<ArrowBinding>,

    #[serde(default, rename = "endBinding")]
    pub end_binding: Option<ArrowBinding>,

    #[serde(default)]
    pub elbowed: Option<bool>,

    #[serde(default, rename = "lastCommittedPoint")]
    pub last_committed_point: Option<[f64; 2]>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Arrowhead {
    Arrow,
    Triangle,
    Bar,
    Dot,
    Circle,
    Diamond,
    #[serde(rename = "triangle_outline")]
    TriangleOutline,
    Crowfoot,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArrowBinding {
    #[serde(default, rename = "elementId")]
    pub element_id: String,

    #[serde(default, rename = "fixedPoint")]
    pub fixed_point: Option<[f64; 2]>,

    #[serde(default)]
    pub mode: Option<String>,

    #[serde(default)]
    pub focus: Option<f64>,

    #[serde(default)]
    pub gap: Option<f64>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TextElement {
    #[serde(flatten)]
    pub base: BaseElement,

    #[serde(default)]
    pub text: String,

    #[serde(default, rename = "originalText")]
    pub original_text: Option<String>,

    #[serde(default = "default_font_size", rename = "fontSize")]
    pub font_size: f64,

    #[serde(default = "default_font_family", rename = "fontFamily")]
    pub font_family: u32,

    #[serde(default = "default_text_align", rename = "textAlign")]
    pub text_align: TextAlign,

    #[serde(default = "default_vertical_align", rename = "verticalAlign")]
    pub vertical_align: VerticalAlign,

    #[serde(default, rename = "containerId")]
    pub container_id: Option<String>,

    #[serde(default = "default_line_height", rename = "lineHeight")]
    pub line_height: f64,

    #[serde(default, rename = "autoResize")]
    pub auto_resize: Option<bool>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign {
    Left,
    Center,
    Right,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VerticalAlign {
    Top,
    Middle,
    Bottom,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FreedrawElement {
    #[serde(flatten)]
    pub base: BaseElement,

    #[serde(default)]
    pub points: Vec<[f64; 2]>,

    #[serde(default)]
    pub pressures: Vec<f64>,

    #[serde(default, rename = "simulatePressure")]
    pub simulate_pressure: bool,

    #[serde(default, rename = "lastCommittedPoint")]
    pub last_committed_point: Option<[f64; 2]>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageElement {
    #[serde(flatten)]
    pub base: BaseElement,

    #[serde(default, rename = "fileId")]
    pub file_id: Option<String>,

    #[serde(default)]
    pub status: String,

    #[serde(default)]
    pub scale: Option<[f64; 2]>,

    #[serde(default)]
    pub crop: Option<ImageCrop>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageCrop {
    #[serde(default)]
    pub x: f64,

    #[serde(default)]
    pub y: f64,

    #[serde(default)]
    pub width: f64,

    #[serde(default)]
    pub height: f64,

    #[serde(default, rename = "naturalWidth")]
    pub natural_width: f64,

    #[serde(default, rename = "naturalHeight")]
    pub natural_height: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FrameElement {
    #[serde(flatten)]
    pub base: BaseElement,

    #[serde(default)]
    pub name: Option<String>,

    #[serde(default, rename = "isCollapsed")]
    pub is_collapsed: Option<bool>,

    #[serde(default)]
    pub clip: Option<bool>,
}

/// Excalidraw element variants known to this renderer.
#[derive(Debug, Clone)]
pub enum Element {
    Rectangle(ShapeElement),
    Ellipse(ShapeElement),
    Diamond(ShapeElement),
    Arrow(LinearElement),
    Line(LinearElement),
    Text(TextElement),
    Freedraw(FreedrawElement),
    Image(ImageElement),
    Frame(FrameElement),
    MagicFrame(FrameElement),
    Embeddable(UnsupportedElement),
    Iframe(UnsupportedElement),
    Unknown { element_type: String, raw: Value },
}

#[derive(Debug, Clone, Deserialize)]
pub struct UnsupportedElement {
    #[serde(flatten)]
    pub base: BaseElement,

    #[serde(flatten)]
    pub raw: HashMap<String, Value>,
}

impl<'de> Deserialize<'de> for Element {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = Value::deserialize(deserializer)?;
        let element_type = match raw.get("type").and_then(Value::as_str) {
            Some(element_type) => String::from(element_type),
            None => String::from("unknown"),
        };

        match element_type.as_str() {
            "rectangle" => deserialize_element(raw, Element::Rectangle),
            "ellipse" => deserialize_element(raw, Element::Ellipse),
            "diamond" => deserialize_element(raw, Element::Diamond),
            "arrow" => deserialize_element(raw, Element::Arrow),
            "line" => deserialize_element(raw, Element::Line),
            "text" => deserialize_element(raw, Element::Text),
            "freedraw" => deserialize_element(raw, Element::Freedraw),
            "image" => deserialize_element(raw, Element::Image),
            "frame" => deserialize_element(raw, Element::Frame),
            "magicframe" => deserialize_element(raw, Element::MagicFrame),
            "embeddable" => deserialize_element(raw, Element::Embeddable),
            "iframe" => deserialize_element(raw, Element::Iframe),
            _ => Ok(Element::Unknown { element_type, raw }),
        }
    }
}

fn deserialize_element<T, F, E>(raw: Value, wrap: F) -> Result<Element, E>
where
    T: for<'de> Deserialize<'de>,
    F: FnOnce(T) -> Element,
    E: serde::de::Error,
{
    serde_json::from_value(raw).map(wrap).map_err(E::custom)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use serde_json::json;

    use super::{
        Arrowhead, Element, ExcalidrawFile, FillStyle, StrokeStyle, TextAlign, VerticalAlign,
    };

    #[test]
    fn deserializes_minimal_file_with_defaults() -> Result<(), Box<dyn Error>> {
        let file: ExcalidrawFile = serde_json::from_value(json!({
            "elements": [
                {
                    "type": "rectangle",
                    "id": "rect-1",
                    "customElementField": true
                }
            ],
            "futureTopLevelField": "kept"
        }))?;

        ensure_eq(&file.file_type, "excalidraw", "default file type")?;
        ensure_eq(&file.version, 2_u32, "default version")?;
        ensure_eq(
            &file.extra.get("futureTopLevelField"),
            Some(&json!("kept")),
            "top-level unknown field",
        )?;

        let [Element::Rectangle(rect)] = file.elements.as_slice() else {
            return Err("rectangle should deserialize as a rectangle element".into());
        };

        ensure_eq(&rect.base.id, "rect-1", "rectangle id")?;
        ensure_eq(&rect.base.stroke_color, "#1e1e1e", "default stroke color")?;
        ensure_eq(
            &rect.base.background_color,
            "transparent",
            "default background color",
        )?;
        ensure_eq(
            &rect.base.fill_style,
            FillStyle::Hachure,
            "default fill style",
        )?;
        ensure_eq(
            &rect.base.stroke_style,
            StrokeStyle::Solid,
            "default stroke style",
        )?;
        ensure_eq(
            &rect.base.extra.get("customElementField"),
            Some(&json!(true)),
            "element unknown field",
        )?;
        Ok(())
    }

    #[test]
    fn preserves_unknown_element_raw_json() -> Result<(), Box<dyn Error>> {
        let file: ExcalidrawFile = serde_json::from_value(json!({
            "type": "excalidraw",
            "version": 2,
            "elements": [
                {
                    "type": "future-shape",
                    "id": "future-1",
                    "x": 10,
                    "futurePayload": {"nested": true}
                }
            ]
        }))?;

        let [Element::Unknown { element_type, raw }] = file.elements.as_slice() else {
            return Err("future element type should remain raw unknown JSON".into());
        };

        ensure_eq(element_type, "future-shape", "unknown element type")?;
        ensure_eq(&raw.get("id"), Some(&json!("future-1")), "unknown raw id")?;
        ensure_eq(
            &raw.get("futurePayload")
                .and_then(|payload| payload.get("nested")),
            Some(&json!(true)),
            "unknown nested raw field",
        )?;
        Ok(())
    }

    #[test]
    fn unsupported_browser_elements_are_placeholder_capable() -> Result<(), Box<dyn Error>> {
        let file: ExcalidrawFile = serde_json::from_value(json!({
            "elements": [
                {"type": "embeddable", "id": "embed-1", "x": 1, "link": "https://example.test"},
                {"type": "iframe", "id": "iframe-1", "width": 200, "height": 100}
            ]
        }))?;

        let [Element::Embeddable(embed), Element::Iframe(iframe)] = file.elements.as_slice() else {
            return Err("unsupported browser elements should deserialize".into());
        };

        ensure_eq(&embed.base.id, "embed-1", "embeddable id")?;
        ensure_eq(
            &embed.base.link.as_deref(),
            Some("https://example.test"),
            "embeddable link",
        )?;
        ensure_eq(&iframe.base.id, "iframe-1", "iframe id")?;
        ensure_eq(&iframe.base.width, 200.0, "iframe width")?;
        ensure_eq(&iframe.base.height, 100.0, "iframe height")?;
        Ok(())
    }

    #[test]
    fn deserializes_known_element_specific_fields() -> Result<(), Box<dyn Error>> {
        let file: ExcalidrawFile = serde_json::from_value(json!({
            "elements": [
                {
                    "type": "arrow",
                    "id": "arrow-1",
                    "points": [[0, 0], [10, 10]],
                    "endArrowhead": "triangle_outline",
                    "endBinding": {"elementId": "rect-1", "gap": 4}
                },
                {
                    "type": "text",
                    "id": "text-1",
                    "text": "Hello",
                    "fontFamily": 3,
                    "textAlign": "center",
                    "verticalAlign": "middle"
                },
                {
                    "type": "image",
                    "id": "image-1",
                    "fileId": "file-1",
                    "scale": [-1, 1],
                    "crop": {"x": 1, "y": 2, "width": 3, "height": 4}
                },
                {
                    "type": "frame",
                    "id": "frame-1",
                    "name": "Frame",
                    "clip": true
                }
            ],
            "files": {
                "file-1": {
                    "mimeType": "image/png",
                    "id": "file-1",
                    "dataURL": "data:image/png;base64,AA=="
                }
            }
        }))?;

        let [Element::Arrow(arrow), Element::Text(text), Element::Image(image), Element::Frame(frame)] =
            file.elements.as_slice()
        else {
            return Err("known element fields should deserialize to expected variants".into());
        };

        ensure_eq(
            &arrow.points.as_slice(),
            [[0.0, 0.0], [10.0, 10.0]].as_slice(),
            "arrow points",
        )?;
        ensure_eq(
            &arrow.end_arrowhead,
            Some(Arrowhead::TriangleOutline),
            "arrowhead",
        )?;
        ensure_eq(
            &arrow
                .end_binding
                .as_ref()
                .map(|binding| binding.element_id.as_str()),
            Some("rect-1"),
            "arrow binding element id",
        )?;

        ensure_eq(&text.text, "Hello", "text content")?;
        ensure_eq(&text.font_family, 3_u32, "font family")?;
        ensure_eq(&text.text_align, TextAlign::Center, "text alignment")?;
        ensure_eq(
            &text.vertical_align,
            VerticalAlign::Middle,
            "vertical alignment",
        )?;

        ensure_eq(&image.file_id.as_deref(), Some("file-1"), "image file id")?;
        ensure_eq(&image.scale, Some([-1.0, 1.0]), "image scale")?;
        ensure_eq(
            &image.crop.as_ref().map(|crop| crop.width),
            Some(3.0),
            "image crop width",
        )?;

        ensure_eq(&frame.name.as_deref(), Some("Frame"), "frame name")?;
        ensure_eq(&frame.clip, Some(true), "frame clip")?;
        ensure_eq(
            &file.files.get("file-1").map(|data| data.mime_type.as_str()),
            Some("image/png"),
            "file mime type",
        )?;
        Ok(())
    }

    fn ensure_eq<T, U>(actual: &T, expected: U, label: &str) -> Result<(), Box<dyn Error>>
    where
        T: PartialEq<U> + std::fmt::Debug,
        U: std::fmt::Debug,
    {
        if actual.eq(&expected) {
            Ok(())
        } else {
            Err(format!("{label}: expected {expected:?}, got {actual:?}").into())
        }
    }
}
