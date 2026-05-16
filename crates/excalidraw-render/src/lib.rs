//! Rendering backend for normalized Excalidraw scenes.
//!
//! This crate will own SVG/PNG generation, rough-rs integration, text layout,
//! image handling, frames, and render warnings.

use std::collections::HashSet;

use excalidraw_core::{
    font_family_css, font_family_primary, font_family_width_factor, Arrowhead, BaseElement, Color,
    Element, FillStyle as ExcalidrawFillStyle, FrameElement, FreedrawElement, ImageElement,
    LinearElement, NormalizedElement, Point, Rect, Scene, ShapeElement, StrokeStyle, TextAlign,
    TextElement, UnsupportedElement, VerticalAlign,
};
use fontdb::{Database, Family, Query};
use rough_rs::svg::drawable_to_paths;
use rough_rs::{Config, Generator, Options as RoughOptions};
use thiserror::Error;
use unicode_width::UnicodeWidthStr;

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
    MissingImageData {
        element_id: String,
    },
    ImagePlaceholder {
        element_id: String,
    },
    UnknownElementPlaceholder {
        element_id: String,
        element_type: String,
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

#[derive(Debug)]
pub struct FontRegistry {
    database: Database,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeasuredText {
    pub width: f64,
    pub height: f64,
    pub line_height: f64,
    pub lines: Vec<String>,
}

impl Default for FontRegistry {
    fn default() -> Self {
        let mut database = Database::new();
        database.load_system_fonts();
        Self { database }
    }
}

impl FontRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn resolve_family(&self, family: u32) -> String {
        let primary = font_family_primary(family);
        let query = Query {
            families: &[Family::Name(primary)],
            ..Query::default()
        };
        if self.database.query(&query).is_some() {
            format!("registered:{primary}")
        } else {
            format!("fallback:{primary}")
        }
    }

    #[must_use]
    pub fn measure_text(&self, text: &TextElement) -> MeasuredText {
        let lines = text_lines(&text.text);
        let width_factor = font_family_width_factor(text.font_family);
        let width = lines
            .iter()
            .map(|line| UnicodeWidthStr::width(line.as_str()) as f64)
            .fold(0.0, f64::max)
            * text.font_size
            * width_factor;
        let line_height = text.font_size * text.line_height;
        let height = lines.len() as f64 * line_height;
        MeasuredText {
            width,
            height,
            line_height,
            lines,
        }
    }
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
    let mut warnings = collect_policy_warnings(scene, options)?;
    let fonts = FontRegistry::new();
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

    let mut content = SvgNode::new("g").attr("id", "excalidraw-content");
    for normalized in &scene.elements {
        let rendered = render_element(normalized, scene, options, &fonts, &mut warnings);
        for def in rendered.defs {
            document = document.def(def);
        }
        for node in rendered.nodes {
            content = content.child(node);
        }
    }
    document = document.node(content);
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

struct RenderedElement {
    defs: Vec<SvgNode>,
    nodes: Vec<SvgNode>,
}

fn nodes_only(nodes: Vec<SvgNode>) -> RenderedElement {
    RenderedElement {
        defs: Vec::new(),
        nodes,
    }
}

fn render_element(
    normalized: &NormalizedElement,
    scene: &Scene,
    options: &RenderOptions,
    fonts: &FontRegistry,
    warnings: &mut Vec<RenderWarning>,
) -> RenderedElement {
    match &normalized.element {
        Element::Rectangle(shape) => nodes_only(render_shape(shape, ShapeKind::Rectangle, options)),
        Element::Ellipse(shape) => nodes_only(render_shape(shape, ShapeKind::Ellipse, options)),
        Element::Diamond(shape) => nodes_only(render_shape(shape, ShapeKind::Diamond, options)),
        Element::Line(linear) => nodes_only(render_linear(
            linear,
            normalized.abs_points.as_deref(),
            false,
        )),
        Element::Arrow(linear) => nodes_only(render_linear(
            linear,
            normalized.abs_points.as_deref(),
            true,
        )),
        Element::Text(text) if options.text_policy == TextPolicy::SvgText => {
            nodes_only(render_text(text, scene, fonts))
        }
        Element::Freedraw(freedraw) => {
            nodes_only(render_freedraw(freedraw, normalized.abs_points.as_deref()))
        }
        Element::Image(image) => render_image(image, scene, options, warnings),
        Element::Frame(frame) | Element::MagicFrame(frame) => nodes_only(render_frame(frame)),
        Element::Embeddable(unsupported) | Element::Iframe(unsupported) => {
            render_unsupported(unsupported, options, warnings)
        }
        Element::Unknown { element_type, raw } => {
            render_unknown(element_type, raw, options, warnings)
        }
        _ => RenderedElement {
            defs: Vec::new(),
            nodes: Vec::new(),
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShapeKind {
    Rectangle,
    Ellipse,
    Diamond,
}

fn render_shape(shape: &ShapeElement, kind: ShapeKind, options: &RenderOptions) -> Vec<SvgNode> {
    match options.quality {
        RenderQuality::Clean => vec![clean_shape_node(shape, kind)],
        RenderQuality::Full | RenderQuality::FastSvg => rough_shape_nodes(shape, kind),
    }
}

fn render_linear(
    linear: &LinearElement,
    abs_points: Option<&[Point]>,
    include_heads: bool,
) -> Vec<SvgNode> {
    let Some(points) = abs_points.filter(|points| points.len() >= 2) else {
        return Vec::new();
    };
    let style = RenderStyle::from_base(&linear.base);
    let mut group = SvgNode::new("g");
    if let Some(transform) = &style.transform {
        group = group.attr("transform", transform);
    }
    if let Some(opacity) = &style.opacity {
        group = group.attr("opacity", opacity);
    }

    let mut path = SvgNode::new("path")
        .attr("d", linear_path_data(points))
        .attr("stroke", style.stroke.clone())
        .attr("stroke-width", style.stroke_width.to_string())
        .attr("fill", "none");
    if let Some(dasharray) = &style.stroke_dasharray {
        path = path.attr("stroke-dasharray", dasharray);
    }
    group = group.child(path);

    if include_heads {
        if let Some((tip, rest)) = points.split_first() {
            if let (Some(head), Some(neighbor)) = (&linear.start_arrowhead, rest.first()) {
                if let Some(node) = arrowhead_node(head, *tip, *neighbor, &style) {
                    group = group.child(node);
                }
            }
        }
        if let Some((tip, rest)) = points.split_last() {
            if let (Some(head), Some(neighbor)) = (&linear.end_arrowhead, rest.last()) {
                if let Some(node) = arrowhead_node(head, *tip, *neighbor, &style) {
                    group = group.child(node);
                }
            }
        }
    }

    vec![group]
}

fn render_text(text: &TextElement, scene: &Scene, fonts: &FontRegistry) -> Vec<SvgNode> {
    if text.text.is_empty() {
        return Vec::new();
    }
    let measurement = fonts.measure_text(text);
    let layout = text_layout(text, scene, &measurement);
    let style = RenderStyle::from_base(&text.base);

    let mut node = SvgNode::new("text")
        .attr("x", layout.x.to_string())
        .attr("y", layout.first_baseline.to_string())
        .attr("fill", text.base.stroke_color.clone())
        .attr("font-family", font_family_css(text.font_family))
        .attr("font-size", text.font_size.to_string())
        .attr("text-anchor", layout.anchor)
        .attr("data-font-source", fonts.resolve_family(text.font_family));
    if let Some(opacity) = &style.opacity {
        node = node.attr("opacity", opacity);
    }
    if let Some(transform) = &style.transform {
        node = node.attr("transform", transform);
    }

    for (index, line) in measurement.lines.iter().enumerate() {
        let mut tspan = SvgNode::new("tspan").attr("x", layout.x.to_string());
        if index == 0 {
            tspan = tspan.attr("y", layout.first_baseline.to_string());
        } else {
            tspan = tspan.attr("dy", measurement.line_height.to_string());
        }
        node = node.child(tspan.text(line));
    }

    vec![node]
}

#[derive(Debug, Clone, PartialEq)]
struct TextLayout {
    x: f64,
    first_baseline: f64,
    anchor: &'static str,
}

fn text_layout(text: &TextElement, scene: &Scene, measurement: &MeasuredText) -> TextLayout {
    let rect = text_layout_rect(text, scene, measurement);
    let x = match text.text_align {
        TextAlign::Center => rect.x + rect.width / 2.0,
        TextAlign::Right => rect.x + rect.width,
        TextAlign::Left | TextAlign::Unknown => rect.x,
    };
    let anchor = match text.text_align {
        TextAlign::Center => "middle",
        TextAlign::Right => "end",
        TextAlign::Left | TextAlign::Unknown => "start",
    };
    let top = match text.vertical_align {
        VerticalAlign::Middle => rect.y + (rect.height - measurement.height).max(0.0) / 2.0,
        VerticalAlign::Bottom => rect.y + (rect.height - measurement.height).max(0.0),
        VerticalAlign::Top | VerticalAlign::Unknown => rect.y,
    };
    TextLayout {
        x,
        first_baseline: top + text.font_size * 0.8,
        anchor,
    }
}

fn text_layout_rect(text: &TextElement, scene: &Scene, measurement: &MeasuredText) -> Rect {
    if let Some(container_id) = &text.container_id {
        if let Some(container) = scene
            .id_map
            .get(container_id)
            .and_then(|index| scene.elements.get(*index))
        {
            if let Some(rect) = container_text_rect(&container.element) {
                return rect;
            }
        }
    }

    Rect {
        x: text.base.x,
        y: text.base.y,
        width: text.base.width.max(measurement.width),
        height: text.base.height.max(measurement.height),
    }
    .normalized()
}

fn container_text_rect(element: &Element) -> Option<Rect> {
    const PADDING: f64 = 8.0;
    let base = match element {
        Element::Rectangle(shape) | Element::Ellipse(shape) | Element::Diamond(shape) => {
            Some(&shape.base)
        }
        _ => None,
    }?;
    let rect = Rect {
        x: base.x,
        y: base.y,
        width: base.width,
        height: base.height,
    }
    .normalized();
    Some(Rect {
        x: rect.x + PADDING,
        y: rect.y + PADDING,
        width: (rect.width - PADDING * 2.0).max(0.0),
        height: (rect.height - PADDING * 2.0).max(0.0),
    })
}

fn text_lines(text: &str) -> Vec<String> {
    let lines: Vec<String> = text.split('\n').map(ToOwned::to_owned).collect();
    if lines.is_empty() {
        vec![String::new()]
    } else {
        lines
    }
}

// --- Freedraw rendering ---

fn render_freedraw(freedraw: &FreedrawElement, abs_points: Option<&[Point]>) -> Vec<SvgNode> {
    let points = match abs_points.filter(|p| p.len() >= 2) {
        Some(p) => p,
        None => return Vec::new(),
    };

    let style = RenderStyle::from_base(&freedraw.base);
    let stroke_width = freedraw.base.stroke_width.max(1.0);

    // Build a simplified stroke outline from points.
    // For v0.1 we use a polyline with round line-join and line-cap.
    let d = freedraw_path_data(points);

    let mut path = SvgNode::new("path")
        .attr("d", d)
        .attr("stroke", style.stroke)
        .attr("stroke-width", stroke_width.to_string())
        .attr("fill", "none")
        .attr("stroke-linecap", "round")
        .attr("stroke-linejoin", "round");

    if let Some(opacity) = &style.opacity {
        path = path.attr("opacity", opacity);
    }
    if let Some(transform) = &style.transform {
        path = path.attr("transform", transform);
    }

    vec![path]
}

fn freedraw_path_data(points: &[Point]) -> String {
    let mut d = String::from("M ");
    if let Some(first) = points.first() {
        d.push_str(&format!("{} {}", first.x, first.y));
    }
    for point in &points[1..] {
        d.push_str(&format!(" L {} {}", point.x, point.y));
    }
    d
}

// --- Image rendering ---

fn render_image(
    image: &ImageElement,
    scene: &Scene,
    options: &RenderOptions,
    warnings: &mut Vec<RenderWarning>,
) -> RenderedElement {
    let data_url = resolve_image_data_url(image, scene);

    match (data_url, options.image_policy) {
        (Some(url), ImagePolicy::Embed | ImagePolicy::Placeholder) => {
            render_image_embed(image, &url)
        }
        (None, ImagePolicy::Embed | ImagePolicy::Placeholder) => {
            warnings.push(RenderWarning::MissingImageData {
                element_id: image.base.id.clone(),
            });
            nodes_only(render_image_placeholder(image))
        }
        (_, ImagePolicy::Skip) => RenderedElement {
            defs: Vec::new(),
            nodes: Vec::new(),
        },
        (_, ImagePolicy::Error) => {
            // This is already caught by collect_policy_warnings, but handle defensively
            RenderedElement {
                defs: Vec::new(),
                nodes: Vec::new(),
            }
        }
    }
}

fn resolve_image_data_url(image: &ImageElement, scene: &Scene) -> Option<String> {
    if let Some(file_id) = &image.file_id {
        if let Some(file_data) = scene.files.get(file_id) {
            if !file_data.data_url.is_empty() {
                return Some(file_data.data_url.clone());
            }
        }
    }
    None
}

fn render_image_embed(image: &ImageElement, data_url: &str) -> RenderedElement {
    let base = &image.base;
    let style = RenderStyle::from_base(base);

    let mut img = SvgNode::new("image")
        .attr("x", base.x.to_string())
        .attr("y", base.y.to_string())
        .attr("width", base.width.to_string())
        .attr("height", base.height.to_string())
        .attr("href", data_url.to_owned());

    if let Some(opacity) = &style.opacity {
        img = img.attr("opacity", opacity);
    }

    // Handle scale flips
    if let Some([sx, sy]) = image.scale {
        if sx < 0.0 || sy < 0.0 {
            let tx = if sx < 0.0 {
                base.x + base.width
            } else {
                base.x
            };
            let ty = if sy < 0.0 {
                base.y + base.height
            } else {
                base.y
            };
            img = img.attr(
                "transform",
                format!(
                    "translate({tx} {ty}) scale({sx} {sy})",
                    sx = sx.abs(),
                    sy = sy.abs(),
                ),
            );
        }
    }

    // Handle crop via clipPath
    let mut clip_defs = Vec::new();
    if let Some(crop) = &image.crop {
        if crop.width > 0.0 && crop.height > 0.0 {
            let clip_id = format!("crop-{}", sanitize_id(&base.id));
            let clip_rect = SvgNode::new("rect")
                .attr("x", crop.x.to_string())
                .attr("y", crop.y.to_string())
                .attr("width", crop.width.to_string())
                .attr("height", crop.height.to_string());
            let clip_path = SvgNode::new("clipPath")
                .attr("id", clip_id.clone())
                .child(clip_rect);
            clip_defs.push(clip_path);
            img = img.attr("clip-path", format!("url(#{clip_id})"));
        }
    }
    let mut nodes = Vec::new();

    if let Some(transform) = &style.transform {
        img = img.attr("transform", transform);
    }

    nodes.push(img);
    RenderedElement {
        defs: clip_defs,
        nodes,
    }
}

fn render_image_placeholder(image: &ImageElement) -> Vec<SvgNode> {
    let base = &image.base;
    let rect = SvgNode::new("rect")
        .attr("x", base.x.to_string())
        .attr("y", base.y.to_string())
        .attr("width", base.width.to_string())
        .attr("height", base.height.to_string())
        .attr("fill", "#f0f0f0")
        .attr("stroke", "#cccccc")
        .attr("stroke-width", "1")
        .attr("stroke-dasharray", "4 4");

    let label_x = base.x + base.width / 2.0;
    let label_y = base.y + base.height / 2.0;
    let label = SvgNode::new("text")
        .attr("x", label_x.to_string())
        .attr("y", label_y.to_string())
        .attr("text-anchor", "middle")
        .attr("dominant-baseline", "central")
        .attr("fill", "#999999")
        .attr("font-size", "12")
        .text("no image");

    vec![rect, label]
}

// --- Frame rendering ---

fn render_frame(frame: &FrameElement) -> Vec<SvgNode> {
    let base = &frame.base;
    let mut group = SvgNode::new("g").attr("data-frame", base.id.clone());

    // Frame border
    let border = SvgNode::new("rect")
        .attr("x", base.x.to_string())
        .attr("y", base.y.to_string())
        .attr("width", base.width.to_string())
        .attr("height", base.height.to_string())
        .attr("fill", "none")
        .attr("stroke", "#adb5bd")
        .attr("stroke-width", "1")
        .attr("stroke-dasharray", "6 4")
        .attr("rx", "2")
        .attr("ry", "2");
    group = group.child(border);

    // Frame label
    if let Some(name) = frame.name.as_deref().filter(|n| !n.is_empty()) {
        let label = SvgNode::new("text")
            .attr("x", base.x.to_string())
            .attr("y", (base.y - 6.0).to_string())
            .attr("fill", "#868e96")
            .attr("font-size", "14")
            .attr("dominant-baseline", "auto")
            .text(name.to_owned());
        group = group.child(label);
    }

    // Collapsed indicator
    if frame.is_collapsed.unwrap_or(false) {
        let indicator = SvgNode::new("text")
            .attr("x", (base.x + base.width / 2.0).to_string())
            .attr("y", (base.y + base.height / 2.0).to_string())
            .attr("text-anchor", "middle")
            .attr("dominant-baseline", "central")
            .attr("fill", "#adb5bd")
            .attr("font-size", "12")
            .text("collapsed");
        group = group.child(indicator);
    }

    vec![group]
}

// --- Unsupported / Unknown element rendering ---

fn render_unsupported(
    element: &UnsupportedElement,
    options: &RenderOptions,
    warnings: &mut Vec<RenderWarning>,
) -> RenderedElement {
    if options.unsupported == UnsupportedElementMode::Skip {
        return RenderedElement {
            defs: Vec::new(),
            nodes: Vec::new(),
        };
    }

    let base = &element.base;
    let width = base.width.max(40.0);
    let height = base.height.max(30.0);

    warnings.push(RenderWarning::UnsupportedElementPlaceholder {
        element_id: base.id.clone(),
        element_type: "unsupported".to_owned(),
    });

    nodes_only(placeholder_group(
        &base.id,
        base.x,
        base.y,
        width,
        height,
        "unsupported",
    ))
}

fn render_unknown(
    element_type: &str,
    raw: &serde_json::Value,
    options: &RenderOptions,
    warnings: &mut Vec<RenderWarning>,
) -> RenderedElement {
    if options.unsupported == UnsupportedElementMode::Skip {
        return RenderedElement {
            defs: Vec::new(),
            nodes: Vec::new(),
        };
    }

    // Try to extract position info from raw JSON
    let x = raw.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let y = raw.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let width = raw
        .get("width")
        .and_then(|v| v.as_f64())
        .unwrap_or(40.0)
        .max(40.0);
    let height = raw
        .get("height")
        .and_then(|v| v.as_f64())
        .unwrap_or(30.0)
        .max(30.0);
    let id = raw.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");

    warnings.push(RenderWarning::UnknownElementPlaceholder {
        element_id: id.to_owned(),
        element_type: element_type.to_owned(),
    });

    nodes_only(placeholder_group(id, x, y, width, height, element_type))
}

fn placeholder_group(id: &str, x: f64, y: f64, w: f64, h: f64, label: &str) -> Vec<SvgNode> {
    let rect = SvgNode::new("rect")
        .attr("x", x.to_string())
        .attr("y", y.to_string())
        .attr("width", w.to_string())
        .attr("height", h.to_string())
        .attr("fill", "#fff3bf")
        .attr("stroke", "#fcc419")
        .attr("stroke-width", "1")
        .attr("stroke-dasharray", "4 4")
        .attr("rx", "2")
        .attr("ry", "2");

    let label_x = x + w / 2.0;
    let label_y = y + h / 2.0;
    let text = SvgNode::new("text")
        .attr("x", label_x.to_string())
        .attr("y", label_y.to_string())
        .attr("text-anchor", "middle")
        .attr("dominant-baseline", "central")
        .attr("fill", "#e67700")
        .attr("font-size", "11")
        .text(label.to_owned());

    let group = SvgNode::new("g")
        .attr("data-placeholder", id)
        .child(rect)
        .child(text);

    vec![group]
}

fn clean_shape_node(shape: &ShapeElement, kind: ShapeKind) -> SvgNode {
    let base = &shape.base;
    let style = RenderStyle::from_base(base);
    let mut node = match kind {
        ShapeKind::Rectangle => {
            let mut rect = SvgNode::new("rect")
                .attr("x", base.x.to_string())
                .attr("y", base.y.to_string())
                .attr("width", base.width.to_string())
                .attr("height", base.height.to_string());
            if let Some(radius) = style.corner_radius {
                rect = rect
                    .attr("rx", radius.to_string())
                    .attr("ry", radius.to_string());
            }
            rect
        }
        ShapeKind::Ellipse => SvgNode::new("ellipse")
            .attr("cx", (base.x + base.width / 2.0).to_string())
            .attr("cy", (base.y + base.height / 2.0).to_string())
            .attr("rx", (base.width / 2.0).to_string())
            .attr("ry", (base.height / 2.0).to_string()),
        ShapeKind::Diamond => SvgNode::new("polygon").attr("points", diamond_points(base)),
    };
    style.apply_to_node(&mut node);
    node
}

fn rough_shape_nodes(shape: &ShapeElement, kind: ShapeKind) -> Vec<SvgNode> {
    let base = &shape.base;
    let style = RenderStyle::from_base(base);
    let generator = Generator::new(Config::default());
    let rough_options = style.rough_options(base);
    let drawable = match kind {
        ShapeKind::Rectangle => {
            generator.rectangle(base.x, base.y, base.width, base.height, Some(rough_options))
        }
        ShapeKind::Ellipse => generator.ellipse(
            base.x + base.width / 2.0,
            base.y + base.height / 2.0,
            base.width,
            base.height,
            Some(rough_options),
        ),
        ShapeKind::Diamond => generator.polygon(&diamond_point_array(base), Some(rough_options)),
    };

    let mut group = SvgNode::new("g");
    if let Some(transform) = &style.transform {
        group = group.attr("transform", transform);
    }
    if let Some(opacity) = &style.opacity {
        group = group.attr("opacity", opacity);
    }
    for path in drawable_to_paths(&drawable) {
        let mut node = SvgNode::new("path")
            .attr("d", path.d)
            .attr("stroke", path.stroke)
            .attr("stroke-width", path.stroke_width.to_string())
            .attr("fill", path.fill);
        if let Some(dasharray) = &style.stroke_dasharray {
            node = node.attr("stroke-dasharray", dasharray);
        }
        group = group.child(node);
    }
    vec![group]
}

fn linear_path_data(points: &[Point]) -> String {
    let mut data = String::new();
    for (index, point) in points.iter().enumerate() {
        if index == 0 {
            data.push('M');
        } else {
            data.push_str(" L");
        }
        data.push_str(&point.x.to_string());
        data.push(' ');
        data.push_str(&point.y.to_string());
    }
    data
}

fn arrowhead_node(
    head: &Arrowhead,
    tip: Point,
    neighbor: Point,
    style: &RenderStyle,
) -> Option<SvgNode> {
    if matches!(head, Arrowhead::Unknown) {
        return None;
    }
    let direction = arrow_direction(tip, neighbor)?;
    let normal = Point {
        x: -direction.y,
        y: direction.x,
    };
    let size = (style.stroke_width * 6.0).max(12.0);
    let base = Point {
        x: tip.x - direction.x * size,
        y: tip.y - direction.y * size,
    };

    let node = match head {
        Arrowhead::Arrow => SvgNode::new("path")
            .attr(
                "d",
                format!(
                    "M{} {} L{} {} M{} {} L{} {}",
                    base.x + normal.x * size * 0.45,
                    base.y + normal.y * size * 0.45,
                    tip.x,
                    tip.y,
                    tip.x,
                    tip.y,
                    base.x - normal.x * size * 0.45,
                    base.y - normal.y * size * 0.45
                ),
            )
            .attr("stroke", style.stroke.clone())
            .attr("stroke-width", style.stroke_width.to_string())
            .attr("fill", "none"),
        Arrowhead::Triangle | Arrowhead::TriangleOutline => {
            let points = triangle_points(tip, base, normal, size);
            let fill = if matches!(head, Arrowhead::TriangleOutline) {
                "none".to_owned()
            } else {
                style.stroke.clone()
            };
            SvgNode::new("polygon")
                .attr("points", points)
                .attr("stroke", style.stroke.clone())
                .attr("stroke-width", style.stroke_width.to_string())
                .attr("fill", fill)
        }
        Arrowhead::Bar => SvgNode::new("path")
            .attr(
                "d",
                format!(
                    "M{} {} L{} {}",
                    tip.x + normal.x * size * 0.5,
                    tip.y + normal.y * size * 0.5,
                    tip.x - normal.x * size * 0.5,
                    tip.y - normal.y * size * 0.5
                ),
            )
            .attr("stroke", style.stroke.clone())
            .attr("stroke-width", style.stroke_width.to_string())
            .attr("fill", "none"),
        Arrowhead::Dot | Arrowhead::Circle => SvgNode::new("circle")
            .attr("cx", tip.x.to_string())
            .attr("cy", tip.y.to_string())
            .attr("r", (size * 0.35).to_string())
            .attr("stroke", style.stroke.clone())
            .attr("stroke-width", style.stroke_width.to_string())
            .attr(
                "fill",
                if matches!(head, Arrowhead::Circle) {
                    "none".to_owned()
                } else {
                    style.stroke.clone()
                },
            ),
        Arrowhead::Diamond => SvgNode::new("polygon")
            .attr("points", diamond_arrowhead_points(tip, base, normal, size))
            .attr("stroke", style.stroke.clone())
            .attr("stroke-width", style.stroke_width.to_string())
            .attr("fill", style.stroke.clone()),
        Arrowhead::Crowfoot => SvgNode::new("path")
            .attr(
                "d",
                format!(
                    "M{} {} L{} {} M{} {} L{} {} M{} {} L{} {}",
                    tip.x,
                    tip.y,
                    base.x + normal.x * size * 0.55,
                    base.y + normal.y * size * 0.55,
                    tip.x,
                    tip.y,
                    base.x,
                    base.y,
                    tip.x,
                    tip.y,
                    base.x - normal.x * size * 0.55,
                    base.y - normal.y * size * 0.55
                ),
            )
            .attr("stroke", style.stroke.clone())
            .attr("stroke-width", style.stroke_width.to_string())
            .attr("fill", "none"),
        Arrowhead::Unknown => return None,
    };
    Some(node)
}

fn arrow_direction(tip: Point, neighbor: Point) -> Option<Point> {
    let dx = tip.x - neighbor.x;
    let dy = tip.y - neighbor.y;
    let length = (dx * dx + dy * dy).sqrt();
    (length > f64::EPSILON).then_some(Point {
        x: dx / length,
        y: dy / length,
    })
}

fn triangle_points(tip: Point, base: Point, normal: Point, size: f64) -> String {
    format!(
        "{},{} {},{} {},{}",
        tip.x,
        tip.y,
        base.x + normal.x * size * 0.5,
        base.y + normal.y * size * 0.5,
        base.x - normal.x * size * 0.5,
        base.y - normal.y * size * 0.5
    )
}

fn diamond_arrowhead_points(tip: Point, base: Point, normal: Point, size: f64) -> String {
    let center = Point {
        x: (tip.x + base.x) / 2.0,
        y: (tip.y + base.y) / 2.0,
    };
    format!(
        "{},{} {},{} {},{} {},{}",
        tip.x,
        tip.y,
        center.x + normal.x * size * 0.35,
        center.y + normal.y * size * 0.35,
        base.x,
        base.y,
        center.x - normal.x * size * 0.35,
        center.y - normal.y * size * 0.35
    )
}

#[derive(Debug, Clone, PartialEq)]
struct RenderStyle {
    stroke: String,
    fill: String,
    rough_fill: Option<String>,
    rough_fill_style: Option<rough_rs::FillStyle>,
    stroke_width: f64,
    stroke_dasharray: Option<String>,
    stroke_dash: Option<Vec<f64>>,
    opacity: Option<String>,
    transform: Option<String>,
    corner_radius: Option<f64>,
}

impl RenderStyle {
    fn from_base(base: &BaseElement) -> Self {
        let stroke_width = base.stroke_width.max(0.0);
        let stroke_dash = stroke_dash_array(&base.stroke_style, stroke_width);
        Self {
            stroke: base.stroke_color.clone(),
            fill: fill_color(base).unwrap_or_else(|| "none".to_owned()),
            rough_fill: fill_color(base),
            rough_fill_style: rough_fill_style(&base.fill_style),
            stroke_width,
            stroke_dasharray: stroke_dash.as_ref().map(|values| {
                values
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" ")
            }),
            stroke_dash,
            opacity: opacity_attr(base),
            transform: rotation_transform(base),
            corner_radius: rectangle_corner_radius(base),
        }
    }

    fn apply_to_node(&self, node: &mut SvgNode) {
        *node = std::mem::replace(node, SvgNode::new("g"))
            .attr("stroke", self.stroke.clone())
            .attr("stroke-width", self.stroke_width.to_string())
            .attr("fill", self.fill.clone());
        if let Some(dasharray) = &self.stroke_dasharray {
            *node = std::mem::replace(node, SvgNode::new("g")).attr("stroke-dasharray", dasharray);
        }
        if let Some(opacity) = &self.opacity {
            *node = std::mem::replace(node, SvgNode::new("g")).attr("opacity", opacity);
        }
        if let Some(transform) = &self.transform {
            *node = std::mem::replace(node, SvgNode::new("g")).attr("transform", transform);
        }
    }

    fn rough_options(&self, base: &BaseElement) -> RoughOptions {
        RoughOptions {
            seed: Some(base.seed),
            stroke: Some(self.stroke.clone()),
            stroke_width: Some(self.stroke_width),
            roughness: Some(base.roughness),
            fill: self.rough_fill.clone(),
            fill_style: self.rough_fill_style,
            stroke_line_dash: self.stroke_dash.clone(),
            fixed_decimal_place_digits: Some(2),
            ..RoughOptions::default()
        }
    }
}

fn fill_color(base: &BaseElement) -> Option<String> {
    if matches!(base.fill_style, ExcalidrawFillStyle::None) {
        return None;
    }
    let value = base.background_color.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("transparent") {
        None
    } else {
        Some(value.to_owned())
    }
}

fn rough_fill_style(fill_style: &ExcalidrawFillStyle) -> Option<rough_rs::FillStyle> {
    match fill_style {
        ExcalidrawFillStyle::Hachure => Some(rough_rs::FillStyle::Hachure),
        ExcalidrawFillStyle::Solid => Some(rough_rs::FillStyle::Solid),
        ExcalidrawFillStyle::CrossHatch => Some(rough_rs::FillStyle::CrossHatch),
        ExcalidrawFillStyle::Dots => Some(rough_rs::FillStyle::Dots),
        ExcalidrawFillStyle::Dashed => Some(rough_rs::FillStyle::Dashed),
        ExcalidrawFillStyle::ZigzagLine => Some(rough_rs::FillStyle::ZigzagLine),
        ExcalidrawFillStyle::None | ExcalidrawFillStyle::Unknown => None,
    }
}

fn stroke_dash_array(stroke_style: &StrokeStyle, stroke_width: f64) -> Option<Vec<f64>> {
    match stroke_style {
        StrokeStyle::Dashed => Some(vec![stroke_width * 4.0, stroke_width * 4.0]),
        StrokeStyle::Dotted => Some(vec![stroke_width, stroke_width * 2.0]),
        StrokeStyle::Solid | StrokeStyle::Unknown => None,
    }
}

fn rectangle_corner_radius(base: &BaseElement) -> Option<f64> {
    base.roundness.as_ref()?;
    Some((base.width.abs().min(base.height.abs()) * 0.25).max(0.0))
}

fn opacity_attr(base: &BaseElement) -> Option<String> {
    (base.opacity < 100.0).then(|| (base.opacity / 100.0).clamp(0.0, 1.0).to_string())
}

fn rotation_transform(base: &BaseElement) -> Option<String> {
    if base.angle.abs() < f64::EPSILON {
        return None;
    }
    let cx = base.x + base.width / 2.0;
    let cy = base.y + base.height / 2.0;
    Some(format!("rotate({} {} {})", base.angle.to_degrees(), cx, cy))
}

fn diamond_points(base: &BaseElement) -> String {
    diamond_point_array(base)
        .iter()
        .map(|point| format!("{},{}", point[0], point[1]))
        .collect::<Vec<_>>()
        .join(" ")
}

fn diamond_point_array(base: &BaseElement) -> Vec<rough_rs::geometry::Point> {
    let cx = base.x + base.width / 2.0;
    let cy = base.y + base.height / 2.0;
    vec![
        [cx, base.y],
        [base.x + base.width, cy],
        [cx, base.y + base.height],
        [base.x, cy],
    ]
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
        color_to_svg, render_svg, sanitize_id, BackgroundMode, DefIdAllocator, FontRegistry,
        ImagePolicy, RenderError, RenderOptions, RenderOutput, RenderWarning, SvgDocument, SvgNode,
        TextPolicy, UnsupportedElementMode,
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
    fn renders_svg_text_with_font_mapping_and_multiline_layout() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[
                    {
                        "type":"text",
                        "id":"text",
                        "x":10,
                        "y":20,
                        "width":120,
                        "height":80,
                        "strokeColor":"#123456",
                        "fontSize":20,
                        "fontFamily":3,
                        "lineHeight":1.5,
                        "textAlign":"center",
                        "verticalAlign":"middle",
                        "text":"Hello\nworld"
                    }
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let output = render_svg(
            &scene,
            &RenderOptions {
                background: BackgroundMode::Transparent,
                ..RenderOptions::default()
            },
        )?;

        ensure(output.value.contains("<text"), "text node")?;
        ensure(
            output
                .value
                .contains("font-family=\"Cascadia Code, Courier New, monospace\""),
            "font family",
        )?;
        ensure(output.value.contains("text-anchor=\"middle\""), "alignment")?;
        ensure(output.value.contains("x=\"70\""), "center x")?;
        ensure(output.value.contains("y=\"46\""), "middle first baseline")?;
        ensure(output.value.contains("dy=\"30\""), "line height")?;
        ensure(output.value.contains(">Hello</tspan>"), "first line")?;
        ensure(output.value.contains(">world</tspan>"), "second line")?;
        usvg::Tree::from_str(&output.value, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn renders_bound_text_inside_shape_containers() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[
                    {
                        "type":"rectangle",
                        "id":"box",
                        "x":100,
                        "y":50,
                        "width":200,
                        "height":100,
                        "boundElements":[{"id":"label","type":"text"}]
                    },
                    {
                        "type":"text",
                        "id":"label",
                        "containerId":"box",
                        "fontSize":20,
                        "fontFamily":2,
                        "textAlign":"right",
                        "verticalAlign":"bottom",
                        "text":"Inside"
                    }
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let output = render_svg(
            &scene,
            &RenderOptions {
                background: BackgroundMode::Transparent,
                quality: super::RenderQuality::Clean,
                ..RenderOptions::default()
            },
        )?;

        ensure(output.value.contains("text-anchor=\"end\""), "right anchor")?;
        ensure(
            output.value.contains("x=\"292\""),
            "container right padding",
        )?;
        ensure(output.value.contains("y=\"133\""), "bottom baseline")?;
        usvg::Tree::from_str(&output.value, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn missing_container_and_arrow_label_text_fall_back_to_stored_text_box(
    ) -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[
                    {
                        "type":"arrow",
                        "id":"arrow",
                        "x":0,
                        "y":0,
                        "points":[[0,0],[100,0]],
                        "boundElements":[{"id":"arrow-label","type":"text"}]
                    },
                    {
                        "type":"text",
                        "id":"arrow-label",
                        "containerId":"arrow",
                        "x":40,
                        "y":10,
                        "width":60,
                        "height":30,
                        "fontSize":10,
                        "textAlign":"center",
                        "text":"A"
                    },
                    {
                        "type":"text",
                        "id":"missing-label",
                        "containerId":"missing",
                        "x":200,
                        "y":10,
                        "width":80,
                        "height":30,
                        "fontSize":10,
                        "textAlign":"right",
                        "text":"Missing"
                    }
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let output = render_svg(
            &scene,
            &RenderOptions {
                background: BackgroundMode::Transparent,
                ..RenderOptions::default()
            },
        )?;

        ensure(
            output.value.contains("x=\"70\""),
            "arrow label stored center",
        )?;
        ensure(
            output.value.contains("x=\"280\""),
            "missing container fallback",
        )?;
        ensure(output.value.contains(">A</tspan>"), "arrow label text")?;
        ensure(output.value.contains(">Missing</tspan>"), "fallback text")?;
        usvg::Tree::from_str(&output.value, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn font_registry_measures_unicode_width_and_line_height() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[
                    {"type":"text","id":"text","fontSize":10,"fontFamily":3,"lineHeight":2,"text":"ab\n界"}
                ]
            }"##,
        )?;
        let [excalidraw_core::Element::Text(text)] = file.elements.as_slice() else {
            return Err("expected text".into());
        };
        let measurement = FontRegistry::new().measure_text(text);

        ensure_eq(&measurement.width, 12.4_f64, "unicode width")?;
        ensure_eq(&measurement.height, 40.0_f64, "text height")?;
        ensure_eq(&measurement.line_height, 20.0_f64, "line height")?;
        Ok(())
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

    #[test]
    fn full_quality_renders_basic_shapes_through_rough_paths() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[
                    {"type":"rectangle","id":"rect","x":0,"y":0,"width":40,"height":20,"seed":42,"backgroundColor":"#ffeeaa"},
                    {"type":"ellipse","id":"ellipse","x":50,"y":0,"width":40,"height":20,"seed":43,"backgroundColor":"#aaddff"},
                    {"type":"diamond","id":"diamond","x":100,"y":0,"width":40,"height":40,"seed":44,"backgroundColor":"#ddffaa"}
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let options = RenderOptions {
            background: BackgroundMode::Transparent,
            quality: super::RenderQuality::Full,
            ..RenderOptions::default()
        };
        let first = render_svg(&scene, &options)?;
        let second = render_svg(&scene, &options)?;

        ensure_eq(&first.value, second.value, "seeded rough output")?;
        ensure(first.value.contains("<path"), "rough path output")?;
        usvg::Tree::from_str(&first.value, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn clean_quality_uses_geometric_primitives() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[
                    {"type":"rectangle","id":"rect","x":0,"y":0,"width":40,"height":20},
                    {"type":"ellipse","id":"ellipse","x":50,"y":0,"width":40,"height":20},
                    {"type":"diamond","id":"diamond","x":100,"y":0,"width":40,"height":40}
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let output = render_svg(
            &scene,
            &RenderOptions {
                background: BackgroundMode::Transparent,
                quality: super::RenderQuality::Clean,
                ..RenderOptions::default()
            },
        )?;

        ensure(output.value.contains("<rect"), "clean rectangle")?;
        ensure(output.value.contains("<ellipse"), "clean ellipse")?;
        ensure(output.value.contains("<polygon"), "clean diamond")?;
        ensure(!output.value.contains("<path"), "no rough paths")?;
        usvg::Tree::from_str(&output.value, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn rough_modes_accept_excalidraw_fill_styles() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[
                    {"type":"rectangle","id":"hachure","x":0,"y":0,"width":20,"height":20,"fillStyle":"hachure","backgroundColor":"#ffeeaa","seed":1},
                    {"type":"rectangle","id":"solid","x":30,"y":0,"width":20,"height":20,"fillStyle":"solid","backgroundColor":"#ffeeaa","seed":2},
                    {"type":"rectangle","id":"cross","x":60,"y":0,"width":20,"height":20,"fillStyle":"cross-hatch","backgroundColor":"#ffeeaa","seed":3},
                    {"type":"rectangle","id":"dots","x":90,"y":0,"width":20,"height":20,"fillStyle":"dots","backgroundColor":"#ffeeaa","seed":4},
                    {"type":"rectangle","id":"dashed","x":120,"y":0,"width":20,"height":20,"fillStyle":"dashed","backgroundColor":"#ffeeaa","seed":5},
                    {"type":"rectangle","id":"zigzag","x":150,"y":0,"width":20,"height":20,"fillStyle":"zigzag-line","backgroundColor":"#ffeeaa","seed":6},
                    {"type":"rectangle","id":"none","x":180,"y":0,"width":20,"height":20,"fillStyle":"none","backgroundColor":"#ffeeaa","seed":7}
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let output = render_svg(
            &scene,
            &RenderOptions {
                background: BackgroundMode::Transparent,
                quality: super::RenderQuality::FastSvg,
                ..RenderOptions::default()
            },
        )?;

        ensure(output.value.contains("<path"), "rough fill style paths")?;
        usvg::Tree::from_str(&output.value, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn shared_style_serializes_dash_opacity_roundness_and_rotation() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[
                    {
                        "type":"rectangle",
                        "id":"styled",
                        "x":10,
                        "y":20,
                        "width":40,
                        "height":20,
                        "angle":1.5707963267948966,
                        "opacity":50,
                        "strokeWidth":3,
                        "strokeStyle":"dashed",
                        "roundness":{"type":3},
                        "backgroundColor":"#abcdef"
                    }
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let clean = render_svg(
            &scene,
            &RenderOptions {
                background: BackgroundMode::Transparent,
                quality: super::RenderQuality::Clean,
                ..RenderOptions::default()
            },
        )?;

        ensure(clean.value.contains("stroke-dasharray=\"12 12\""), "dash")?;
        ensure(clean.value.contains("opacity=\"0.5\""), "opacity")?;
        ensure(clean.value.contains("rx=\"5\""), "roundness")?;
        ensure(
            clean.value.contains("transform=\"rotate(90 30 30)\""),
            "rotation",
        )?;

        let rough = render_svg(
            &scene,
            &RenderOptions {
                background: BackgroundMode::Transparent,
                quality: super::RenderQuality::Full,
                ..RenderOptions::default()
            },
        )?;
        ensure(
            rough.value.contains("stroke-dasharray=\"12 12\""),
            "rough dash",
        )?;
        ensure(rough.value.contains("opacity=\"0.5\""), "rough opacity")?;
        Ok(())
    }

    #[test]
    fn renders_lines_from_stored_points_with_dash_styles() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[
                    {
                        "type":"line",
                        "id":"line",
                        "x":5,
                        "y":7,
                        "strokeWidth":2,
                        "strokeStyle":"dotted",
                        "points":[[0,0],[10,5],[20,0]]
                    }
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let output = render_svg(
            &scene,
            &RenderOptions {
                background: BackgroundMode::Transparent,
                ..RenderOptions::default()
            },
        )?;

        ensure(output.value.contains("d=\"M5 7 L15 12 L25 7\""), "path")?;
        ensure(
            output.value.contains("stroke-dasharray=\"2 4\""),
            "dotted dash",
        )?;
        usvg::Tree::from_str(&output.value, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn renders_explicit_arrowhead_geometry_for_supported_types() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[
                    {"type":"arrow","id":"arrow","x":0,"y":0,"strokeWidth":2,"points":[[0,0],[40,0]],"startArrowhead":"arrow","endArrowhead":"triangle"},
                    {"type":"arrow","id":"outline","x":0,"y":20,"strokeWidth":2,"points":[[0,0],[40,0]],"startArrowhead":"triangle_outline","endArrowhead":"bar"},
                    {"type":"arrow","id":"dots","x":0,"y":40,"strokeWidth":2,"points":[[0,0],[40,0]],"startArrowhead":"dot","endArrowhead":"circle"},
                    {"type":"arrow","id":"diamond","x":0,"y":60,"strokeWidth":2,"points":[[0,0],[40,0]],"startArrowhead":"diamond","endArrowhead":"crowfoot"}
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let output = render_svg(
            &scene,
            &RenderOptions {
                background: BackgroundMode::Transparent,
                ..RenderOptions::default()
            },
        )?;

        ensure(output.value.contains("<polygon"), "polygon heads")?;
        ensure(output.value.contains("<circle"), "circle heads")?;
        ensure(output.value.contains("fill=\"none\""), "outline head")?;
        ensure(output.value.matches("<path").count() >= 5, "path heads")?;
        usvg::Tree::from_str(&output.value, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn renders_freedraw_as_polyline() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[{"type":"freedraw","id":"fd","x":10,"y":20,"width":50,"height":30,
                    "points":[[0,0],[10,5],[30,0],[50,10]],
                    "pressures":[0.2,0.5,0.8,0.3],"simulatePressure":true}]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let output = render_svg(&scene, &RenderOptions::default())?;
        ensure(output.value.contains("<path"), "freedraw path")?;
        ensure(
            output.value.contains("stroke-linecap=\"round\""),
            "round cap",
        )?;
        ensure(
            output.value.contains("stroke-linejoin=\"round\""),
            "round join",
        )?;
        usvg::Tree::from_str(&output.value, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn renders_freedraw_with_simulated_pressure() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[{"type":"freedraw","id":"fd2","x":0,"y":0,"width":20,"height":20,
                    "points":[[0,0],[10,10],[20,0]],
                    "pressures":[0.1,0.9,0.1],"simulatePressure":false}]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let output = render_svg(&scene, &RenderOptions::default())?;
        ensure(output.value.contains("<path"), "freedraw pressure path")?;
        usvg::Tree::from_str(&output.value, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn renders_image_with_data_url() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[{"type":"image","id":"img1","x":0,"y":0,"width":100,"height":80,
                    "fileId":"file123","status":"saved"}],
                "files":{"file123":{"id":"file123","mimeType":"image/png",
                    "dataURL":"data:image/png;base64,iVBOR"}}
            }"##,
        )?;
        let scene = normalize_file(&file);
        let output = render_svg(&scene, &RenderOptions::default())?;
        ensure(output.value.contains("<image"), "image element")?;
        ensure(
            output.value.contains("data:image/png;base64,iVBOR"),
            "data url",
        )?;
        usvg::Tree::from_str(&output.value, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn renders_image_placeholder_when_missing_data() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[{"type":"image","id":"img2","x":10,"y":20,"width":100,"height":80,
                    "fileId":"missing","status":"saved"}]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let output = render_svg(&scene, &RenderOptions::default())?;
        ensure(
            output.warnings.iter().any(|w| {
                matches!(
                    w,
                    RenderWarning::MissingImageData { element_id } if element_id == "img2"
                )
            }),
            "missing image warning",
        )?;
        ensure(output.value.contains("no image"), "placeholder text")?;
        usvg::Tree::from_str(&output.value, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn renders_image_with_crop_and_scale_flips() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[{"type":"image","id":"img3","x":0,"y":0,"width":100,"height":80,
                    "fileId":"f1","scale":[-1,1],
                    "crop":{"x":10,"y":10,"width":80,"height":60,
                            "naturalWidth":100,"naturalHeight":80}}],
                "files":{"f1":{"id":"f1","mimeType":"image/png",
                    "dataURL":"data:image/png;base64,iVBOR"}}
            }"##,
        )?;
        let scene = normalize_file(&file);
        let output = render_svg(&scene, &RenderOptions::default())?;
        ensure(output.value.contains("clipPath"), "crop clip path")?;
        ensure(output.value.contains("clip-path"), "clip ref")?;
        ensure(output.value.contains("scale("), "scale flip")?;
        usvg::Tree::from_str(&output.value, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn renders_frame_with_label_and_border() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[{"type":"frame","id":"fr1","x":0,"y":0,"width":200,"height":150,
                    "name":"My Frame"}]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let output = render_svg(&scene, &RenderOptions::default())?;
        ensure(output.value.contains("data-frame"), "frame group")?;
        ensure(output.value.contains("My Frame"), "frame label")?;
        ensure(
            output.value.contains("stroke-dasharray"),
            "frame dashed border",
        )?;
        usvg::Tree::from_str(&output.value, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn renders_collapsed_frame() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[{"type":"frame","id":"fr2","x":0,"y":0,"width":200,"height":150,
                    "name":"Collapsed","isCollapsed":true}]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let output = render_svg(&scene, &RenderOptions::default())?;
        ensure(output.value.contains("collapsed"), "collapsed indicator")?;
        usvg::Tree::from_str(&output.value, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn renders_magicframe_as_frame() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[{"type":"magicframe","id":"mf1","x":0,"y":0,"width":200,"height":150,
                    "name":"Magic"}]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let output = render_svg(&scene, &RenderOptions::default())?;
        ensure(output.value.contains("Magic"), "magicframe label")?;
        usvg::Tree::from_str(&output.value, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn renders_unsupported_elements_as_placeholders() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[
                    {"type":"embeddable","id":"emb1","x":0,"y":0,"width":200,"height":100},
                    {"type":"iframe","id":"ifr1","x":0,"y":120,"width":200,"height":100}
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let output = render_svg(&scene, &RenderOptions::default())?;
        ensure(
            output.value.contains("data-placeholder"),
            "placeholder attribute",
        )?;
        ensure(output.value.contains("unsupported"), "placeholder label")?;
        usvg::Tree::from_str(&output.value, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn renders_unknown_elements_as_placeholders() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[{"type":"customWidget","id":"cw1","x":50,"y":50,"width":120,"height":80}]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let output = render_svg(&scene, &RenderOptions::default())?;
        ensure(
            output.warnings.iter().any(|w| matches!(
                w,
                RenderWarning::UnknownElementPlaceholder { element_type, .. } if element_type == "customWidget"
            )),
            "unknown element warning",
        )?;
        ensure(
            output.value.contains("customWidget"),
            "unknown element label",
        )?;
        usvg::Tree::from_str(&output.value, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn skips_unsupported_and_unknown_when_mode_is_skip() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements":[
                    {"type":"embeddable","id":"emb2","x":0,"y":0,"width":100,"height":100},
                    {"type":"iframe","id":"ifr2","x":120,"y":0,"width":100,"height":100},
                    {"type":"customWidget","id":"cw2","x":240,"y":0,"width":100,"height":100}
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let opts = RenderOptions {
            unsupported: UnsupportedElementMode::Skip,
            ..RenderOptions::default()
        };
        let output = render_svg(&scene, &opts)?;
        ensure(
            !output.value.contains("data-placeholder"),
            "no placeholders when skip",
        )?;
        usvg::Tree::from_str(&output.value, &usvg::Options::default())?;
        Ok(())
    }

    #[test]
    fn all_fixtures_parse_normalize_and_render() -> Result<(), Box<dyn Error>> {
        let fixtures = [
            ("simple_shapes", 3),
            ("text_standalone", 2),
            ("text_containers", 2),
            ("arrows_basic", 3),
            ("arrows_bound", 4),
            ("freedraw", 2),
            ("image_embed", 3),
            ("frame_clip", 4),
            ("unsupported", 3),
            ("complex_diagram", 8),
            ("large_200_elements", 200),
        ];
        for (name, expected_count) in fixtures {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("tests/fixtures")
                .join(format!("{name}.excalidraw"));
            let raw = std::fs::read_to_string(&path).map_err(|e| format!("fixture {name}: {e}"))?;
            let file = parse_str(&raw)
                .map_err(|e: excalidraw_core::ParseError| format!("parse {name}: {e}"))?;
            ensure_eq(
                &file.elements.len(),
                expected_count,
                &format!("{name} element count"),
            )?;
            let scene = normalize_file(&file);
            ensure(
                scene.elements.len() == expected_count,
                &format!(
                    "{name}: expected {expected_count} normalized elements, got {}",
                    scene.elements.len()
                ),
            )?;
            let output = render_svg(&scene, &RenderOptions::default())
                .map_err(|e| format!("render {name}: {e}"))?;
            ensure(
                output.value.contains("<svg"),
                &format!("{name}: missing svg root"),
            )?;
            usvg::Tree::from_str(&output.value, &usvg::Options::default())
                .map_err(|e| format!("{name} usvg: {e}"))?;
        }
        Ok(())
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
