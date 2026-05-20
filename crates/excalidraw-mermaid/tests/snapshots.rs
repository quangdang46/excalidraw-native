//! Visual + structural regression snapshots for every Tier 1 Mermaid fixture.
//!
//! For each `.mmd` fixture under `tests/fixtures/mermaid/` we lock down both
//!
//! 1. The Excalidraw JSON that `excalidraw-mermaid` produces (the canonical
//!    conversion output — covers shape ids, geometry, bindings, frames,
//!    arrowheads, edge labels).
//! 2. The SVG rendered by `excalidraw-render` from that JSON (the visible
//!    output — covers rough geometry, fills, text placement, viewBox).
//!
//! Snapshots use `insta` and live under
//! `crates/excalidraw-mermaid/tests/snapshots/`. Element ids and rough seeds
//! are deterministic (`STABLE_SEED = 1_337`), so a passing snapshot run means
//! the rendered diagram is byte-identical to the committed baseline.
//!
//! To accept intentional changes, run:
//!
//! ```sh
//! cargo insta test -p excalidraw-mermaid --accept
//! ```

use std::path::PathBuf;

use excalidraw_mermaid::{
    parse_to_excalidraw_file, parse_to_excalidraw_value, FlowchartCurve, MermaidConvertOptions,
    OnUnsupported,
};

/// Render configuration used for every snapshot. Kept deterministic on
/// purpose: `FastSvg` skips embedded fonts (which would bloat snapshots with
/// base64), but still emits the full rough geometry that we want to lock
/// down. `BackgroundMode::FromFile` ensures backgrounds come from the
/// converted scene rather than a CLI override.
fn render_options() -> excalidraw_render::RenderOptions {
    excalidraw_render::RenderOptions {
        scale: 1.0,
        padding: 16.0,
        background: excalidraw_render::BackgroundMode::FromFile,
        quality: excalidraw_render::RenderQuality::FastSvg,
        unsupported: excalidraw_render::UnsupportedElementMode::Placeholder,
        image_policy: excalidraw_render::ImagePolicy::Skip,
        text_policy: excalidraw_render::TextPolicy::SvgText,
    }
}

fn convert_options() -> MermaidConvertOptions {
    MermaidConvertOptions {
        // Lock font_size to the README/MCP default so snapshots survive a
        // future change to `MermaidConvertOptions::default()`.
        font_size: 16.0,
        flowchart_curve: FlowchartCurve::Linear,
        max_edges: 5_000,
        max_text_size: 4_000,
        on_unsupported: OnUnsupported::Placeholder,
        hachure_fill: false,
    }
}

fn fixture(name: &str) -> String {
    let path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/mermaid")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read fixture {}: {err}", path.display()))
}

/// Snapshot the conversion output (Excalidraw JSON) and the rendered SVG for
/// `fixture_name`. The snapshot suffix is derived from the fixture so that
/// every `.mmd` produces its own pair of `.snap` files.
fn snapshot_fixture(fixture_name: &str) {
    let src = fixture(fixture_name);
    let opts = convert_options();

    // 1. Conversion snapshot — full Excalidraw JSON in canonical form.
    let json = parse_to_excalidraw_value(&src, &opts)
        .unwrap_or_else(|e| panic!("convert {fixture_name}: {e}"));
    insta::with_settings!({
        snapshot_suffix => fixture_name.trim_end_matches(".mmd"),
        prepend_module_to_snapshot => false,
        description => format!("Mermaid → Excalidraw JSON for {fixture_name}"),
    }, {
        insta::assert_json_snapshot!("excalidraw", json);
    });

    // 2. Render snapshot — SVG produced by the v0.1 renderer.
    let file = parse_to_excalidraw_file(&src, &opts)
        .unwrap_or_else(|e| panic!("convert file {fixture_name}: {e}"));
    let scene = excalidraw_core::normalize_file(&file);
    let svg = excalidraw_render::render_svg(&scene, &render_options())
        .unwrap_or_else(|e| panic!("render {fixture_name}: {e}"));
    insta::with_settings!({
        snapshot_suffix => fixture_name.trim_end_matches(".mmd"),
        prepend_module_to_snapshot => false,
        description => format!("Rendered SVG for {fixture_name}"),
        omit_expression => true,
    }, {
        insta::assert_snapshot!("svg", svg.value);
    });
}

macro_rules! snapshot_test {
    ($name:ident, $fixture:literal) => {
        #[test]
        fn $name() {
            snapshot_fixture($fixture);
        }
    };
}

// Flowcharts (PLAN §21.13)
snapshot_test!(flowchart_basic, "flowchart_basic.mmd");
snapshot_test!(flowchart_subgraph, "flowchart_subgraph.mmd");
snapshot_test!(flowchart_styled, "flowchart_styled.mmd");
snapshot_test!(flowchart_shapes, "flowchart_shapes.mmd");
snapshot_test!(flowchart_50nodes, "flowchart_50nodes.mmd");

// Sequence
snapshot_test!(sequence_basic, "sequence_basic.mmd");
snapshot_test!(sequence_loops, "sequence_loops.mmd");
snapshot_test!(sequence_activations, "sequence_activations.mmd");
snapshot_test!(sequence_notes, "sequence_notes.mmd");

// Class
snapshot_test!(class_basic, "class_basic.mmd");
snapshot_test!(class_inheritance, "class_inheritance.mmd");
snapshot_test!(class_namespaces, "class_namespaces.mmd");

// State
snapshot_test!(state_basic, "state_basic.mmd");
snapshot_test!(state_composite, "state_composite.mmd");
snapshot_test!(state_choice, "state_choice.mmd");

// ER
snapshot_test!(er_basic, "er_basic.mmd");
snapshot_test!(er_cardinalities, "er_cardinalities.mmd");

// Fallback behaviour (PLAN §21.13: "unsupported fallback behavior").
#[test]
fn unsupported_gantt_placeholder_snapshot() {
    let src = fixture("unsupported_gantt.mmd");
    let opts = convert_options();
    let json = parse_to_excalidraw_value(&src, &opts).expect("placeholder fallback");
    insta::with_settings!({
        snapshot_suffix => "unsupported_gantt",
        prepend_module_to_snapshot => false,
        description => "Mermaid → Excalidraw placeholder JSON for unsupported_gantt.mmd",
    }, {
        insta::assert_json_snapshot!("excalidraw", json);
    });
}
