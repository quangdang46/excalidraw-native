//! Tier-2 fallback: emit a single placeholder rectangle + text element when a
//! diagram type is not handled by a dedicated converter and
//! [`crate::options::OnUnsupported::Placeholder`] is selected.

use serde_json::Value;

use crate::builder::{self, Rect, Text};
use crate::error::MermaidConvertError;
use crate::options::{MermaidConvertOptions, OnUnsupported};

const PLACEHOLDER_WIDTH: f64 = 320.0;
const PLACEHOLDER_HEIGHT: f64 = 120.0;

/// Build placeholder element values for an unsupported diagram type, honoring
/// [`MermaidConvertOptions::on_unsupported`].
pub fn build_unsupported_placeholder(
    diagram_type: &str,
    options: &MermaidConvertOptions,
) -> Result<Vec<Value>, MermaidConvertError> {
    match options.on_unsupported {
        OnUnsupported::Error => Err(MermaidConvertError::Unsupported {
            diagram_type: diagram_type.to_string(),
        }),
        OnUnsupported::Placeholder => Ok(build(diagram_type, options)),
    }
}

fn build(diagram_type: &str, options: &MermaidConvertOptions) -> Vec<Value> {
    let id = format!("placeholder-{diagram_type}");
    let text_id = format!("placeholder-{diagram_type}-text");
    let mut rect = builder::rectangle(
        &Rect {
            id: &id,
            x: 0.0,
            y: 0.0,
            width: PLACEHOLDER_WIDTH,
            height: PLACEHOLDER_HEIGHT,
            fill: Some("#fff5f5"),
            rounded: true,
            frame_id: None,
        },
        options,
    );
    builder::bind_text(&mut rect, &text_id);
    let label = format!(
        "Mermaid diagram type \"{diagram_type}\" is not yet supported by excalidraw-mermaid"
    );
    let text = builder::text(
        &Text {
            id: &text_id,
            x: 0.0,
            y: 0.0,
            width: PLACEHOLDER_WIDTH,
            height: PLACEHOLDER_HEIGHT,
            text: &label,
            font_size: options.font_size,
            align: "center",
            container_id: Some(&id),
            frame_id: None,
        },
        options,
    );
    vec![rect, text]
}
