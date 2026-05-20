//! Visual regression snapshots for every v0.1 `.excalidraw` fixture.
//!
//! These tests lock down the SVG output of the renderer for each of the
//! hand-crafted fixtures under `tests/fixtures/`. They complement the
//! Mermaid snapshot suite (in `crates/excalidraw-mermaid/tests/snapshots.rs`)
//! by exercising the renderer end-to-end through the *user-authored*
//! Excalidraw JSON format rather than through the Mermaid converter.
//!
//! Stable inputs:
//!
//! * Every fixture has explicit element ids and `seed` values, so the rough
//!   geometry is deterministic.
//! * `RenderQuality::FastSvg` skips embedded fonts (avoids base64 blobs in
//!   snapshots) while still emitting the full rough output.
//! * `ImagePolicy::Skip` prevents the image fixture from inlining a data
//!   URL — instead it appears as a placeholder rect, which is what we want
//!   to snapshot.
//!
//! Snapshots live under `crates/excalidraw-render/tests/snapshots/`. To
//! accept intentional changes (after reviewing the diff!):
//!
//! ```sh
//! cargo insta test -p excalidraw-render --accept
//! ```

use std::path::{Path, PathBuf};

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures")
        .join(name)
}

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

fn render_fixture(name: &str) -> String {
    let path = fixture_path(name);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read fixture {}: {err}", path.display()));
    let file = excalidraw_core::parse_str(&raw)
        .unwrap_or_else(|err| panic!("parse fixture {name}: {err}"));
    let scene = excalidraw_core::normalize_file(&file);
    let output = excalidraw_render::render_svg(&scene, &render_options())
        .unwrap_or_else(|err| panic!("render fixture {name}: {err}"));
    output.value
}

fn snapshot_fixture(name: &str) {
    let svg = render_fixture(name);
    insta::with_settings!({
        snapshot_suffix => name.trim_end_matches(".excalidraw"),
        prepend_module_to_snapshot => false,
        description => format!("Rendered SVG for {name}"),
        omit_expression => true,
    }, {
        insta::assert_snapshot!("svg", svg);
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

// Shapes / text fixtures
snapshot_test!(simple_shapes, "simple_shapes.excalidraw");
snapshot_test!(text_standalone, "text_standalone.excalidraw");
snapshot_test!(text_containers, "text_containers.excalidraw");

// Linear / binding fixtures
snapshot_test!(arrows_basic, "arrows_basic.excalidraw");
snapshot_test!(arrows_bound, "arrows_bound.excalidraw");

// Freedraw — covers rough-rs `curve` path emission.
snapshot_test!(freedraw, "freedraw.excalidraw");

// Frame + child clipPath default-on behaviour.
snapshot_test!(frame_clip, "frame_clip.excalidraw");

// Image (placeholder, since ImagePolicy::Skip drops embed but the
// element still emits an outline box) — locks placeholder behaviour.
snapshot_test!(image_embed, "image_embed.excalidraw");

// Unsupported element placeholder behaviour.
snapshot_test!(unsupported, "unsupported.excalidraw");

// Larger / mixed scenes.
snapshot_test!(complex_diagram, "complex_diagram.excalidraw");
snapshot_test!(flowchart_complex, "flowchart_complex.excalidraw");
snapshot_test!(architecture_diagram, "architecture_diagram.excalidraw");

/// The 200-element fixture is huge (~150 KB rendered SVG). We still want
/// regression coverage so we hash the SVG instead of storing the entire
/// payload — any change to a single element flips the digest.
#[test]
fn large_200_elements_digest() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let svg = render_fixture("large_200_elements.excalidraw");
    let mut hasher = DefaultHasher::new();
    svg.hash(&mut hasher);
    let digest = format!("{:016x} len={}", hasher.finish(), svg.len());
    insta::with_settings!({
        prepend_module_to_snapshot => false,
        description => "DefaultHasher digest + byte length for large_200_elements.excalidraw",
        omit_expression => true,
    }, {
        insta::assert_snapshot!("large_200_elements_digest", digest);
    });
}
