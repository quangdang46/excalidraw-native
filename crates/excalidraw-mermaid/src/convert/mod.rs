//! Diagram-type dispatcher for Mermaid → Excalidraw conversion.

use merman_render::model::{LayoutDiagram, LayoutedDiagram};
use serde_json::Value;

use crate::builder;
use crate::error::MermaidConvertError;
use crate::fallback;
use crate::options::MermaidConvertOptions;

mod class;
mod common;
mod er;
mod flowchart;
mod sequence;
mod state;

/// Convert a fully-laid-out diagram into a flat list of Excalidraw element
/// JSON values. The list preserves the conversion order so callers can build
/// a `.excalidraw` document or append the elements into a larger scene.
pub fn convert_layouted(
    layouted: &LayoutedDiagram,
    options: &MermaidConvertOptions,
) -> Result<Vec<Value>, MermaidConvertError> {
    let elements = match &layouted.layout {
        LayoutDiagram::FlowchartV2(layout) => {
            flowchart::convert(layout, &layouted.semantic, options)?
        }
        LayoutDiagram::SequenceDiagram(layout) => {
            sequence::convert(layout, &layouted.semantic, options)?
        }
        LayoutDiagram::ClassDiagramV2(layout) => {
            class::convert(layout, &layouted.semantic, options)?
        }
        LayoutDiagram::StateDiagramV2(layout) => {
            state::convert(layout, &layouted.semantic, options)?
        }
        LayoutDiagram::ErDiagram(layout) => er::convert(layout, &layouted.semantic, options)?,
        _ => fallback::build_unsupported_placeholder(&layouted.meta.diagram_type, options)?,
    };
    enforce_limits(&elements, options)?;
    Ok(elements)
}

/// Convert and wrap into a `.excalidraw` JSON document.
pub fn convert_layouted_to_file(
    layouted: &LayoutedDiagram,
    options: &MermaidConvertOptions,
) -> Result<Value, MermaidConvertError> {
    let elements = convert_layouted(layouted, options)?;
    Ok(builder::build_document(
        elements,
        &layouted.meta.diagram_type,
    ))
}

fn enforce_limits(
    elements: &[Value],
    options: &MermaidConvertOptions,
) -> Result<(), MermaidConvertError> {
    let mut edges = 0_usize;
    for element in elements {
        if element.get("type").and_then(Value::as_str) == Some("arrow") {
            edges += 1;
            if edges > options.max_edges {
                return Err(MermaidConvertError::LimitExceeded {
                    message: format!("more than {} edges produced", options.max_edges),
                });
            }
        }
    }
    Ok(())
}
