//! End-to-end Mermaid → Excalidraw → SVG/PNG smoke tests.
//!
//! These exercise the v0.2 gate: every Tier 1 diagram parses, lays out, and
//! converts into elements that round-trip through `excalidraw-core` and
//! render through `excalidraw-render` without errors.

use std::path::PathBuf;

use excalidraw_mermaid::{
    parse_to_excalidraw, parse_to_excalidraw_file, parse_to_excalidraw_value, MermaidConvertOptions,
};

fn fixture(name: &str) -> String {
    let path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/mermaid")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read fixture {}: {err}", path.display()))
}

fn assert_renders(file: &excalidraw_core::ExcalidrawFile) {
    let scene = excalidraw_core::normalize_file(file);
    let opts = excalidraw_render::RenderOptions {
        scale: 1.0,
        padding: 16.0,
        background: excalidraw_render::BackgroundMode::FromFile,
        quality: excalidraw_render::RenderQuality::FastSvg,
        unsupported: excalidraw_render::UnsupportedElementMode::Placeholder,
        image_policy: excalidraw_render::ImagePolicy::Skip,
        text_policy: excalidraw_render::TextPolicy::SvgText,
    };
    let svg = excalidraw_render::render_svg(&scene, &opts).expect("svg render");
    assert!(svg.value.contains("<svg"), "svg render produced output");
}

#[test]
fn flowchart_basic_round_trips() {
    let src = fixture("flowchart_basic.mmd");
    let options = MermaidConvertOptions::default();
    let elements = parse_to_excalidraw(&src, &options).expect("parse flowchart");
    assert!(elements.len() >= 5, "got {} elements", elements.len());
    let file = parse_to_excalidraw_file(&src, &options).expect("parse file");
    assert!(!file.elements.is_empty(), "elements present");
    assert_renders(&file);
}

#[test]
fn flowchart_subgraph_emits_frame() {
    let src = fixture("flowchart_subgraph.mmd");
    let options = MermaidConvertOptions::default();
    let value = parse_to_excalidraw_value(&src, &options).expect("parse subgraph");
    let elements = value
        .get("elements")
        .and_then(|v| v.as_array())
        .expect("elements array");
    let frames: Vec<&serde_json::Value> = elements
        .iter()
        .filter(|e| e.get("type").and_then(|t| t.as_str()) == Some("frame"))
        .collect();
    assert!(
        !frames.is_empty(),
        "subgraph should emit at least one frame"
    );
}

#[test]
fn sequence_basic_round_trips() {
    let src = fixture("sequence_basic.mmd");
    let options = MermaidConvertOptions::default();
    let file = parse_to_excalidraw_file(&src, &options).expect("parse sequence");
    assert!(file.elements.len() >= 2, "actors emitted");
    assert_renders(&file);
}

#[test]
fn state_basic_round_trips() {
    let src = fixture("state_basic.mmd");
    let options = MermaidConvertOptions::default();
    let file = parse_to_excalidraw_file(&src, &options).expect("parse state");
    assert!(!file.elements.is_empty());
    assert_renders(&file);
}

#[test]
fn class_basic_round_trips() {
    let src = fixture("class_basic.mmd");
    let options = MermaidConvertOptions::default();
    let file = parse_to_excalidraw_file(&src, &options).expect("parse class");
    assert!(!file.elements.is_empty());
    assert_renders(&file);
}

#[test]
fn class_basic_has_labels() {
    let src = fixture("class_basic.mmd");
    let options = MermaidConvertOptions::default();
    let value = parse_to_excalidraw_value(&src, &options).expect("class");
    let json = serde_json::to_string(&value).unwrap();
    assert!(json.contains("Animal"), "class name Animal must appear");
    assert!(json.contains("Dog"), "class name Dog must appear");
    assert!(
        json.contains("makeSound"),
        "method makeSound must appear in class text"
    );
}

#[test]
fn sequence_basic_has_labels() {
    let src = fixture("sequence_basic.mmd");
    let options = MermaidConvertOptions::default();
    let value = parse_to_excalidraw_value(&src, &options).expect("seq");
    let json = serde_json::to_string(&value).unwrap();
    assert!(json.contains("Alice"), "actor Alice must appear");
    assert!(json.contains("Bob"), "actor Bob must appear");
    assert!(
        json.contains("Hello Bob"),
        "message 'Hello Bob' must appear"
    );
}

#[test]
fn er_basic_round_trips() {
    let src = fixture("er_basic.mmd");
    let options = MermaidConvertOptions::default();
    let file = parse_to_excalidraw_file(&src, &options).expect("parse er");
    assert!(!file.elements.is_empty());
    assert_renders(&file);
}

#[test]
fn er_basic_has_labels() {
    let src = fixture("er_basic.mmd");
    let options = MermaidConvertOptions::default();
    let value = parse_to_excalidraw_value(&src, &options).expect("er");
    let json = serde_json::to_string(&value).unwrap();
    assert!(
        json.contains("CUSTOMER"),
        "entity CUSTOMER must appear in text"
    );
    assert!(json.contains("ORDER"), "entity ORDER must appear in text");
}

