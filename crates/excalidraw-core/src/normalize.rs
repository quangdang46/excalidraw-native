//! Scene normalization for renderer consumers.

use std::collections::HashMap;

use crate::{
    parse_excalidraw_color, BaseElement, Color, Element, ExcalidrawFile, LinearElement, TextElement,
};

const FRACTIONAL_INDEX_DIGITS: &str =
    "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const MIN_FRACTIONAL_INDEX: &str = "A00000000000000000000000000";

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
        let bounds = element_bounds(element, abs_points.as_deref());
        content_bounds = if elements.is_empty() {
            bounds
        } else {
            content_bounds.union(bounds)
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
            rotated_bounds: bounds,
            container_id: text_element(element).and_then(|text| text.container_id.clone()),
            frame_id: base.frame_id.clone(),
        });
    }

    apply_z_order(&mut elements, &mut warnings);
    let id_map = build_id_map(&elements);

    Scene {
        export_bounds: content_bounds.padded(16.0),
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

fn element_bounds(element: &Element, abs_points: Option<&[Point]>) -> Rect {
    if let Some(points) = abs_points {
        return Rect::from_points(points);
    }

    match element_base(element) {
        Some(base) => Rect {
            x: base.x,
            y: base.y,
            width: base.width,
            height: base.height,
        },
        None => Rect::empty(),
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
                x: 0.0,
                y: 7.0,
                width: 15.0,
                height: 20.0,
            },
            "line bounds",
        )?;
        ensure_eq(&scene.content_bounds, line.bounds, "scene bounds")
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
