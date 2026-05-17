//! End-to-end CLI benchmarks for process startup and file IO.

use std::path::{Path, PathBuf};
use std::process::Command;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

include!("../../../benches/benchmark_targets.rs");

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures")
        .join(name)
}

fn run_cli_to_svg(input: &Path, output: &Path) {
    let status = Command::new("../../target/release/excd")
        .arg("to-svg")
        .arg(input)
        .arg("--output")
        .arg(output)
        .arg("--warnings")
        .arg("silent")
        .status()
        .expect("spawn excd to-svg");
    assert!(status.success(), "excd to-svg failed: {status}");
    let svg = std::fs::read_to_string(output).expect("read rendered svg");
    assert!(svg.contains("<svg"), "rendered output should be SVG");
    black_box(svg);
}

fn run_cli_info(input: &Path) {
    let output = Command::new("../../target/release/excd")
        .arg("info")
        .arg(input)
        .arg("--format")
        .arg("json")
        .output()
        .expect("spawn excd info");
    assert!(
        output.status.success(),
        "excd info failed: {}",
        output.status
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("element_count"),
        "info output should include element_count"
    );
    black_box(output.stdout);
}

fn bench_cli_svg_cases(c: &mut Criterion) {
    let mut group = c.benchmark_group("cli_e2e");
    group.sample_size(10);

    for (case, file, target_ms, fail_ms) in [
        ("simple_svg_file_io", "simple_shapes.excalidraw", 120, 1200),
        (
            "complex_svg_file_io",
            "complex_diagram.excalidraw",
            160,
            1600,
        ),
        (
            "large_svg_file_io",
            "large_200_elements.excalidraw",
            350,
            3500,
        ),
    ] {
        let input = fixture(file);
        let target = BenchTarget::new("cli_e2e", case, target_ms, fail_ms);
        report_target(target);
        group.bench_with_input(BenchmarkId::new("to-svg", target.id()), &target, |b, _| {
            b.iter(|| {
                let dir = tempfile::tempdir().expect("tempdir");
                let output = dir.path().join("out.svg");
                run_cli_to_svg(black_box(&input), black_box(&output));
            })
        });
    }

    group.finish();
}

fn bench_cli_info_startup(c: &mut Criterion) {
    let mut group = c.benchmark_group("cli_e2e");
    group.sample_size(10);

    let input = fixture("simple_shapes.excalidraw");
    let target = BenchTarget::new("cli_e2e", "info_startup_json", 80, 800);
    report_target(target);
    group.bench_with_input(BenchmarkId::new("info", target.id()), &target, |b, _| {
        b.iter(|| run_cli_info(black_box(&input)))
    });

    group.finish();
}

criterion_group!(cli_e2e, bench_cli_svg_cases, bench_cli_info_startup);
criterion_main!(cli_e2e);
