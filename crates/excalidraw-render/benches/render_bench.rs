//! Benchmarks for SVG and PNG rendering paths.

use std::path::Path;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use excalidraw_core::{normalize_file, parse_str};
use excalidraw_render::{render_png, render_svg, RenderOptions};

include!("../../../benches/benchmark_targets.rs");

fn fixture_path(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures")
        .join(name)
}

fn bench_svg_simple(c: &mut Criterion) {
    let raw = std::fs::read_to_string(fixture_path("simple_shapes.excalidraw")).unwrap();
    let file = parse_str(&raw).unwrap();
    let scene = normalize_file(&file);
    let target = BenchTarget::new("render_svg_lib", "simple_3elem", 8, 80);
    report_target(target);
    c.bench_with_input(BenchmarkId::new("svg", target.id()), &target, |b, _| {
        b.iter(|| render_svg(black_box(&scene), &RenderOptions::default()))
    });
}

fn bench_svg_complex(c: &mut Criterion) {
    let raw = std::fs::read_to_string(fixture_path("complex_diagram.excalidraw")).unwrap();
    let file = parse_str(&raw).unwrap();
    let scene = normalize_file(&file);
    let target = BenchTarget::new("render_svg_lib", "complex_8elem", 15, 150);
    report_target(target);
    c.bench_with_input(BenchmarkId::new("svg", target.id()), &target, |b, _| {
        b.iter(|| render_svg(black_box(&scene), &RenderOptions::default()))
    });
}

fn bench_svg_large(c: &mut Criterion) {
    let raw = std::fs::read_to_string(fixture_path("large_200_elements.excalidraw")).unwrap();
    let file = parse_str(&raw).unwrap();
    let scene = normalize_file(&file);
    let target = BenchTarget::new("render_svg_lib", "large_200elem", 150, 1500);
    report_target(target);
    c.bench_with_input(BenchmarkId::new("svg", target.id()), &target, |b, _| {
        b.iter(|| render_svg(black_box(&scene), &RenderOptions::default()))
    });
}

fn bench_svg_clean(c: &mut Criterion) {
    let raw = std::fs::read_to_string(fixture_path("simple_shapes.excalidraw")).unwrap();
    let file = parse_str(&raw).unwrap();
    let scene = normalize_file(&file);
    let opts = RenderOptions {
        quality: excalidraw_render::RenderQuality::Clean,
        ..Default::default()
    };
    let target = BenchTarget::new("render_svg_lib", "clean_simple", 3, 30);
    report_target(target);
    c.bench_with_input(BenchmarkId::new("svg", target.id()), &target, |b, _| {
        b.iter(|| render_svg(black_box(&scene), black_box(&opts)))
    });
}

fn bench_png_simple(c: &mut Criterion) {
    let raw = std::fs::read_to_string(fixture_path("simple_shapes.excalidraw")).unwrap();
    let file = parse_str(&raw).unwrap();
    let scene = normalize_file(&file);
    let target = BenchTarget::new("render_png_lib", "simple_3elem", 25, 250);
    report_target(target);
    c.bench_with_input(BenchmarkId::new("png", target.id()), &target, |b, _| {
        b.iter(|| render_png(black_box(&scene), &RenderOptions::default()))
    });
}

fn bench_png_large(c: &mut Criterion) {
    let raw = std::fs::read_to_string(fixture_path("large_200_elements.excalidraw")).unwrap();
    let file = parse_str(&raw).unwrap();
    let scene = normalize_file(&file);
    let target = BenchTarget::new("render_png_lib", "large_200elem", 300, 3000);
    report_target(target);
    c.bench_with_input(BenchmarkId::new("png", target.id()), &target, |b, _| {
        b.iter(|| {
            render_png(
                black_box(&scene),
                &RenderOptions {
                    scale: 1.0,
                    ..Default::default()
                },
            )
        })
    });
}

fn bench_full_pipeline(c: &mut Criterion) {
    let raw = std::fs::read_to_string(fixture_path("complex_diagram.excalidraw")).unwrap();
    let target = BenchTarget::new("render_svg_lib", "parse_normalize_complex_svg", 25, 250);
    report_target(target);
    c.bench_with_input(
        BenchmarkId::new("pipeline", target.id()),
        &target,
        |b, _| {
            b.iter(|| {
                let file = parse_str(black_box(&raw)).unwrap();
                let scene = normalize_file(&file);
                render_svg(black_box(&scene), &RenderOptions::default())
            })
        },
    );
}

criterion_group!(
    render_svg_lib,
    bench_svg_simple,
    bench_svg_complex,
    bench_svg_large,
    bench_svg_clean,
    bench_full_pipeline,
);
criterion_group!(render_png_lib, bench_png_simple, bench_png_large,);
criterion_main!(render_svg_lib, render_png_lib);
