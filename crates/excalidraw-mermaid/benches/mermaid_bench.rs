//! Comprehensive benchmarks for the Mermaid → Excalidraw pipeline.
//!
//! Modeled after mermaid-rs-renderer's benchmark structure with separate groups
//! for each pipeline stage: parse/layout, convert, render, and end-to-end.
//!
//! By default (no env var), runs a fast smoke validation of all fixtures.
//! Set `EXCD_RUN_CRITERION_BENCHES=1` to run full Criterion measurements.

use criterion::{black_box, criterion_group, BenchmarkId, Criterion};
use excalidraw_mermaid::engine::layout_mermaid;
use excalidraw_mermaid::options::MermaidConvertOptions;
use excalidraw_mermaid::{parse_to_excalidraw, parse_to_excalidraw_value};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const BENCH_FIXTURES: &[&str] = &[
    "flowchart_tiny",
    "flowchart_small",
    "flowchart_medium",
    "flowchart_large",
    "sequence_tiny",
    "sequence_medium",
    "class_tiny",
    "class_medium",
    "state_tiny",
    "state_medium",
];

fn fixture(name: &str) -> &'static str {
    match name {
        "flowchart_tiny" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/benches/fixtures/flowchart_tiny.mmd"
        )),
        "flowchart_small" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/benches/fixtures/flowchart_small.mmd"
        )),
        "flowchart_medium" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/benches/fixtures/flowchart_medium.mmd"
        )),
        "flowchart_large" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/benches/fixtures/flowchart_large.mmd"
        )),
        "sequence_tiny" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/benches/fixtures/sequence_tiny.mmd"
        )),
        "sequence_medium" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/benches/fixtures/sequence_medium.mmd"
        )),
        "class_tiny" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/benches/fixtures/class_tiny.mmd"
        )),
        "class_medium" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/benches/fixtures/class_medium.mmd"
        )),
        "state_tiny" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/benches/fixtures/state_tiny.mmd"
        )),
        "state_medium" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/benches/fixtures/state_medium.mmd"
        )),
        _ => panic!("unknown fixture: {name}"),
    }
}

// ---------------------------------------------------------------------------
// Programmatic fixture generators (for scalability benchmarks)
// ---------------------------------------------------------------------------

