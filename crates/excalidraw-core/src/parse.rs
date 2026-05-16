//! Public parsing API for `.excalidraw` JSON payloads.

use std::io::Read;

use thiserror::Error;

use crate::types::ExcalidrawFile;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("input is not valid Excalidraw JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("failed to read Excalidraw input: {0}")]
    Io(#[from] std::io::Error),
}

pub fn parse_str(input: &str) -> Result<ExcalidrawFile, ParseError> {
    serde_json::from_str(input).map_err(ParseError::from)
}

pub fn parse_slice(input: &[u8]) -> Result<ExcalidrawFile, ParseError> {
    serde_json::from_slice(input).map_err(ParseError::from)
}

pub fn parse_reader(mut input: impl Read) -> Result<ExcalidrawFile, ParseError> {
    let mut buffer = String::new();
    input.read_to_string(&mut buffer)?;
    parse_str(&buffer)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::{parse_reader, parse_slice, parse_str, Element};

    #[test]
    fn parse_str_returns_typed_file() -> Result<(), Box<dyn Error>> {
        let file = parse_str(r#"{"elements":[{"type":"rectangle","id":"r1"}]}"#)?;
        let [Element::Rectangle(rect)] = file.elements.as_slice() else {
            return Err("expected one rectangle".into());
        };

        ensure_eq(&rect.base.id, "r1", "rectangle id")
    }

    #[test]
    fn parse_slice_and_reader_match_parse_str() -> Result<(), Box<dyn Error>> {
        let input = br#"{"elements":[{"type":"text","id":"t1","text":"Hello"}]}"#;
        let from_slice = parse_slice(input)?;
        let from_reader = parse_reader(input.as_slice())?;

        ensure_eq(
            &from_slice.elements.len(),
            from_reader.elements.len(),
            "element count",
        )
    }

    #[test]
    fn corrupt_json_returns_structured_error() -> Result<(), Box<dyn Error>> {
        let error = parse_str("{not-json").map(|_| "unexpected success".to_owned());
        if error.is_err() {
            Ok(())
        } else {
            Err("corrupt JSON should fail".into())
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

    use std::path::Path;

    fn fixture_path(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("tests/fixtures")
            .join(name)
    }

    fn parse_fixture(name: &str) -> Result<crate::ExcalidrawFile, Box<dyn Error>> {
        let raw = std::fs::read_to_string(fixture_path(name))
            .map_err(|e| format!("fixture {name}: {e}"))?;
        parse_str(&raw).map_err(|e| format!("parse {name}: {e}").into())
    }

    fn element_type_names(file: &crate::ExcalidrawFile) -> Vec<String> {
        file.elements
            .iter()
            .map(|e| match e {
                Element::Rectangle(_) => "rectangle",
                Element::Ellipse(_) => "ellipse",
                Element::Diamond(_) => "diamond",
                Element::Arrow(_) => "arrow",
                Element::Line(_) => "line",
                Element::Text(_) => "text",
                Element::Freedraw(_) => "freedraw",
                Element::Image(_) => "image",
                Element::Frame(_) => "frame",
                Element::MagicFrame(_) => "magicframe",
                Element::Embeddable(_) => "embeddable",
                Element::Iframe(_) => "iframe",
                Element::Unknown { element_type, .. } => element_type.as_str(),
            })
            .map(String::from)
            .collect()
    }

    #[test]
    fn parse_simple_shapes_fixture() -> Result<(), Box<dyn Error>> {
        let file = parse_fixture("simple_shapes.excalidraw")?;
        ensure_eq(&file.elements.len(), 3, "element count")?;
        let types = element_type_names(&file);
        ensure(types.contains(&"rectangle".to_owned()), "has rectangle")?;
        ensure(types.contains(&"ellipse".to_owned()), "has ellipse")?;
        ensure(types.contains(&"diamond".to_owned()), "has diamond")?;
        Ok(())
    }

    #[test]
    fn parse_text_standalone_fixture() -> Result<(), Box<dyn Error>> {
        let file = parse_fixture("text_standalone.excalidraw")?;
        ensure_eq(&file.elements.len(), 2, "element count")?;
        let texts: Vec<&Element> = file.elements.iter().collect();
        for elem in &texts {
            if let Element::Text(t) = elem {
                ensure(!t.text.is_empty(), "text not empty")?;
            } else {
                return Err("expected text elements".into());
            }
        }
        Ok(())
    }

    #[test]
    fn parse_text_containers_fixture() -> Result<(), Box<dyn Error>> {
        let file = parse_fixture("text_containers.excalidraw")?;
        ensure_eq(&file.elements.len(), 2, "element count")?;
        let text = file
            .elements
            .iter()
            .find_map(|e| match e {
                Element::Text(t) => Some(t),
                _ => None,
            })
            .ok_or("no text element")?;
        ensure(text.container_id.as_deref() == Some("box1"), "container_id")?;
        Ok(())
    }

    #[test]
    fn parse_arrows_fixture() -> Result<(), Box<dyn Error>> {
        let file = parse_fixture("arrows_basic.excalidraw")?;
        ensure_eq(&file.elements.len(), 3, "element count")?;
        let arrow = file
            .elements
            .iter()
            .find_map(|e| match e {
                Element::Arrow(a) => Some(a),
                _ => None,
            })
            .ok_or("no arrow")?;
        ensure(arrow.points.len() == 2, "arrow has 2 points")?;
        ensure(
            matches!(arrow.end_arrowhead, Some(crate::Arrowhead::Arrow)),
            "end arrowhead type",
        )?;
        Ok(())
    }

    #[test]
    fn parse_bound_arrows_fixture() -> Result<(), Box<dyn Error>> {
        let file = parse_fixture("arrows_bound.excalidraw")?;
        ensure_eq(&file.elements.len(), 4, "element count")?;
        let arrow = file
            .elements
            .iter()
            .find_map(|e| match e {
                Element::Arrow(a) => Some(a),
                _ => None,
            })
            .ok_or("no arrow")?;
        ensure(arrow.start_binding.is_some(), "start binding")?;
        ensure(arrow.end_binding.is_some(), "end binding")?;
        ensure(arrow.base.bound_elements.len() == 1, "arrow has bound text")?;
        Ok(())
    }

    #[test]
    fn parse_freedraw_fixture() -> Result<(), Box<dyn Error>> {
        let file = parse_fixture("freedraw.excalidraw")?;
        ensure_eq(&file.elements.len(), 2, "element count")?;
        let fd: Vec<_> = file
            .elements
            .iter()
            .filter_map(|e| match e {
                Element::Freedraw(f) => Some(f),
                _ => None,
            })
            .collect();
        ensure(fd.len() == 2, "freedraw count")?;
        ensure(!fd[0].points.is_empty(), "freedraw has points")?;
        ensure(!fd[0].pressures.is_empty(), "freedraw has pressures")?;
        Ok(())
    }

    #[test]
    fn parse_image_fixture_with_files() -> Result<(), Box<dyn Error>> {
        let file = parse_fixture("image_embed.excalidraw")?;
        ensure_eq(&file.elements.len(), 3, "element count")?;
        ensure(!file.files.is_empty(), "has files")?;
        let img = file
            .elements
            .iter()
            .find_map(|e| match e {
                Element::Image(i) => Some(i),
                _ => None,
            })
            .ok_or("no image")?;
        ensure(img.file_id.as_deref() == Some("tiny_png"), "fileId")?;
        let file_data = file.files.get("tiny_png").ok_or("no file data")?;
        ensure(file_data.data_url.starts_with("data:image/png"), "data URL")?;
        Ok(())
    }

    #[test]
    fn parse_frame_fixture() -> Result<(), Box<dyn Error>> {
        let file = parse_fixture("frame_clip.excalidraw")?;
        ensure_eq(&file.elements.len(), 4, "element count")?;
        let frames: Vec<_> = file
            .elements
            .iter()
            .filter_map(|e| match e {
                Element::Frame(f) => Some(f),
                _ => None,
            })
            .collect();
        ensure(frames.len() == 2, "frame count")?;
        ensure(frames[0].name.as_deref() == Some("My Frame"), "frame name")?;
        let magic: Vec<_> = file
            .elements
            .iter()
            .filter_map(|e| match e {
                Element::MagicFrame(f) => Some(f),
                _ => None,
            })
            .collect();
        ensure(magic.len() == 1, "magicframe count")?;
        Ok(())
    }

    #[test]
    fn parse_unsupported_and_unknown_preserves_fields() -> Result<(), Box<dyn Error>> {
        let file = parse_fixture("unsupported.excalidraw")?;
        ensure_eq(&file.elements.len(), 3, "element count")?;
        let embeddable = file
            .elements
            .iter()
            .find_map(|e| match e {
                Element::Embeddable(u) => Some(u),
                _ => None,
            })
            .ok_or("no embeddable")?;
        ensure(!embeddable.base.id.is_empty(), "embeddable has id")?;
        let unknown = file
            .elements
            .iter()
            .find_map(|e| match e {
                Element::Unknown { element_type, raw } => Some((element_type, raw)),
                _ => None,
            })
            .ok_or("no unknown")?;
        ensure(unknown.0 == "customWidget", "unknown type")?;
        ensure(
            unknown.1.get("someField").is_some(),
            "preserved unknown field",
        )?;
        Ok(())
    }

    #[test]
    fn parse_complex_diagram_fixture() -> Result<(), Box<dyn Error>> {
        let file = parse_fixture("complex_diagram.excalidraw")?;
        ensure_eq(&file.elements.len(), 8, "element count")?;
        let types = element_type_names(&file);
        ensure(types.contains(&"rectangle".to_owned()), "has rectangle")?;
        ensure(types.contains(&"arrow".to_owned()), "has arrow")?;
        ensure(types.contains(&"diamond".to_owned()), "has diamond")?;
        ensure(types.contains(&"freedraw".to_owned()), "has freedraw")?;
        ensure(types.contains(&"text".to_owned()), "has text")?;
        ensure(types.contains(&"line".to_owned()), "has line")?;
        Ok(())
    }

    #[test]
    fn parse_large_200_fixture() -> Result<(), Box<dyn Error>> {
        let file = parse_fixture("large_200_elements.excalidraw")?;
        ensure_eq(&file.elements.len(), 200, "element count")?;
        Ok(())
    }

    #[test]
    fn parse_preserves_app_state_and_unknown_top_level_fields() -> Result<(), Box<dyn Error>> {
        let file = parse_str(
            r##"{"elements":[],"appState":{"viewBackgroundColor":"#abc","theme":"dark","customField":42},"extraField":"hello"}"##,
        )?;
        ensure(
            file.app_state.view_background_color.as_deref() == Some("#abc"),
            "view bg",
        )?;
        ensure(file.app_state.theme.as_deref() == Some("dark"), "theme")?;
        ensure(
            file.extra.contains_key("extraField"),
            "preserved extra field",
        )?;
        Ok(())
    }

    #[test]
    fn parse_missing_optional_fields_use_defaults() -> Result<(), Box<dyn Error>> {
        let file = parse_str(r#"{"elements":[{"type":"rectangle","id":"r1"}]}"#)?;
        let Element::Rectangle(rect) = &file.elements[0] else {
            return Err("expected rectangle".into());
        };
        ensure(
            (rect.base.opacity - 100.0).abs() < f64::EPSILON,
            "default opacity",
        )?;
        ensure(
            (rect.base.stroke_width - 2.0).abs() < f64::EPSILON,
            "default stroke width",
        )?;
        ensure(
            rect.base.fill_style == crate::FillStyle::Hachure,
            "default fill",
        )?;
        ensure(
            rect.base.stroke_style == crate::StrokeStyle::Solid,
            "default stroke",
        )?;
        Ok(())
    }

    fn ensure(value: bool, label: &str) -> Result<(), Box<dyn Error>> {
        if value {
            Ok(())
        } else {
            Err(label.to_owned().into())
        }
    }
}