#[test]
fn state_basic_has_pseudo_ellipses() {
    let src = fixture("state_basic.mmd");
    let options = MermaidConvertOptions::default();
    let value = parse_to_excalidraw_value(&src, &options).expect("state");
    let elements = value
        .get("elements")
        .and_then(|v| v.as_array())
        .expect("elements");
    let ellipses: Vec<_> = elements
        .iter()
        .filter(|e| e.get("type").and_then(|t| t.as_str()) == Some("ellipse"))
        .collect();
    assert!(
        ellipses.len() >= 2,
        "start/end pseudo-states should be ellipses, got {}",
        ellipses.len()
    );
}

// --- New fixture tests (Tier 1 coverage) ---

#[test]
fn flowchart_styled_round_trips() {
    let src = fixture("flowchart_styled.mmd");
    let options = MermaidConvertOptions::default();
    let file = parse_to_excalidraw_file(&src, &options).expect("parse styled");
    assert_renders(&file);
}

#[test]
fn flowchart_shapes_round_trips() {
    let src = fixture("flowchart_shapes.mmd");
    let options = MermaidConvertOptions::default();
    let file = parse_to_excalidraw_file(&src, &options).expect("parse shapes");
    let value = parse_to_excalidraw_value(&src, &options).expect("shapes value");
    let json = serde_json::to_string(&value).unwrap();
    assert!(json.contains("Diamond"), "Diamond label present");
    assert!(json.contains("Rectangle"), "Rectangle label present");
    assert!(json.contains("Circle"), "Circle label present");
    assert_renders(&file);
}

#[test]
fn flowchart_50nodes_round_trips() {
    let src = fixture("flowchart_50nodes.mmd");
    let options = MermaidConvertOptions::default();
    let file = parse_to_excalidraw_file(&src, &options).expect("parse 50nodes");
    assert!(
        file.elements.len() >= 40,
        "expected many elements, got {}",
        file.elements.len()
    );
    assert_renders(&file);
}

#[test]
fn sequence_loops_round_trips() {
    let src = fixture("sequence_loops.mmd");
    let options = MermaidConvertOptions::default();
    let file = parse_to_excalidraw_file(&src, &options).expect("parse loops");
    assert_renders(&file);
}

#[test]
fn sequence_activations_round_trips() {
    let src = fixture("sequence_activations.mmd");
    let options = MermaidConvertOptions::default();
    let file = parse_to_excalidraw_file(&src, &options).expect("parse activations");
    assert_renders(&file);
}

#[test]
fn sequence_notes_round_trips() {
    let src = fixture("sequence_notes.mmd");
    let options = MermaidConvertOptions::default();
    let file = parse_to_excalidraw_file(&src, &options).expect("parse notes");
    assert_renders(&file);
}

#[test]
fn class_inheritance_round_trips() {
    let src = fixture("class_inheritance.mmd");
    let options = MermaidConvertOptions::default();
    let file = parse_to_excalidraw_file(&src, &options).expect("parse inheritance");
    let value = parse_to_excalidraw_value(&src, &options).expect("inheritance value");
    let json = serde_json::to_string(&value).unwrap();
    assert!(json.contains("Shape"), "Shape class present");
    assert!(json.contains("Circle"), "Circle class present");
    assert!(json.contains("Square"), "Square class present");
    assert_renders(&file);
}

#[test]
fn class_namespaces_round_trips() {
    let src = fixture("class_namespaces.mmd");
    let options = MermaidConvertOptions::default();
    let file = parse_to_excalidraw_file(&src, &options).expect("parse namespaces");
    assert_renders(&file);
}

#[test]
fn state_composite_round_trips() {
    let src = fixture("state_composite.mmd");
    let options = MermaidConvertOptions::default();
    let file = parse_to_excalidraw_file(&src, &options).expect("parse composite");
    assert_renders(&file);
}

#[test]
fn state_choice_round_trips() {
    let src = fixture("state_choice.mmd");
    let options = MermaidConvertOptions::default();
    let file = parse_to_excalidraw_file(&src, &options).expect("parse choice");
    assert_renders(&file);
}

#[test]
fn er_cardinalities_round_trips() {
    let src = fixture("er_cardinalities.mmd");
    let options = MermaidConvertOptions::default();
    let file = parse_to_excalidraw_file(&src, &options).expect("parse cardinalities");
    assert_renders(&file);
}

#[test]
fn unsupported_gantt_falls_back() {
    let src = fixture("unsupported_gantt.mmd");
    let options = MermaidConvertOptions::default();
    let value = parse_to_excalidraw_value(&src, &options).expect("gantt fallback");
    let elements = value
        .get("elements")
        .and_then(|v| v.as_array())
        .expect("elements");
    assert!(!elements.is_empty(), "gantt should produce placeholder");
}

#[test]
fn unsupported_diagram_falls_back_to_placeholder() {
    let src = "pie title pets\n    \"Dogs\": 386\n    \"Cats\": 85\n";
    let options = MermaidConvertOptions::default();
    let value = parse_to_excalidraw_value(src, &options).expect("placeholder fallback");
    let elements = value
        .get("elements")
        .and_then(|v| v.as_array())
        .expect("elements array");
    assert!(
        !elements.is_empty(),
        "placeholder should produce at least one element"
    );
}

#[test]
fn unsupported_diagram_can_error() {
    let src = "pie title pets\n    \"Dogs\": 386\n    \"Cats\": 85\n";
    let options = MermaidConvertOptions {
        on_unsupported: excalidraw_mermaid::OnUnsupported::Error,
        ..MermaidConvertOptions::default()
    };
    let err = parse_to_excalidraw(src, &options).err();
    assert!(err.is_some(), "should fail with on_unsupported=error");
}
