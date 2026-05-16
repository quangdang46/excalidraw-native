//! Scene normalization for renderer consumers.

use std::collections::HashMap;

use crate::{
    parse_excalidraw_color, Arrowhead, BaseElement, Color, Element, ExcalidrawFile, FrameElement,
    FreedrawElement, ImageElement, LinearElement, ShapeElement, TextElement, UnsupportedElement,
};

const FRACTIONAL_INDEX_DIGITS: &str =
    "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const MIN_FRACTIONAL_INDEX: &str = "A00000000000000000000000000";
const EXPORT_PADDING: f64 = 16.0;
const ROUGHNESS_BOUNDS_MARGIN: f64 = 1.5;
const TEXT_WIDTH_FACTOR: f64 = 0.62;
const FRAME_LABEL_HEIGHT: f64 = 32.0;
const FRAME_LABEL_HORIZONTAL_PADDING: f64 = 16.0;
const UNSUPPORTED_PLACEHOLDER_MIN_SIZE: f64 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }

    #[must_use]
    pub fn from_points(points: &[Point]) -> Self {
        let Some(first) = points.first() else {
            return Self::empty();
        };

        let mut min_x = first.x;
        let mut min_y = first.y;
        let mut max_x = first.x;
        let mut max_y = first.y;

        for point in points {
            min_x = min_x.min(point.x);
            min_y = min_y.min(point.y);
            max_x = max_x.max(point.x);
            max_y = max_y.max(point.y);
        }

        Self {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        }
    }

    #[must_use]
    pub fn union(self, other: Self) -> Self {
        if self.width == 0.0 && self.height == 0.0 {
            return other;
        }
        if other.width == 0.0 && other.height == 0.0 {
            return self;
        }

        let min_x = self.x.min(other.x);
        let min_y = self.y.min(other.y);
        let max_x = (self.x + self.width).max(other.x + other.width);
        let max_y = (self.y + self.height).max(other.y + other.height);

        Self {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        }
    }

    #[must_use]
    pub fn padded(self, padding: f64) -> Self {
        Self {
            x: self.x - padding,
            y: self.y - padding,
            width: self.width + padding * 2.0,
            height: self.height + padding * 2.0,
        }
    }

    #[must_use]
    pub fn normalized(self) -> Self {
        let x = if self.width < 0.0 {
            self.x + self.width
        } else {
            self.x
        };
        let y = if self.height < 0.0 {
            self.y + self.height
        } else {
            self.y
        };

        Self {
            x,
            y,
            width: self.width.abs(),
            height: self.height.abs(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Scene {
    pub elements: Vec<NormalizedElement>,
    pub id_map: HashMap<String, usize>,
    pub frame_children: HashMap<String, Vec<String>>,
    pub bound_texts: HashMap<String, Vec<String>>,
    pub bound_arrows: HashMap<String, Vec<String>>,
    pub background_color: Color,
    pub content_bounds: Rect,
    pub export_bounds: Rect,
    pub warnings: Vec<SceneWarning>,
}

#[derive(Debug, Clone)]
pub struct NormalizedElement {
    pub element: Element,
    pub original_order: usize,
    pub render_order: usize,
    pub abs_points: Option<Vec<Point>>,
    pub bounds: Rect,
    pub rotated_bounds: Rect,
    pub container_id: Option<String>,
    pub frame_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneWarning {
    MissingElementId { original_order: usize },
    InvalidBackgroundColor { value: String },
    ZOrderFallback { reason: ZOrderFallbackReason },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZOrderFallbackReason {
    MissingIndex { element_id: String },
    InvalidIndex { element_id: String, value: String },
    DuplicateIndex { value: String },
}

pub fn normalize_file(file: &ExcalidrawFile) -> Scene {
    let mut warnings = Vec::new();
    let background_color = match file
        .app_state
        .view_background_color
        .as_deref()
        .map(parse_excalidraw_color)
    {
        Some(Ok(color)) => color,
        Some(Err(_)) => {
            let value = file
                .app_state
                .view_background_color
                .clone()
                .unwrap_or_default();
            warnings.push(SceneWarning::InvalidBackgroundColor { value });
            Color::rgb(255, 255, 255)
        }
        None => Color::rgb(255, 255, 255),
    };

    let mut elements = Vec::new();
    let mut frame_children: HashMap<String, Vec<String>> = HashMap::new();
    let mut bound_texts: HashMap<String, Vec<String>> = HashMap::new();
    let mut bound_arrows: HashMap<String, Vec<String>> = HashMap::new();
    let mut content_bounds = Rect::empty();

    for (original_order, element) in file.elements.iter().enumerate() {
        let Some(base) = element_base(element) else {
            continue;
        };
        if base.is_deleted {
            continue;
        }
        if base.id.is_empty() {
            warnings.push(SceneWarning::MissingElementId { original_order });
        }

        index_bound_elements(base, &mut bound_texts, &mut bound_arrows);

        let abs_points = element_abs_points(element);
        let (bounds, rotated_bounds) = element_bounds(element, abs_points.as_deref());
        content_bounds = if elements.is_empty() {
            rotated_bounds
        } else {
            content_bounds.union(rotated_bounds)
        };

        if let Some(frame_id) = &base.frame_id {
            frame_children
                .entry(frame_id.clone())
                .or_default()
                .push(base.id.clone());
        }
        if let Some(text) = text_element(element) {
            if let Some(container_id) = &text.container_id {
                push_unique(
                    bound_texts.entry(container_id.clone()).or_default(),
                    text.base.id.clone(),
                );
            }
        }

        elements.push(NormalizedElement {
            element: element.clone(),
            original_order,
            render_order: elements.len(),
            abs_points,
            bounds,
            rotated_bounds,
            container_id: text_element(element).and_then(|text| text.container_id.clone()),
            frame_id: base.frame_id.clone(),
        });
    }

    apply_z_order(&mut elements, &mut warnings);
    let id_map = build_id_map(&elements);

    Scene {
        export_bounds: content_bounds.padded(EXPORT_PADDING),
        elements,
        id_map,
        frame_children,
        bound_texts,
        bound_arrows,
        background_color,
        content_bounds,
        warnings,
    }
}

impl ExcalidrawFile {
    #[must_use]
    pub fn normalize(&self) -> Scene {
        normalize_file(self)
    }
}

fn build_id_map(elements: &[NormalizedElement]) -> HashMap<String, usize> {
    let mut id_map = HashMap::new();
    for (index, element) in elements.iter().enumerate() {
        let Some(base) = element_base(&element.element) else {
            continue;
        };
        if !base.id.is_empty() {
            id_map.insert(base.id.clone(), index);
        }
    }
    id_map
}

fn apply_z_order(elements: &mut [NormalizedElement], warnings: &mut Vec<SceneWarning>) {
    let any_index = elements
        .iter()
        .filter_map(|element| element_base(&element.element))
        .any(|base| base.index.is_some());
    if !any_index {
        apply_original_order(elements);
        return;
    }

    let mut seen = std::collections::HashSet::new();

    for element in elements.iter() {
        let Some(base) = element_base(&element.element) else {
            continue;
        };
        let Some(index) = &base.index else {
            warnings.push(SceneWarning::ZOrderFallback {
                reason: ZOrderFallbackReason::MissingIndex {
                    element_id: base.id.clone(),
                },
            });
            apply_original_order(elements);
            return;
        };
        if !is_valid_fractional_index(index) {
            warnings.push(SceneWarning::ZOrderFallback {
                reason: ZOrderFallbackReason::InvalidIndex {
                    element_id: base.id.clone(),
                    value: index.clone(),
                },
            });
            apply_original_order(elements);
            return;
        }
        if !seen.insert(index.as_str()) {
            warnings.push(SceneWarning::ZOrderFallback {
                reason: ZOrderFallbackReason::DuplicateIndex {
                    value: index.clone(),
                },
            });
            apply_original_order(elements);
            return;
        }
    }

    elements.sort_by(|left, right| {
        let left_index = element_base(&left.element).and_then(|base| base.index.as_deref());
        let right_index = element_base(&right.element).and_then(|base| base.index.as_deref());
        left_index
            .cmp(&right_index)
            .then(left.original_order.cmp(&right.original_order))
    });
    apply_render_order(elements);
}

fn apply_original_order(elements: &mut [NormalizedElement]) {
    elements.sort_by_key(|element| element.original_order);
    apply_render_order(elements);
}

fn apply_render_order(elements: &mut [NormalizedElement]) {
    for (render_order, element) in elements.iter_mut().enumerate() {
        element.render_order = render_order;
    }
}

fn is_valid_fractional_index(index: &str) -> bool {
    let Some(integer_part_length) = fractional_index_integer_length(index) else {
        return false;
    };
    if integer_part_length > index.len()
        || index == MIN_FRACTIONAL_INDEX
        || !index
            .bytes()
            .all(|byte| FRACTIONAL_INDEX_DIGITS.as_bytes().contains(&byte))
    {
        return false;
    }
    let Some(fractional_part) = index.get(integer_part_length..) else {
        return false;
    };
    !fractional_part.ends_with('0')
}

fn fractional_index_integer_length(index: &str) -> Option<usize> {
    match *index.as_bytes().first()? {
        head @ b'a'..=b'z' => Some(usize::from(head - b'a' + 2)),
        head @ b'A'..=b'Z' => Some(usize::from(b'Z' - head + 2)),
        _ => None,
    }
}

fn element_base(element: &Element) -> Option<&BaseElement> {
    match element {
        Element::Rectangle(shape) | Element::Ellipse(shape) | Element::Diamond(shape) => {
            Some(&shape.base)
        }
        Element::Arrow(linear) | Element::Line(linear) => Some(&linear.base),
        Element::Text(text) => Some(&text.base),
        Element::Freedraw(freedraw) => Some(&freedraw.base),
        Element::Image(image) => Some(&image.base),
        Element::Frame(frame) | Element::MagicFrame(frame) => Some(&frame.base),
        Element::Embeddable(unsupported) | Element::Iframe(unsupported) => Some(&unsupported.base),
        Element::Unknown { .. } => None,
    }
}

fn text_element(element: &Element) -> Option<&TextElement> {
    match element {
        Element::Text(text) => Some(text),
        _ => None,
    }
}

fn linear_element(element: &Element) -> Option<&LinearElement> {
    match element {
        Element::Arrow(linear) | Element::Line(linear) => Some(linear),
        _ => None,
    }
}

fn index_bound_elements(
    base: &BaseElement,
    bound_texts: &mut HashMap<String, Vec<String>>,
    bound_arrows: &mut HashMap<String, Vec<String>>,
) {
    for bound in &base.bound_elements {
        match bound.element_type.as_str() {
            "text" => push_unique(
                bound_texts.entry(base.id.clone()).or_default(),
                bound.id.clone(),
            ),
            "arrow" => push_unique(
                bound_arrows.entry(base.id.clone()).or_default(),
                bound.id.clone(),
            ),
            _ => {}
        }
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn element_abs_points(element: &Element) -> Option<Vec<Point>> {
    let linear = linear_element(element)?;
    Some(
        linear
            .points
            .iter()
            .map(|&[x, y]| Point {
                x: linear.base.x + x,
                y: linear.base.y + y,
            })
            .collect(),
    )
}

fn element_bounds(element: &Element, abs_points: Option<&[Point]>) -> (Rect, Rect) {
    let bounds = match element {
        Element::Rectangle(shape) | Element::Ellipse(shape) | Element::Diamond(shape) => {
            shape_bounds(shape)
        }
        Element::Arrow(linear) | Element::Line(linear) => linear_bounds(linear, abs_points),
        Element::Text(text) => text_bounds(text),
        Element::Freedraw(freedraw) => freedraw_bounds(freedraw),
        Element::Image(image) => image_bounds(image),
        Element::Frame(frame) | Element::MagicFrame(frame) => frame_bounds(frame),
        Element::Embeddable(unsupported) | Element::Iframe(unsupported) => {
            unsupported_bounds(unsupported)
        }
        Element::Unknown { .. } => Rect::empty(),
    };
    let rotated_bounds = rotate_bounds(bounds, element_base(element));
    (bounds, rotated_bounds)
}

fn shape_bounds(shape: &ShapeElement) -> Rect {
    base_rect(&shape.base).padded(element_padding(&shape.base))
}

fn linear_bounds(linear: &LinearElement, abs_points: Option<&[Point]>) -> Rect {
    let mut bounds = abs_points
        .filter(|points| !points.is_empty())
        .map(Rect::from_points)
        .unwrap_or_else(|| base_rect(&linear.base));
    bounds = bounds.padded(element_padding(&linear.base));

    if let Some(points) = abs_points {
        if let (Some(first), Some(last)) = (points.first(), points.last()) {
            if linear.start_arrowhead.is_some() {
                bounds = bounds.union(arrowhead_bounds(
                    *first,
                    &linear.base,
                    &linear.start_arrowhead,
                ));
            }
            if linear.end_arrowhead.is_some() {
                bounds = bounds.union(arrowhead_bounds(*last, &linear.base, &linear.end_arrowhead));
            }
        }
    }

    bounds
}

fn freedraw_bounds(freedraw: &FreedrawElement) -> Rect {
    if freedraw.points.is_empty() {
        return base_rect(&freedraw.base).padded(element_padding(&freedraw.base));
    }

    let points: Vec<Point> = freedraw
        .points
        .iter()
        .map(|&[x, y]| Point {
            x: freedraw.base.x + x,
            y: freedraw.base.y + y,
        })
        .collect();
    Rect::from_points(&points).padded(element_padding(&freedraw.base))
}

fn text_bounds(text: &TextElement) -> Rect {
    let measured = measure_text(text);
    let base = base_rect(&text.base);
    Rect {
        x: base.x,
        y: base.y,
        width: base.width.max(measured.width),
        height: base.height.max(measured.height),
    }
    .padded(element_padding(&text.base))
}

fn image_bounds(image: &ImageElement) -> Rect {
    let mut bounds = base_rect(&image.base);
    if let Some([scale_x, scale_y]) = image.scale {
        bounds.width *= scale_x.abs();
        bounds.height *= scale_y.abs();
    }
    if let Some(crop) = &image.crop {
        if crop.width > 0.0 && crop.height > 0.0 {
            bounds.width = bounds.width.max(crop.width);
            bounds.height = bounds.height.max(crop.height);
        }
    }
    bounds.padded(element_padding(&image.base))
}

fn frame_bounds(frame: &FrameElement) -> Rect {
    let mut bounds = base_rect(&frame.base).padded(element_padding(&frame.base));
    if let Some(name) = frame.name.as_deref().filter(|name| !name.is_empty()) {
        let label_width =
            name.chars().count() as f64 * 14.0 * TEXT_WIDTH_FACTOR + FRAME_LABEL_HORIZONTAL_PADDING;
        let label_bounds = Rect {
            x: frame.base.x,
            y: frame.base.y - FRAME_LABEL_HEIGHT,
            width: label_width.max(frame.base.width.min(160.0)),
            height: FRAME_LABEL_HEIGHT,
        };
        bounds = bounds.union(label_bounds);
    }
    bounds
}

fn unsupported_bounds(unsupported: &UnsupportedElement) -> Rect {
    let mut bounds = base_rect(&unsupported.base);
    bounds.width = bounds.width.max(UNSUPPORTED_PLACEHOLDER_MIN_SIZE);
    bounds.height = bounds.height.max(UNSUPPORTED_PLACEHOLDER_MIN_SIZE);
    bounds.padded(element_padding(&unsupported.base))
}

fn base_rect(base: &BaseElement) -> Rect {
    Rect {
        x: base.x,
        y: base.y,
        width: base.width,
        height: base.height,
    }
    .normalized()
}

fn element_padding(base: &BaseElement) -> f64 {
    (base.stroke_width.max(0.0) / 2.0) + base.roughness.max(0.0) * ROUGHNESS_BOUNDS_MARGIN
}

fn arrowhead_bounds(point: Point, base: &BaseElement, arrowhead: &Option<Arrowhead>) -> Rect {
    let scale = match arrowhead {
        Some(Arrowhead::Bar) => 4.0,
        Some(Arrowhead::Dot | Arrowhead::Circle | Arrowhead::Diamond) => 5.0,
        Some(_) => 6.0,
        None => 0.0,
    };
    let radius = (base.stroke_width.max(1.0) * scale).max(10.0);
    Rect {
        x: point.x - radius,
        y: point.y - radius,
        width: radius * 2.0,
        height: radius * 2.0,
    }
}

fn measure_text(text: &TextElement) -> Rect {
    let line_count = text.text.lines().count().max(1) as f64;
    let max_chars = text
        .text
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or_default() as f64;
    Rect {
        x: text.base.x,
        y: text.base.y,
        width: max_chars * text.font_size * TEXT_WIDTH_FACTOR,
        height: line_count * text.font_size * text.line_height,
    }
}

fn rotate_bounds(bounds: Rect, base: Option<&BaseElement>) -> Rect {
    let Some(base) = base else {
        return bounds;
    };
    if base.angle == 0.0 {
        return bounds;
    }

    let center = Point {
        x: base.x + base.width / 2.0,
        y: base.y + base.height / 2.0,
    };
    let corners = [
        Point {
            x: bounds.x,
            y: bounds.y,
        },
        Point {
            x: bounds.x + bounds.width,
            y: bounds.y,
        },
        Point {
            x: bounds.x + bounds.width,
            y: bounds.y + bounds.height,
        },
        Point {
            x: bounds.x,
            y: bounds.y + bounds.height,
        },
    ];
    let rotated: Vec<Point> = corners
        .into_iter()
        .map(|point| rotate_point(point, center, base.angle))
        .collect();
    Rect::from_points(&rotated)
}

fn rotate_point(point: Point, center: Point, angle: f64) -> Point {
    let sin = angle.sin();
    let cos = angle.cos();
    let dx = point.x - center.x;
    let dy = point.y - center.y;
    Point {
        x: center.x + dx * cos - dy * sin,
        y: center.y + dx * sin + dy * cos,
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::{
        normalize_file, parse_str, Color, Point, Rect, SceneWarning, ZOrderFallbackReason,
    };

    #[test]
    fn filters_deleted_elements_and_preserves_order() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements": [
                    {"type":"rectangle","id":"a","x":1,"y":2,"width":3,"height":4},
                    {"type":"ellipse","id":"deleted","isDeleted":true},
                    {"type":"diamond","id":"b","x":10,"y":20,"width":30,"height":40}
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);

        ensure_eq(&scene.elements.len(), 2_usize, "visible element count")?;
        let [first, second] = scene.elements.as_slice() else {
            return Err("expected two normalized elements".into());
        };
        ensure_eq(&first.original_order, 0_usize, "first original order")?;
        ensure_eq(&first.render_order, 0_usize, "first render order")?;
        ensure_eq(&second.original_order, 2_usize, "second original order")?;
        ensure_eq(&scene.id_map.get("a"), Some(&0_usize), "id map a")?;
        ensure_eq(&scene.id_map.get("b"), Some(&1_usize), "id map b")
    }

    #[test]
    fn builds_relationship_indexes() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements": [
                    {
                        "type":"rectangle",
                        "id":"container",
                        "boundElements":[
                            {"id":"label","type":"text"},
                            {"id":"arrow","type":"arrow"}
                        ]
                    },
                    {"type":"text","id":"label","containerId":"container"},
                    {"type":"arrow","id":"arrow","frameId":"frame","points":[[0,0],[10,0]]},
                    {"type":"frame","id":"frame"}
                ]
            }"##,
        )?;
        let scene = file.normalize();

        ensure_eq(
            &string_slice(&scene.bound_texts, "container"),
            Some(vec!["label"]),
            "bound texts",
        )?;
        ensure_eq(
            &string_slice(&scene.bound_arrows, "container"),
            Some(vec!["arrow"]),
            "bound arrows",
        )?;
        ensure_eq(
            &string_slice(&scene.frame_children, "frame"),
            Some(vec!["arrow"]),
            "frame children",
        )
    }

    #[test]
    fn converts_linear_points_to_absolute_points_and_bounds() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements": [
                    {"type":"line","id":"line","x":5,"y":7,"points":[[0,0],[10,20],[-5,3]]}
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let [line] = scene.elements.as_slice() else {
            return Err("expected one normalized line".into());
        };

        ensure_eq(
            &line.abs_points.as_deref(),
            Some(
                [
                    Point { x: 5.0, y: 7.0 },
                    Point { x: 15.0, y: 27.0 },
                    Point { x: 0.0, y: 10.0 },
                ]
                .as_slice(),
            ),
            "absolute points",
        )?;
        ensure_eq(
            &line.bounds,
            Rect {
                x: -2.5,
                y: 4.5,
                width: 20.0,
                height: 25.0,
            },
            "line bounds",
        )?;
        ensure_eq(&scene.content_bounds, line.bounds, "scene bounds")
    }

    #[test]
    fn expands_and_rotates_shape_bounds() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements": [
                    {"type":"rectangle","id":"rect","x":0,"y":0,"width":10,"height":20,"angle":1.5707963267948966}
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let [rect] = scene.elements.as_slice() else {
            return Err("expected one normalized rectangle".into());
        };

        ensure_rect_close(
            &rect.bounds,
            Rect {
                x: -2.5,
                y: -2.5,
                width: 15.0,
                height: 25.0,
            },
            "unrotated rectangle bounds",
        )?;
        ensure_rect_close(
            &rect.rotated_bounds,
            Rect {
                x: -7.5,
                y: 2.5,
                width: 25.0,
                height: 15.0,
            },
            "rotated rectangle bounds",
        )?;
        ensure_rect_close(&scene.content_bounds, rect.rotated_bounds, "scene bounds")?;
        ensure_rect_close(
            &scene.export_bounds,
            rect.rotated_bounds.padded(16.0),
            "export bounds",
        )
    }

    #[test]
    fn estimates_text_frame_image_and_unsupported_bounds() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements": [
                    {"type":"text","id":"text","x":0,"y":0,"fontSize":10,"lineHeight":2,"text":"abcd\nxy"},
                    {"type":"frame","id":"frame","x":10,"y":20,"width":100,"height":50,"name":"Board"},
                    {"type":"image","id":"image","x":200,"y":5,"width":20,"height":10,"scale":[-2,3]},
                    {"type":"embeddable","id":"embed","x":5,"y":6}
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let [text, frame, image, embed] = scene.elements.as_slice() else {
            return Err("expected four normalized elements".into());
        };

        ensure_rect_close(
            &text.bounds,
            Rect {
                x: -2.5,
                y: -2.5,
                width: 29.8,
                height: 45.0,
            },
            "text bounds",
        )?;
        ensure_rect_close(
            &frame.bounds,
            Rect {
                x: 7.5,
                y: -12.0,
                width: 105.0,
                height: 84.5,
            },
            "frame label bounds",
        )?;
        ensure_rect_close(
            &image.bounds,
            Rect {
                x: 197.5,
                y: 2.5,
                width: 45.0,
                height: 35.0,
            },
            "scaled image bounds",
        )?;
        ensure_rect_close(
            &embed.bounds,
            Rect {
                x: 2.5,
                y: 3.5,
                width: 29.0,
                height: 29.0,
            },
            "unsupported placeholder bounds",
        )
    }

    #[test]
    fn includes_arrowhead_extents_in_linear_bounds() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements": [
                    {"type":"arrow","id":"arrow","x":0,"y":0,"strokeWidth":4,"roughness":0,"points":[[0,0],[10,0]],"startArrowhead":"dot","endArrowhead":"arrow"}
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);
        let [arrow] = scene.elements.as_slice() else {
            return Err("expected one normalized arrow".into());
        };

        ensure_rect_close(
            &arrow.bounds,
            Rect {
                x: -20.0,
                y: -24.0,
                width: 54.0,
                height: 48.0,
            },
            "arrowhead bounds",
        )
    }

    #[test]
    fn records_background_and_scene_warnings() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "appState": {"viewBackgroundColor":"not-a-color"},
                "elements": [{"type":"rectangle","x":1,"y":2,"width":3,"height":4}]
            }"##,
        )?;
        let scene = normalize_file(&file);

        ensure_eq(
            &scene.background_color,
            Color::rgb(255, 255, 255),
            "invalid background fallback",
        )?;
        ensure_eq(
            &scene.warnings.as_slice(),
            [
                SceneWarning::InvalidBackgroundColor {
                    value: "not-a-color".to_owned(),
                },
                SceneWarning::MissingElementId { original_order: 0 },
            ]
            .as_slice(),
            "scene warnings",
        )
    }

    #[test]
    fn uses_fractional_indexes_when_all_visible_indexes_are_valid() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements": [
                    {"type":"rectangle","id":"last","index":"a2"},
                    {"type":"rectangle","id":"first","index":"a0"},
                    {"type":"rectangle","id":"middle","index":"a1"}
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);

        ensure_eq(
            &ordered_ids(&scene),
            vec!["first", "middle", "last"],
            "z-order",
        )?;
        let [first, middle, last] = scene.elements.as_slice() else {
            return Err("expected three normalized elements".into());
        };
        ensure_eq(&first.original_order, 1_usize, "first original order")?;
        ensure_eq(&middle.original_order, 2_usize, "middle original order")?;
        ensure_eq(&last.original_order, 0_usize, "last original order")?;
        ensure_eq(&scene.id_map.get("first"), Some(&0_usize), "id map first")?;
        ensure_eq(&scene.id_map.get("last"), Some(&2_usize), "id map last")
    }

    #[test]
    fn orders_common_excalidraw_fractional_index_shapes() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements": [
                    {"type":"rectangle","id":"after-a0","index":"a01"},
                    {"type":"rectangle","id":"first-positive","index":"a0"},
                    {"type":"rectangle","id":"negative","index":"Zz"},
                    {"type":"rectangle","id":"next-positive","index":"a1"}
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);

        ensure_eq(
            &ordered_ids(&scene),
            vec!["negative", "first-positive", "after-a0", "next-positive"],
            "fractional index order",
        )
    }

    #[test]
    fn falls_back_to_original_order_when_indexes_are_missing_or_invalid(
    ) -> Result<(), Box<dyn Error>> {
        let missing = parse_str(
            r##"{
                "elements": [
                    {"type":"rectangle","id":"a","index":"a0"},
                    {"type":"rectangle","id":"b"}
                ]
            }"##,
        )?;
        let missing_scene = normalize_file(&missing);
        ensure_eq(
            &ordered_ids(&missing_scene),
            vec!["a", "b"],
            "missing fallback order",
        )?;
        ensure_eq(
            &missing_scene.warnings.as_slice(),
            [SceneWarning::ZOrderFallback {
                reason: ZOrderFallbackReason::MissingIndex {
                    element_id: "b".to_owned(),
                },
            }]
            .as_slice(),
            "missing fallback warning",
        )?;

        let invalid = parse_str(
            r##"{
                "elements": [
                    {"type":"rectangle","id":"a","index":"a0"},
                    {"type":"rectangle","id":"b","index":"a10"}
                ]
            }"##,
        )?;
        let invalid_scene = normalize_file(&invalid);
        ensure_eq(
            &ordered_ids(&invalid_scene),
            vec!["a", "b"],
            "invalid fallback order",
        )?;
        ensure_eq(
            &invalid_scene.warnings.as_slice(),
            [SceneWarning::ZOrderFallback {
                reason: ZOrderFallbackReason::InvalidIndex {
                    element_id: "b".to_owned(),
                    value: "a10".to_owned(),
                },
            }]
            .as_slice(),
            "invalid fallback warning",
        )
    }

    #[test]
    fn falls_back_to_original_order_when_fractional_indexes_are_duplicated(
    ) -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{
                "elements": [
                    {"type":"rectangle","id":"a","index":"a0"},
                    {"type":"rectangle","id":"b","index":"a0"}
                ]
            }"##,
        )?;
        let scene = normalize_file(&file);

        ensure_eq(
            &ordered_ids(&scene),
            vec!["a", "b"],
            "duplicate fallback order",
        )?;
        ensure_eq(
            &scene.warnings.as_slice(),
            [SceneWarning::ZOrderFallback {
                reason: ZOrderFallbackReason::DuplicateIndex {
                    value: "a0".to_owned(),
                },
            }]
            .as_slice(),
            "duplicate fallback warning",
        )
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

    fn ensure_rect_close(actual: &Rect, expected: Rect, label: &str) -> Result<(), Box<dyn Error>> {
        const EPSILON: f64 = 1e-9;
        let close = (actual.x - expected.x).abs() < EPSILON
            && (actual.y - expected.y).abs() < EPSILON
            && (actual.width - expected.width).abs() < EPSILON
            && (actual.height - expected.height).abs() < EPSILON;
        if close {
            Ok(())
        } else {
            Err(format!("{label}: expected {expected:?}, got {actual:?}").into())
        }
    }

    fn string_slice<'a>(
        map: &'a std::collections::HashMap<String, Vec<String>>,
        key: &str,
    ) -> Option<Vec<&'a str>> {
        map.get(key)
            .map(|values| values.iter().map(String::as_str).collect())
    }

    fn ordered_ids(scene: &crate::Scene) -> Vec<&str> {
        scene
            .elements
            .iter()
            .filter_map(|element| super::element_base(&element.element))
            .map(|base| base.id.as_str())
            .collect()
    }
}