fn dense_flowchart_source(nodes: usize, extra_edges: usize) -> String {
    let mut out = String::from("flowchart LR\n");
    if nodes == 0 {
        return out;
    }
    for i in 0..nodes {
        out.push_str(&format!("  N{}[Node {}]\n", i, i));
    }
    for i in 0..nodes.saturating_sub(1) {
        out.push_str(&format!("  N{} --> N{}\n", i, i + 1));
    }
    let mut count = 0usize;
    for i in 0..nodes {
        for j in (i + 2)..nodes {
            if count >= extra_edges {
                break;
            }
            out.push_str(&format!("  N{} --> N{}\n", i, j));
            count += 1;
        }
        if count >= extra_edges {
            break;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Benchmark: Mermaid parse + layout (merman-core/merman-render)
// ---------------------------------------------------------------------------

fn bench_mermaid_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("mermaid_layout");
    let opts = MermaidConvertOptions::default();
    for name in BENCH_FIXTURES {
        let input = fixture(name);
        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let layouted = layout_mermaid(black_box(data), &opts).expect("layout failed");
                black_box(&layouted);
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Convert layouted diagram → Excalidraw elements
// ---------------------------------------------------------------------------

fn bench_mermaid_convert(c: &mut Criterion) {
    let mut group = c.benchmark_group("mermaid_convert");
    let opts = MermaidConvertOptions::default();
    for name in BENCH_FIXTURES {
        let input = fixture(name);
        let layouted = layout_mermaid(input, &opts).expect("layout failed");
        group.bench_with_input(BenchmarkId::from_parameter(name), &layouted, |b, data| {
            b.iter(|| {
                let file =
                    excalidraw_mermaid::convert::convert_layouted_to_file(black_box(data), &opts)
                        .expect("convert failed");
                black_box(file);
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: End-to-end (parse → layout → convert → Excalidraw elements)
// ---------------------------------------------------------------------------

fn bench_mermaid_end_to_end(c: &mut Criterion) {
    let mut group = c.benchmark_group("mermaid_end_to_end");
    let opts = MermaidConvertOptions::default();
    for name in BENCH_FIXTURES {
        let input = fixture(name);
        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let elements =
                    parse_to_excalidraw(black_box(data), &opts).expect("end-to-end failed");
                black_box(elements.len());
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: End-to-end → JSON value (includes serialization)
// ---------------------------------------------------------------------------

fn bench_mermaid_to_json(c: &mut Criterion) {
    let mut group = c.benchmark_group("mermaid_to_json");
    let opts = MermaidConvertOptions::default();
    for name in BENCH_FIXTURES {
        let input = fixture(name);
        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let value =
                    parse_to_excalidraw_value(black_box(data), &opts).expect("to_json failed");
                black_box(&value);
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Dense graph scalability (edge routing stress)
// ---------------------------------------------------------------------------

fn bench_mermaid_dense_graphs(c: &mut Criterion) {
    let mut group = c.benchmark_group("mermaid_dense_graphs");
    let opts = MermaidConvertOptions::default();
    for (nodes, extra_edges) in [(10usize, 10usize), (20, 30), (30, 60)] {
        let name = format!("dense_{}n_{}e", nodes, extra_edges);
        let input = dense_flowchart_source(nodes, extra_edges);
        group.bench_with_input(BenchmarkId::from_parameter(&name), &input, |b, data| {
            b.iter(|| {
                let elements =
                    parse_to_excalidraw(black_box(data), &opts).expect("dense graph failed");
                black_box(elements.len());
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: SVG render from Mermaid (full pipeline including excalidraw-render)
// ---------------------------------------------------------------------------

fn bench_mermaid_render_svg(c: &mut Criterion) {
    let mut group = c.benchmark_group("mermaid_render_svg");
    let opts = MermaidConvertOptions::default();
    let render_opts = excalidraw_render::RenderOptions::default();
    for name in [
        "flowchart_tiny",
        "flowchart_small",
        "flowchart_medium",
        "sequence_tiny",
        "class_tiny",
        "state_tiny",
    ] {
        let input = fixture(name);
        let file =
            excalidraw_mermaid::parse_to_excalidraw_file(input, &opts).expect("parse failed");
        let scene = excalidraw_core::normalize_file(&file);
        group.bench_with_input(BenchmarkId::from_parameter(name), &scene, |b, data| {
            b.iter(|| {
                let result = excalidraw_render::render_svg(black_box(data), &render_opts)
                    .expect("render failed");
                black_box(result.value.len());
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Smoke validation (default when not running Criterion)
// ---------------------------------------------------------------------------

fn smoke_validate_bench_inputs() {
    let opts = MermaidConvertOptions::default();

    eprintln!("Validating all {} bench fixtures...", BENCH_FIXTURES.len());
    for name in BENCH_FIXTURES {
        let input = fixture(name);
        let elements = parse_to_excalidraw(input, &opts)
            .unwrap_or_else(|e| panic!("{name}: conversion failed: {e}"));
        assert!(
            !elements.is_empty(),
            "{name}: expected at least one element"
        );
        eprintln!("  {name}: {} elements OK", elements.len());
    }

    eprintln!("Validating dense graph generators...");
    for (nodes, extra_edges) in [(10usize, 10usize), (20, 30), (30, 60)] {
        let input = dense_flowchart_source(nodes, extra_edges);
        let elements = parse_to_excalidraw(&input, &opts)
            .unwrap_or_else(|e| panic!("dense_{nodes}_{extra_edges}: failed: {e}"));
        assert!(
            !elements.is_empty(),
            "dense_{nodes}_{extra_edges}: expected elements"
        );
        eprintln!(
            "  dense_{nodes}n_{extra_edges}e: {} elements OK",
            elements.len()
        );
    }

    eprintln!("Validating SVG render pipeline...");
    for name in ["flowchart_tiny", "flowchart_small", "class_tiny"] {
        let input = fixture(name);
        let file = excalidraw_mermaid::parse_to_excalidraw_file(input, &opts)
            .unwrap_or_else(|e| panic!("{name}: file conversion failed: {e}"));
        let scene = excalidraw_core::normalize_file(&file);
        let render_opts = excalidraw_render::RenderOptions::default();
        let result = excalidraw_render::render_svg(&scene, &render_opts)
            .unwrap_or_else(|e| panic!("{name}: SVG render failed: {e}"));
        assert!(result.value.contains("<svg"), "{name}: expected SVG output");
        eprintln!("  {name}: SVG {} bytes OK", result.value.len());
    }

    eprintln!("All smoke validations passed!");
}

// ---------------------------------------------------------------------------
// Criterion groups & main
// ---------------------------------------------------------------------------

criterion_group!(
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(200))
        .measurement_time(Duration::from_millis(500));
    targets = bench_mermaid_layout,
              bench_mermaid_convert,
              bench_mermaid_end_to_end,
              bench_mermaid_to_json,
              bench_mermaid_dense_graphs,
              bench_mermaid_render_svg
);

fn main() {
    if std::env::var_os("EXCD_RUN_CRITERION_BENCHES").is_some() {
        benches();
        Criterion::default().configure_from_args().final_summary();
    } else {
        smoke_validate_bench_inputs();
    }
}
