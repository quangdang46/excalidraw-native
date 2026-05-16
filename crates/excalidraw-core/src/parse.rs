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
}
