//! SVG validity and visual regression integration tests.

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

fn render_fixture_svg(
    name: &str,
    opts: &excalidraw_render::RenderOptions,
) -> excalidraw_render::RenderOutput<String> {
    let raw = std::fs::read_to_string(fixture_path(name)).unwrap();
    let file = excalidraw_core::parse_str(&raw).unwrap();
    let scene = excalidraw_core::normalize_file(&file);
    excalidraw_render::render_svg(&scene, opts).unwrap()
}

fn validate_svg(svg: &str, label: &str) {
    assert!(svg.starts_with("<?xml"), "{label}: missing XML declaration");
    assert!(svg.contains("<svg"), "{label}: missing <svg> root");
    usvg::Tree::from_str(svg, &usvg::Options::default())
        .unwrap_or_else(|e| panic!("{label}: invalid SVG via usvg: {e}"));
}

fn assert_contains(svg: &str, needle: &str, label: &str) {
    assert!(svg.contains(needle), "{label}: expected '{needle}'");
}

#[test]
fn svg_validity_simple_shapes() {
    let output = render_fixture_svg("simple_shapes.excalidraw", &Default::default());
    validate_svg(&output.value, "simple_shapes");
    // Full quality renders all shapes via rough-rs as <path> elements.
    // Just verify multiple elements are present.
    let path_count = output.value.matches("<path").count();
    assert!(
        path_count >= 3,
        "simple_shapes: expected at least 3 paths, got {path_count}"
    );
}

#[test]
fn svg_validity_text_standalone() {
    let output = render_fixture_svg("text_standalone.excalidraw", &Default::default());
    validate_svg(&output.value, "text_standalone");
    assert_contains(&output.value, "<text", "text_standalone");
}

#[test]
fn svg_validity_text_containers() {
    let output = render_fixture_svg("text_containers.excalidraw", &Default::default());
    validate_svg(&output.value, "text_containers");
    assert_contains(&output.value, "<rect", "text_containers");
    assert_contains(&output.value, "<text", "text_containers");
}

#[test]
fn svg_validity_arrows_basic() {
    let output = render_fixture_svg("arrows_basic.excalidraw", &Default::default());
    validate_svg(&output.value, "arrows_basic");
    assert_contains(&output.value, "<path", "arrows_basic");
}

#[test]
fn svg_validity_arrows_bound() {
    let output = render_fixture_svg("arrows_bound.excalidraw", &Default::default());
    validate_svg(&output.value, "arrows_bound");
    assert_contains(&output.value, "label", "arrows_bound");
    // Bindings must be reflected in the SVG so consumers can identify which
    // shapes an arrow connects to (and so endpoints visually attach to those
    // shapes' edges instead of their original raw points).
    assert_contains(&output.value, r#"id="conn""#, "arrows_bound id");
    assert_contains(
        &output.value,
        r#"data-start-binding="src""#,
        "arrows_bound start binding",
    );
    assert_contains(
        &output.value,
        r#"data-end-binding="dst""#,
        "arrows_bound end binding",
    );
    // The arrow's raw points are (90,55)->(150,55) in scene coordinates; with
    // `src` (x=10,w=80) and `dst` (x=150,w=80) plus 1px gap, the rendered
    // endpoints should sit at x=91 and x=149 (the unique line-only path).
    assert_contains(
        &output.value,
        r#"<path d="M91 55 L149 55""#,
        "arrows_bound attached endpoints",
    );
}

#[test]
fn svg_validity_freedraw() {
    let output = render_fixture_svg("freedraw.excalidraw", &Default::default());
    validate_svg(&output.value, "freedraw");
    assert_contains(&output.value, "round", "freedraw");
}

#[test]
fn svg_validity_image_embed() {
    let output = render_fixture_svg("image_embed.excalidraw", &Default::default());
    validate_svg(&output.value, "image_embed");
    assert_contains(&output.value, "<image", "image_embed");
    assert_contains(&output.value, "no image", "image_embed");
}

#[test]
fn svg_validity_frame_clip() {
    let output = render_fixture_svg("frame_clip.excalidraw", &Default::default());
    validate_svg(&output.value, "frame_clip");
    assert_contains(&output.value, "My Frame", "frame_clip");
}

#[test]
fn svg_validity_unsupported() {
    let output = render_fixture_svg("unsupported.excalidraw", &Default::default());
    validate_svg(&output.value, "unsupported");
    assert_contains(&output.value, "unsupported", "unsupported");
    assert_contains(&output.value, "customWidget", "unsupported");
}

#[test]
fn svg_validity_complex_diagram() {
    let output = render_fixture_svg("complex_diagram.excalidraw", &Default::default());
    validate_svg(&output.value, "complex_diagram");
}

#[test]
fn svg_validity_large_200() {
    let output = render_fixture_svg("large_200_elements.excalidraw", &Default::default());
    validate_svg(&output.value, "large_200");
}

#[test]
fn png_rasterization_all_fixtures() {
    let fixtures = [
        "simple_shapes.excalidraw",
        "text_standalone.excalidraw",
        "freedraw.excalidraw",
        "frame_clip.excalidraw",
        "complex_diagram.excalidraw",
    ];
    for name in &fixtures {
        let raw = std::fs::read_to_string(fixture_path(name)).unwrap();
        let file = excalidraw_core::parse_str(&raw).unwrap();
        let scene = excalidraw_core::normalize_file(&file);
        let output = excalidraw_render::render_png(
            &scene,
            &excalidraw_render::RenderOptions {
                scale: 1.0,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(output.value.starts_with(b"\x89PNG"), "{name}: PNG magic");
        assert!(output.value.len() > 100, "{name}: PNG too small");
    }
}

#[test]
fn warnings_for_unsupported_and_missing_images() {
    let output = render_fixture_svg("unsupported.excalidraw", &Default::default());
    assert!(
        !output.warnings.is_empty(),
        "unsupported should have warnings"
    );

    let output = render_fixture_svg("image_embed.excalidraw", &Default::default());
    assert!(
        output
            .warnings
            .iter()
            .any(|w| matches!(w, excalidraw_render::RenderWarning::MissingImageData { .. })),
        "image_embed should warn about missing image"
    );
}
