//! Parse-only validation for Excalidraw payloads.

use serde::Serialize;
use thiserror::Error;

use crate::{
    parse_excalidraw_color, parse_str, BaseElement, Element, ExcalidrawFile, FileData,
    LinearElement, TextElement,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationLimits {
    pub max_elements: usize,
    pub max_element_size_bytes: usize,
    pub max_payload_size_bytes: usize,
    pub max_text_length: usize,
    pub max_points_per_element: usize,
    pub max_files: usize,
    pub max_file_size_bytes: usize,
    pub max_image_data_url_bytes: usize,
}

impl Default for ValidationLimits {
    fn default() -> Self {
        Self {
            max_elements: 10_000,
            max_element_size_bytes: 1_000_000,
            max_payload_size_bytes: 50_000_000,
            max_text_length: 1_000_000,
            max_points_per_element: 100_000,
            max_files: 1_000,
            max_file_size_bytes: 25_000_000,
            max_image_data_url_bytes: 25_000_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationReport {
    pub valid: bool,
    pub element_count: usize,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationReport {
    fn from_parts(
        element_count: usize,
        errors: Vec<ValidationError>,
        warnings: Vec<ValidationWarning>,
    ) -> Self {
        Self {
            valid: errors.is_empty(),
            element_count,
            errors,
            warnings,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize)]
pub enum ValidationError {
    #[error("payload is {actual} bytes, above limit {limit}")]
    PayloadTooLarge { actual: usize, limit: usize },

    #[error("payload is not valid JSON: {message}")]
    InvalidJson { message: String },

    #[error("element count is {actual}, above limit {limit}")]
    TooManyElements { actual: usize, limit: usize },

    #[error("element {element_id} is {actual} bytes, above limit {limit}")]
    ElementTooLarge {
        element_id: String,
        actual: usize,
        limit: usize,
    },

    #[error("text element {element_id} text length is {actual}, above limit {limit}")]
    TextTooLong {
        element_id: String,
        actual: usize,
        limit: usize,
    },

    #[error("linear element {element_id} has {actual} points, above limit {limit}")]
    TooManyPoints {
        element_id: String,
        actual: usize,
        limit: usize,
    },

    #[error("file count is {actual}, above limit {limit}")]
    TooManyFiles { actual: usize, limit: usize },

    #[error("file {file_id} is {actual} bytes, above limit {limit}")]
    FileTooLarge {
        file_id: String,
        actual: usize,
        limit: usize,
    },

    #[error("image data URL for file {file_id} is {actual} bytes, above limit {limit}")]
    ImageDataUrlTooLarge {
        file_id: String,
        actual: usize,
        limit: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ValidationWarning {
    UnknownElementType {
        element_type: String,
    },
    UnsupportedElementType {
        element_id: String,
        element_type: &'static str,
    },
    MissingImage {
        element_id: String,
        file_id: Option<String>,
    },
    InvalidColor {
        element_id: String,
        field: &'static str,
        value: String,
    },
    InvalidIndex {
        element_id: String,
        value: String,
    },
}

impl std::fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationWarning::UnknownElementType { element_type } => {
                write!(f, "unknown element type: {element_type}")
            }
            ValidationWarning::UnsupportedElementType {
                element_id,
                element_type,
            } => {
                write!(f, "unsupported element type {element_type} ({element_id})")
            }
            ValidationWarning::MissingImage {
                element_id,
                file_id,
            } => {
                write!(
                    f,
                    "missing image for element {element_id} (file: {})",
                    file_id.as_deref().unwrap_or("unknown")
                )
            }
            ValidationWarning::InvalidColor {
                element_id,
                field,
                value,
            } => {
                write!(
                    f,
                    "invalid color '{value}' for {field} in element {element_id}"
                )
            }
            ValidationWarning::InvalidIndex { element_id, value } => {
                write!(f, "invalid index '{value}' for element {element_id}")
            }
        }
    }
}

pub fn validate_str(input: &str, limits: &ValidationLimits) -> ValidationReport {
    if input.len() > limits.max_payload_size_bytes {
        return ValidationReport::from_parts(
            0,
            vec![ValidationError::PayloadTooLarge {
                actual: input.len(),
                limit: limits.max_payload_size_bytes,
            }],
            Vec::new(),
        );
    }

    match parse_str(input) {
        Ok(file) => validate_file(&file, limits),
        Err(error) => ValidationReport::from_parts(
            0,
            vec![ValidationError::InvalidJson {
                message: error.to_string(),
            }],
            Vec::new(),
        ),
    }
}

pub fn validate_file(file: &ExcalidrawFile, limits: &ValidationLimits) -> ValidationReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if file.elements.len() > limits.max_elements {
        errors.push(ValidationError::TooManyElements {
            actual: file.elements.len(),
            limit: limits.max_elements,
        });
    }

    if file.files.len() > limits.max_files {
        errors.push(ValidationError::TooManyFiles {
            actual: file.files.len(),
            limit: limits.max_files,
        });
    }

    for (file_id, data) in &file.files {
        validate_file_data(file_id, data, limits, &mut errors);
    }

    for element in &file.elements {
        validate_element(element, file, limits, &mut errors, &mut warnings);
    }

    ValidationReport::from_parts(file.elements.len(), errors, warnings)
}

fn validate_file_data(
    file_id: &str,
    data: &FileData,
    limits: &ValidationLimits,
    errors: &mut Vec<ValidationError>,
) {
    if data.data_url.len() > limits.max_file_size_bytes {
        errors.push(ValidationError::FileTooLarge {
            file_id: file_id.to_owned(),
            actual: data.data_url.len(),
            limit: limits.max_file_size_bytes,
        });
    }

    if data.data_url.len() > limits.max_image_data_url_bytes {
        errors.push(ValidationError::ImageDataUrlTooLarge {
            file_id: file_id.to_owned(),
            actual: data.data_url.len(),
            limit: limits.max_image_data_url_bytes,
        });
    }
}

fn validate_element(
    element: &Element,
    file: &ExcalidrawFile,
    limits: &ValidationLimits,
    errors: &mut Vec<ValidationError>,
    warnings: &mut Vec<ValidationWarning>,
) {
    match element {
        Element::Arrow(linear) | Element::Line(linear) => {
            validate_linear(linear, limits, errors);
            validate_base(&linear.base, warnings);
        }
        Element::Text(text) => {
            validate_text(text, limits, errors);
            validate_base(&text.base, warnings);
        }
        Element::Image(image) => {
            validate_base(&image.base, warnings);
            match image.file_id.as_deref() {
                Some(file_id) => match file.files.get(file_id) {
                    Some(data) if !data.data_url.is_empty() => {}
                    _ => warnings.push(ValidationWarning::MissingImage {
                        element_id: image.base.id.clone(),
                        file_id: Some(file_id.to_owned()),
                    }),
                },
                None => warnings.push(ValidationWarning::MissingImage {
                    element_id: image.base.id.clone(),
                    file_id: None,
                }),
            }
        }
        Element::Embeddable(unsupported) => {
            validate_base(&unsupported.base, warnings);
            warnings.push(ValidationWarning::UnsupportedElementType {
                element_id: unsupported.base.id.clone(),
                element_type: "embeddable",
            });
        }
        Element::Iframe(unsupported) => {
            validate_base(&unsupported.base, warnings);
            warnings.push(ValidationWarning::UnsupportedElementType {
                element_id: unsupported.base.id.clone(),
                element_type: "iframe",
            });
        }
        Element::Unknown { element_type, raw } => {
            let element_id = raw
                .get("id")
                .and_then(|id| id.as_str())
                .unwrap_or("<unknown>")
                .to_owned();
            let actual = raw.to_string().len();
            if actual > limits.max_element_size_bytes {
                errors.push(ValidationError::ElementTooLarge {
                    element_id,
                    actual,
                    limit: limits.max_element_size_bytes,
                });
            }
            warnings.push(ValidationWarning::UnknownElementType {
                element_type: element_type.clone(),
            });
        }
        Element::Rectangle(shape) | Element::Ellipse(shape) | Element::Diamond(shape) => {
            validate_base(&shape.base, warnings);
        }
        Element::Freedraw(freedraw) => {
            validate_base(&freedraw.base, warnings);
            if freedraw.points.len() > limits.max_points_per_element {
                errors.push(ValidationError::TooManyPoints {
                    element_id: freedraw.base.id.clone(),
                    actual: freedraw.points.len(),
                    limit: limits.max_points_per_element,
                });
            }
        }
        Element::Frame(frame) | Element::MagicFrame(frame) => {
            validate_base(&frame.base, warnings);
        }
    }
}

fn validate_base(base: &BaseElement, warnings: &mut Vec<ValidationWarning>) {
    if parse_excalidraw_color(&base.stroke_color).is_err() {
        warnings.push(ValidationWarning::InvalidColor {
            element_id: base.id.clone(),
            field: "strokeColor",
            value: base.stroke_color.clone(),
        });
    }

    if parse_excalidraw_color(&base.background_color).is_err() {
        warnings.push(ValidationWarning::InvalidColor {
            element_id: base.id.clone(),
            field: "backgroundColor",
            value: base.background_color.clone(),
        });
    }

    if let Some(index) = &base.index {
        if !is_plausible_fractional_index(index) {
            warnings.push(ValidationWarning::InvalidIndex {
                element_id: base.id.clone(),
                value: index.clone(),
            });
        }
    }
}

fn validate_text(text: &TextElement, limits: &ValidationLimits, errors: &mut Vec<ValidationError>) {
    if text.text.len() > limits.max_text_length {
        errors.push(ValidationError::TextTooLong {
            element_id: text.base.id.clone(),
            actual: text.text.len(),
            limit: limits.max_text_length,
        });
    }
}

fn validate_linear(
    linear: &LinearElement,
    limits: &ValidationLimits,
    errors: &mut Vec<ValidationError>,
) {
    if linear.points.len() > limits.max_points_per_element {
        errors.push(ValidationError::TooManyPoints {
            element_id: linear.base.id.clone(),
            actual: linear.points.len(),
            limit: limits.max_points_per_element,
        });
    }
}

fn is_plausible_fractional_index(index: &str) -> bool {
    !index.is_empty()
        && index
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::{validate_str, ValidationError, ValidationLimits, ValidationWarning};

    #[test]
    fn validate_reports_corrupt_json_as_error() -> Result<(), Box<dyn Error>> {
        let report = validate_str("{bad-json", &ValidationLimits::default());
        ensure_eq(&report.valid, false, "valid flag")?;
        ensure_eq(&report.element_count, 0_usize, "element count")?;
        if report
            .errors
            .iter()
            .any(|error| matches!(error, ValidationError::InvalidJson { .. }))
        {
            Ok(())
        } else {
            Err("expected invalid JSON error".into())
        }
    }

    #[test]
    fn validate_reports_hard_limits() -> Result<(), Box<dyn Error>> {
        let limits = ValidationLimits {
            max_payload_size_bytes: 5,
            ..ValidationLimits::default()
        };
        let report = validate_str(r#"{"elements":[]}"#, &limits);
        ensure_eq(&report.valid, false, "payload valid flag")?;
        if report
            .errors
            .iter()
            .any(|error| matches!(error, ValidationError::PayloadTooLarge { .. }))
        {
            Ok(())
        } else {
            Err("expected payload too large error".into())
        }
    }

    #[test]
    fn validate_reports_element_file_text_and_point_limits() -> Result<(), Box<dyn Error>> {
        let limits = ValidationLimits {
            max_elements: 1,
            max_text_length: 3,
            max_points_per_element: 1,
            max_files: 0,
            max_file_size_bytes: 3,
            max_image_data_url_bytes: 3,
            ..ValidationLimits::default()
        };
        let report = validate_str(
            r##"{
                "elements": [
                    {"type":"text","id":"t1","text":"hello"},
                    {"type":"line","id":"l1","points":[[0,0],[1,1]]}
                ],
                "files": {
                    "f1": {"id":"f1","dataURL":"data:image/png;base64,AAAA"}
                }
            }"##,
            &limits,
        );

        ensure_eq(&report.valid, false, "valid flag")?;
        ensure_has_error(&report.errors, "too many elements", |error| {
            matches!(error, ValidationError::TooManyElements { .. })
        })?;
        ensure_has_error(&report.errors, "text too long", |error| {
            matches!(error, ValidationError::TextTooLong { .. })
        })?;
        ensure_has_error(&report.errors, "too many points", |error| {
            matches!(error, ValidationError::TooManyPoints { .. })
        })?;
        ensure_has_error(&report.errors, "too many files", |error| {
            matches!(error, ValidationError::TooManyFiles { .. })
        })?;
        ensure_has_error(&report.errors, "file too large", |error| {
            matches!(error, ValidationError::FileTooLarge { .. })
        })?;
        ensure_has_error(&report.errors, "image data too large", |error| {
            matches!(error, ValidationError::ImageDataUrlTooLarge { .. })
        })
    }

    #[test]
    fn validate_reports_recoverable_warnings() -> Result<(), Box<dyn Error>> {
        let report = validate_str(
            r##"{
                "elements": [
                    {"type":"unknown-shape","id":"u1"},
                    {"type":"embeddable","id":"e1"},
                    {"type":"iframe","id":"i1"},
                    {"type":"image","id":"img1","fileId":"missing"},
                    {
                        "type":"rectangle",
                        "id":"r1",
                        "strokeColor":"not-a-color",
                        "backgroundColor":"also-bad",
                        "index":"bad index"
                    }
                ]
            }"##,
            &ValidationLimits::default(),
        );

        ensure_eq(&report.valid, true, "valid flag")?;
        ensure_has_warning(&report.warnings, "unknown element", |warning| {
            matches!(warning, ValidationWarning::UnknownElementType { .. })
        })?;
        ensure_has_warning(&report.warnings, "unsupported element", |warning| {
            matches!(warning, ValidationWarning::UnsupportedElementType { .. })
        })?;
        ensure_has_warning(&report.warnings, "missing image", |warning| {
            matches!(warning, ValidationWarning::MissingImage { .. })
        })?;
        ensure_has_warning(&report.warnings, "invalid color", |warning| {
            matches!(warning, ValidationWarning::InvalidColor { .. })
        })?;
        ensure_has_warning(&report.warnings, "invalid index", |warning| {
            matches!(warning, ValidationWarning::InvalidIndex { .. })
        })
    }

    fn ensure_has_error(
        errors: &[ValidationError],
        label: &str,
        predicate: impl Fn(&ValidationError) -> bool,
    ) -> Result<(), Box<dyn Error>> {
        if errors.iter().any(predicate) {
            Ok(())
        } else {
            Err(format!("missing validation error: {label}").into())
        }
    }

    fn ensure_has_warning(
        warnings: &[ValidationWarning],
        label: &str,
        predicate: impl Fn(&ValidationWarning) -> bool,
    ) -> Result<(), Box<dyn Error>> {
        if warnings.iter().any(predicate) {
            Ok(())
        } else {
            Err(format!("missing validation warning: {label}").into())
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
