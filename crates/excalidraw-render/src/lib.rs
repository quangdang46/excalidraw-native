//! Rendering backend for normalized Excalidraw scenes.
//!
//! This crate will own SVG/PNG generation, rough-rs integration, text layout,
//! image handling, frames, and render warnings.

use std::collections::HashSet;

use excalidraw_core::{Color, Element, Rect, Scene};
use thiserror::Error;

/// Current crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the canonical crate boundary for diagnostics and smoke tests.
#[must_use]
pub fn crate_boundary() -> &'static str {
    "svg-png-render"
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackgroundMode {
    FromFile,
    Transparent,
    Override(Color),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderQuality {
    Full,
    FastSvg,
    Clean,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedElementMode {
    Placeholder,
    Skip,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePolicy {
    Embed,
    Placeholder,
    Skip,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextPolicy {
    SvgText,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderOptions {
    pub scale: f64,
    pub padding: f64,
    pub background: BackgroundMode,
    pub quality: RenderQuality,
    pub unsupported: UnsupportedElementMode,
    pub image_policy: ImagePolicy,
    pub text_policy: TextPolicy,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            scale: 1.0,
            padding: 16.0,
            background: BackgroundMode::FromFile,
            quality: RenderQuality::Full,
            unsupported: UnsupportedElementMode::Placeholder,
            image_policy: ImagePolicy::Embed,
            text_policy: TextPolicy::SvgText,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOutput<T> {
    pub value: T,
    pub warnings: Vec<RenderWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderWarning {
    UnsupportedElementPlaceholder {
        element_id: String,
        element_type: String,
    },
    UnsupportedElementSkipped {
        element_id: String,
        element_type: String,
    },
    ImageSkipped {
        element_id: String,
    },
    TextSkipped {
        element_id: String,
    },
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("SVG definition reference is missing: {0}")]
    MissingDefinition(String),

    #[error("SVG output is not parseable: {0}")]
    InvalidSvg(String),

    #[error("unsupported element is configured as an error: {element_type} {element_id}")]
    UnsupportedElement {
        element_id: String,
        element_type: String,
    },

    #[error("image rendering is configured as an error: {element_id}")]
    ImageBlocked { element_id: String },

    #[error("PNG rendering is not implemented yet")]
    PngNotImplemented,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgNode {
    tag: String,
    attrs: Vec<(String, String)>,
    children: Vec<SvgNode>,
    text: Option<String>,
}

impl SvgNode {
    #[must_use]
    pub fn new(tag: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            attrs: Vec::new(),
            children: Vec::new(),
            text: None,
        }
    }

    #[must_use]
    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.push((name.into(), value.into()));
        self
    }

    #[must_use]
    pub fn child(mut self, child: SvgNode) -> Self {
        self.children.push(child);
        self
    }

    #[must_use]
    pub fn text(mut self, value: impl Into<String>) -> Self {
        self.text = Some(value.into());
        self
    }

    fn write_to(&self, output: &mut String) {
        output.push('<');
        output.push_str(&self.tag);
        for (name, value) in &self.attrs {
            output.push(' ');
            output.push_str(name);
            output.push_str("=\"");
            output.push_str(&escape_attr(value));
            output.push('"');
        }

        if self.children.is_empty() && self.text.is_none() {
            output.push_str("/>");
            return;
        }

        output.push('>');
        if let Some(text) = &self.text {
            output.push_str(&escape_text(text));
        }
        for child in &self.children {
            child.write_to(output);
        }
        output.push_str("</");
        output.push_str(&self.tag);
        output.push('>');
    }

    fn collect_def_references(&self, refs: &mut Vec<String>) {
        for (_, value) in &self.attrs {
            collect_url_reference(value, refs);
        }
        for child in &self.children {
            child.collect_def_references(refs);
        }
    }

    fn id(&self) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(name, _)| name == "id")
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SvgDocument {
    view_box: Rect,
    width: u32,
    height: u32,
    defs: Vec<SvgNode>,
    nodes: Vec<SvgNode>,
}

impl SvgDocument {
    #[must_use]
    pub fn new(view_box: Rect) -> Self {
        Self::new_scaled(view_box, 1.0)
    }

    #[must_use]
    pub fn new_scaled(view_box: Rect, scale: f64) -> Self {
        let safe_scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let width = ceil_to_u32(view_box.width * safe_scale).max(1);
        let height = ceil_to_u32(view_box.height * safe_scale).max(1);
        Self {
            view_box,
            width,
            height,
            defs: Vec::new(),
            nodes: Vec::new(),
        }
    }

    #[must_use]
    pub fn def(mut self, node: SvgNode) -> Self {
        self.defs.push(node);
        self
    }

    #[must_use]
    pub fn node(mut self, node: SvgNode) -> Self {
        self.nodes.push(node);
        self
    }

    pub fn validate_references(&self) -> Result<(), RenderError> {
        let def_ids: HashSet<&str> = self.defs.iter().filter_map(SvgNode::id).collect();
        let mut references = Vec::new();
        for node in &self.nodes {
            node.collect_def_references(&mut references);
        }
        for reference in references {
            if !def_ids.contains(reference.as_str()) {
                return Err(RenderError::MissingDefinition(reference));
            }
        }
        Ok(())
    }

    pub fn to_string(&self) -> Result<String, RenderError> {
        self.validate_references()?;

        let mut output = String::new();
        output.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        output.push_str(r#"<svg xmlns="http://www.w3.org/2000/svg" version="1.1""#);
        write_attr(&mut output, "width", &self.width.to_string());
        write_attr(&mut output, "height", &self.height.to_string());
        write_attr(&mut output, "viewBox", &format_rect_view_box(self.view_box));
        output.push('>');

        if !self.defs.is_empty() {
            output.push_str("<defs>");
            for def in &self.defs {
                def.write_to(&mut output);
            }
            output.push_str("</defs>");
        }

        for node in &self.nodes {
            node.write_to(&mut output);
        }
        output.push_str("</svg>");
        Ok(output)
    }
}

#[derive(Debug, Clone, Default)]
pub struct DefIdAllocator {
    used: HashSet<String>,
}

impl DefIdAllocator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allocate(&mut self, prefix: &str, source: &str) -> String {
        let base = sanitize_id(&format!("{prefix}-{source}"));
        let mut candidate = base.clone();
        let mut suffix = 2_u32;
        while self.used.contains(&candidate) {
            candidate = format!("{base}-{suffix}");
            suffix += 1;
        }
        self.used.insert(candidate.clone());
        candidate
    }
}

pub fn render_svg(
    scene: &Scene,
    options: &RenderOptions,
) -> Result<RenderOutput<String>, RenderError> {
    let warnings = collect_policy_warnings(scene, options)?;
    let view_box = scene.content_bounds.padded(options.padding.max(0.0));
    let mut document = SvgDocument::new_scaled(view_box, options.scale);

    if let Some(background) = background_color(scene, options) {
        document = document.node(
            SvgNode::new("rect")
                .attr("x", view_box.x.to_string())
                .attr("y", view_box.y.to_string())
                .attr("width", view_box.width.to_string())
                .attr("height", view_box.height.to_string())
                .attr("fill", color_to_svg(background)),
        );
    }

    document = document.node(SvgNode::new("g").attr("id", "excalidraw-content"));
    let svg = document.to_string()?;
    usvg::Tree::from_str(&svg, &usvg::Options::default())
        .map_err(|error| RenderError::InvalidSvg(error.to_string()))?;
    Ok(RenderOutput {
        value: svg,
        warnings,
    })
}

pub fn render_png(
    _scene: &Scene,
    _options: &RenderOptions,
) -> Result<RenderOutput<Vec<u8>>, RenderError> {
    Err(RenderError::PngNotImplemented)
}

fn collect_policy_warnings(
    scene: &Scene,
    options: &RenderOptions,
) -> Result<Vec<RenderWarning>, RenderError> {
    let mut warnings = Vec::new();
    for normalized in &scene.elements {
        match &normalized.element {
            Element::Embeddable(element) => handle_unsupported(
                "embeddable",
                &element.base.id,
                options.unsupported,
                &mut warnings,
            )?,
            Element::Iframe(element) => handle_unsupported(
                "iframe",
                &element.base.id,
                options.unsupported,
                &mut warnings,
            )?,
            Element::Image(image) => match options.image_policy {
                ImagePolicy::Embed | ImagePolicy::Placeholder => {}
                ImagePolicy::Skip => warnings.push(RenderWarning::ImageSkipped {
                    element_id: image.base.id.clone(),
                }),
                ImagePolicy::Error => {
                    return Err(RenderError::ImageBlocked {
                        element_id: image.base.id.clone(),
                    });
                }
            },
            Element::Text(text) if options.text_policy == TextPolicy::Skip => {
                warnings.push(RenderWarning::TextSkipped {
                    element_id: text.base.id.clone(),
                });
            }
            _ => {}
        }
    }
    Ok(warnings)
}

fn handle_unsupported(
    element_type: &str,
    element_id: &str,
    mode: UnsupportedElementMode,
    warnings: &mut Vec<RenderWarning>,
) -> Result<(), RenderError> {
    match mode {
        UnsupportedElementMode::Placeholder => {
            warnings.push(RenderWarning::UnsupportedElementPlaceholder {
                element_id: element_id.to_owned(),
                element_type: element_type.to_owned(),
            });
            Ok(())
        }
        UnsupportedElementMode::Skip => {
            warnings.push(RenderWarning::UnsupportedElementSkipped {
                element_id: element_id.to_owned(),
                element_type: element_type.to_owned(),
            });
            Ok(())
        }
        UnsupportedElementMode::Error => Err(RenderError::UnsupportedElement {
            element_id: element_id.to_owned(),
            element_type: element_type.to_owned(),
        }),
    }
}

fn background_color(scene: &Scene, options: &RenderOptions) -> Option<Color> {
    match options.background {
        BackgroundMode::FromFile => {
            (scene.background_color.a > 0.0).then_some(scene.background_color)
        }
        BackgroundMode::Transparent => None,
        BackgroundMode::Override(color) => (color.a > 0.0).then_some(color),
    }
}

#[must_use]
pub fn sanitize_id(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ':') {
            output.push(character);
        } else {
            output.push('-');
        }
    }
    if !output
        .as_bytes()
        .first()
        .is_some_and(|byte| is_valid_id_start(*byte))
    {
        output.insert_str(0, "id-");
    }
    output
}

#[must_use]
pub fn color_to_svg(color: Color) -> String {
    if color.a <= 0.0 {
        "none".to_owned()
    } else {
        format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
    }
}

fn is_valid_id_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || matches!(byte, b'_' | b':')
}

fn collect_url_reference(value: &str, refs: &mut Vec<String>) {
    let mut remainder = value;
    while let Some(start) = remainder.find("url(#") {
        let Some(after_start) = remainder.get(start + 5..) else {
            break;
        };
        let Some(end) = after_start.find(')') else {
            break;
        };
        if let Some(reference) = after_start.get(..end) {
            refs.push(reference.to_owned());
        }
        let Some(next) = after_start.get(end + 1..) else {
            break;
        };
        remainder = next;
    }
}

fn write_attr(output: &mut String, name: &str, value: &str) {
    output.push(' ');
    output.push_str(name);
    output.push_str("=\"");
    output.push_str(&escape_attr(value));
    output.push('"');
}

fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn format_rect_view_box(rect: Rect) -> String {
    format!("{} {} {} {}", rect.x, rect.y, rect.width, rect.height)
}

fn ceil_to_u32(value: f64) -> u32 {
    if !value.is_finite() || value <= 0.0 {
        return 1;
    }
    value.ceil().min(f64::from(u32::MAX)) as u32
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use excalidraw_core::{normalize_file, parse_str};

    use super::{
        color_to_svg, render_svg, sanitize_id, BackgroundMode, DefIdAllocator, ImagePolicy,
        RenderError, RenderOptions, RenderOutput, RenderWarning, SvgDocument, SvgNode, TextPolicy,
        UnsupportedElementMode,
    };

    #[test]
    fn render_svg_outputs_usvg_parseable_document() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "appState":{"viewBackgroundColor":"#ffeeaa"},
                "elements":[{"type":"rectangle","id":"r","x":5,"y":6,"width":10,"height":10}]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let output = render_svg(&scene, &RenderOptions::default())?;
        let svg = output.value;

        ensure(svg.contains("<svg"), "svg root")?;
        ensure(svg.contains("fill=\"#ffeeaa\""), "background fill")?;
        usvg::Tree::from_str(&svg, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn escapes_text_attributes_and_sanitizes_ids() -> Result<(), Box<dyn Error>> {
        let document = SvgDocument::new(excalidraw_core::Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        })
        .node(
            SvgNode::new("text")
                .attr("id", sanitize_id("12 bad <id>"))
                .attr("data-label", "\"quoted\" & <tag>")
                .text("A&B < C"),
        );
        let svg = document.to_string()?;

        ensure(svg.contains("id=\"id-12-bad--id-\""), "sanitized id")?;
        ensure(
            svg.contains("data-label=\"&quot;quoted&quot; &amp; &lt;tag&gt;\""),
            "escaped attribute",
        )?;
        ensure(svg.contains(">A&amp;B &lt; C</text>"), "escaped text")?;
        usvg::Tree::from_str(&svg, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn validates_definition_references_and_allocates_unique_ids() -> Result<(), Box<dyn Error>> {
        let mut ids = DefIdAllocator::new();
        let first = ids.allocate("clip", "frame 1");
        let second = ids.allocate("clip", "frame 1");
        ensure_eq(&first.as_str(), "clip-frame-1", "first id")?;
        ensure_eq(&second.as_str(), "clip-frame-1-2", "second id")?;

        let valid = SvgDocument::new(excalidraw_core::Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        })
        .def(SvgNode::new("clipPath").attr("id", &first))
        .node(SvgNode::new("g").attr("clip-path", format!("url(#{first})")));
        valid.validate_references()?;

        let invalid = SvgDocument::new(excalidraw_core::Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        })
        .node(SvgNode::new("g").attr("clip-path", "url(#missing)"));
        ensure_eq(
            &invalid
                .validate_references()
                .map_err(|error| error.to_string()),
            Err(RenderError::MissingDefinition("missing".to_owned()).to_string()),
            "missing reference",
        )
    }

    #[test]
    fn supports_background_modes_and_paint_serialization() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "appState":{"viewBackgroundColor":"#112233"},
                "elements":[{"type":"rectangle","id":"r","width":10,"height":10}]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let transparent = render_svg(
            &scene,
            &RenderOptions {
                background: BackgroundMode::Transparent,
                ..RenderOptions::default()
            },
        )?;
        ensure(
            !transparent.value.contains("<rect"),
            "transparent omits background",
        )?;

        let override_svg = render_svg(
            &scene,
            &RenderOptions {
                background: BackgroundMode::Override(excalidraw_core::Color::rgb(1, 2, 3)),
                ..RenderOptions::default()
            },
        )?;
        ensure(
            override_svg.value.contains("fill=\"#010203\""),
            "override fill",
        )?;
        ensure_eq(
            &color_to_svg(excalidraw_core::Color::transparent()).as_str(),
            "none",
            "transparent paint",
        )
    }

    #[test]
    fn render_options_control_policy_warnings() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[
                    {"type":"iframe","id":"embed","width":10,"height":10},
                    {"type":"image","id":"image","width":10,"height":10},
                    {"type":"text","id":"text","text":"hello"}
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let output = render_svg(
            &scene,
            &RenderOptions {
                scale: 2.0,
                padding: 4.0,
                unsupported: UnsupportedElementMode::Skip,
                image_policy: ImagePolicy::Skip,
                text_policy: TextPolicy::Skip,
                ..RenderOptions::default()
            },
        )?;

        ensure(output.value.contains("width=\""), "width attr")?;
        ensure_eq(
            &output.warnings,
            vec![
                RenderWarning::UnsupportedElementSkipped {
                    element_id: "embed".to_owned(),
                    element_type: "iframe".to_owned(),
                },
                RenderWarning::ImageSkipped {
                    element_id: "image".to_owned(),
                },
                RenderWarning::TextSkipped {
                    element_id: "text".to_owned(),
                },
            ],
            "policy warnings",
        )
    }

    #[test]
    fn render_policy_errors_are_structured() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[{"type":"embeddable","id":"embed","width":10,"height":10}]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let error = render_svg(
            &scene,
            &RenderOptions {
                unsupported: UnsupportedElementMode::Error,
                ..RenderOptions::default()
            },
        )
        .map(|_: RenderOutput<String>| ())
        .map_err(|error| error.to_string());

        ensure_eq(
            &error,
            Err("unsupported element is configured as an error: embeddable embed".to_owned()),
            "unsupported error",
        )?;

        let png_error = super::render_png(&scene, &RenderOptions::default())
            .map(|_: RenderOutput<Vec<u8>>| ())
            .map_err(|error| error.to_string());
        ensure_eq(
            &png_error,
            Err("PNG rendering is not implemented yet".to_owned()),
            "png error",
        )
    }

    fn ensure(value: bool, label: &str) -> Result<(), Box<dyn Error>> {
        if value {
            Ok(())
        } else {
            Err(label.to_owned().into())
        }
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
