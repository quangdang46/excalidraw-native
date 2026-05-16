//! CLI integration tests for the excd binary.

use std::fs;
use std::path::Path;
use std::path::PathBuf;

use assert_cmd::Command;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures")
        .join(name)
}

fn excd() -> Command {
    Command::cargo_bin("excd").unwrap()
}

#[test]
fn to_svg_stdout() {
    let output = excd()
        .arg("to-svg")
        .arg(fixture("simple_shapes.excalidraw"))
        .output()
        .unwrap();
    assert!(output.status.success(), "exit code");
    let svg = String::from_utf8_lossy(&output.stdout);
    assert!(svg.contains("<svg"), "svg root in stdout");
    assert!(svg.contains("<rect"), "contains rect");
}

#[test]
fn to_svg_file_output() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.svg");
    excd()
        .arg("to-svg")
        .arg(fixture("simple_shapes.excalidraw"))
        .arg("-o")
        .arg(&out)
        .assert()
        .success();
    let content = fs::read_to_string(&out).unwrap();
    assert!(content.contains("<svg"), "svg root in file");
}

#[test]
fn to_svg_clean_quality() {
    let output = excd()
        .arg("to-svg")
        .arg(fixture("simple_shapes.excalidraw"))
        .arg("--quality")
        .arg("clean")
        .output()
        .unwrap();
    assert!(output.status.success());
    let svg = String::from_utf8_lossy(&output.stdout);
    assert!(svg.contains("<rect"), "clean rect");
    assert!(svg.contains("<ellipse"), "clean ellipse");
}

#[test]
fn to_svg_transparent_background() {
    let output = excd()
        .arg("to-svg")
        .arg(fixture("simple_shapes.excalidraw"))
        .arg("--background")
        .arg("transparent")
        .output()
        .unwrap();
    assert!(output.status.success());
    let svg = String::from_utf8_lossy(&output.stdout);
    assert!(!svg.contains("fill=\"#ffffff\""), "no white background");
}

#[test]
fn to_png_file_output() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.png");
    excd()
        .arg("to-png")
        .arg(fixture("simple_shapes.excalidraw"))
        .arg("-o")
        .arg(&out)
        .assert()
        .success();
    let data = fs::read(&out).unwrap();
    assert!(data.starts_with(b"\x89PNG"), "PNG magic bytes");
}

#[test]
fn to_png_scale_affects_size() {
    let dir = tempfile::tempdir().unwrap();

    let small = dir.path().join("small.png");
    excd()
        .arg("to-png")
        .arg(fixture("simple_shapes.excalidraw"))
        .arg("-o")
        .arg(&small)
        .arg("--scale")
        .arg("1")
        .assert()
        .success();

    let large = dir.path().join("large.png");
    excd()
        .arg("to-png")
        .arg(fixture("simple_shapes.excalidraw"))
        .arg("-o")
        .arg(&large)
        .arg("--scale")
        .arg("2")
        .assert()
        .success();

    let small_size = fs::metadata(&small).unwrap().len();
    let large_size = fs::metadata(&large).unwrap().len();
    assert!(large_size > small_size, "scaled PNG should be larger");
}

#[test]
fn convert_to_svg() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("output.svg");
    excd()
        .arg("convert")
        .arg(fixture("simple_shapes.excalidraw"))
        .arg(&out)
        .assert()
        .success();
    let content = fs::read_to_string(&out).unwrap();
    assert!(content.contains("<svg"));
}

#[test]
fn convert_to_png() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("output.png");
    excd()
        .arg("convert")
        .arg(fixture("simple_shapes.excalidraw"))
        .arg(&out)
        .assert()
        .success();
    let data = fs::read(&out).unwrap();
    assert!(data.starts_with(b"\x89PNG"));
}

#[test]
fn convert_unsupported_extension_fails() {
    excd()
        .arg("convert")
        .arg(fixture("simple_shapes.excalidraw"))
        .arg("/tmp/output.bmp")
        .assert()
        .failure();
}

#[test]
fn validate_valid_file() {
    excd()
        .arg("validate")
        .arg(fixture("simple_shapes.excalidraw"))
        .assert()
        .stdout(predicates::str::contains("valid"))
        .success();
}

#[test]
fn validate_json_output() {
    let output = excd()
        .arg("validate")
        .arg(fixture("simple_shapes.excalidraw"))
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("valid"),
        "validate json should contain 'valid', got: {stdout}"
    );
}

#[test]
fn validate_malformed_input() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.excalidraw");
    fs::write(&bad, "not json at all").unwrap();
    excd()
        .arg("validate")
        .arg(&bad)
        .assert()
        .stdout(predicates::str::contains("invalid"))
        .failure();
}

#[test]
fn info_text_output() {
    let output = excd()
        .arg("info")
        .arg(fixture("complex_diagram.excalidraw"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Elements: 8"), "element count");
    assert!(stdout.contains("rectangle"), "type breakdown");
}

#[test]
fn info_json_output() {
    let output = excd()
        .arg("info")
        .arg(fixture("simple_shapes.excalidraw"))
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("element_count"),
        "info json should contain element_count, got: {stdout}"
    );
}

#[test]
fn describe_lists_elements() {
    let output = excd()
        .arg("describe")
        .arg(fixture("simple_shapes.excalidraw"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rect1"), "element id");
    assert!(stdout.contains("rectangle"), "element type");
}

#[test]
fn warnings_text_mode() {
    let output = excd()
        .arg("to-svg")
        .arg(fixture("unsupported.excalidraw"))
        .arg("--warnings")
        .arg("text")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning:"), "warnings in stderr");
}

#[test]
fn warnings_silent_mode() {
    let output = excd()
        .arg("to-svg")
        .arg(fixture("unsupported.excalidraw"))
        .arg("--warnings")
        .arg("silent")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("warning:"), "no warnings in silent mode");
}

#[test]
fn help_lists_all_commands() {
    excd()
        .arg("--help")
        .assert()
        .stdout(predicates::str::contains("to-svg"))
        .stdout(predicates::str::contains("to-png"))
        .stdout(predicates::str::contains("convert"))
        .stdout(predicates::str::contains("info"))
        .stdout(predicates::str::contains("describe"))
        .stdout(predicates::str::contains("validate"))
        .success();
}

#[test]
fn version_flag() {
    excd()
        .arg("--version")
        .assert()
        .stdout(predicates::str::contains("excd"))
        .success();
}
