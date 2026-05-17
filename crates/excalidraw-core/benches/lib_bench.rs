//! Benchmarks for parse and normalize library paths.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use excalidraw_core::{normalize_file, parse_str};

include!("../../../benches/benchmark_targets.rs");

fn simple_shapes_json() -> &'static str {
    r#"{"elements":[
        {"type":"rectangle","id":"r1","x":10,"y":10,"width":100,"height":60},
        {"type":"ellipse","id":"e1","x":130,"y":10,"width":80,"height":60},
        {"type":"text","id":"t1","x":10,"y":90,"width":200,"height":25,"text":"Hello","originalText":"Hello","fontSize":20,"fontFamily":1}
    ]}"#
}

fn large_json() -> String {
    let mut elements = Vec::new();
    for i in 0..200 {
        let etype = match i % 5 {
            0 => "rectangle",
            1 => "ellipse",
            2 => "text",
            3 => "arrow",
            _ => "line",
        };
        let extra = if etype == "text" {
            format!(",\"text\":\"Item {i}\",\"originalText\":\"Item {i}\",\"fontSize\":14,\"fontFamily\":1")
        } else if etype == "arrow" || etype == "line" {
            ",\"points\":[[0,0],[60,0]]".to_owned()
        } else {
            String::new()
        };
        elements.push(format!(
            "{{\"type\":\"{etype}\",\"id\":\"e{i}\",\"x\":{x},\"y\":{y},\"width\":60,\"height\":40{extra}}}",
            x = (i % 20) * 70,
            y = (i / 20) * 50,
        ));
    }
    format!("{{\"elements\":[{}]}}", elements.join(","))
}

fn bench_parse_simple(c: &mut Criterion) {
    let target = BenchTarget::new("parse_lib", "simple", 2, 20);
    report_target(target);
    c.bench_with_input(BenchmarkId::new("parse", target.id()), &target, |b, _| {
        b.iter(|| parse_str(black_box(simple_shapes_json())))
    });
}

fn bench_parse_large(c: &mut Criterion) {
    let json = large_json();
    let target = BenchTarget::new("parse_lib", "large_200", 20, 200);
    report_target(target);
    c.bench_with_input(BenchmarkId::new("parse", target.id()), &target, |b, _| {
        b.iter(|| parse_str(black_box(&json)))
    });
}

fn bench_normalize_simple(c: &mut Criterion) {
    let file = parse_str(simple_shapes_json()).unwrap();
    let target = BenchTarget::new("normalize_lib", "simple", 1, 10);
    report_target(target);
    c.bench_with_input(
        BenchmarkId::new("normalize", target.id()),
        &target,
        |b, _| b.iter(|| normalize_file(black_box(&file))),
    );
}

fn bench_normalize_large(c: &mut Criterion) {
    let json = large_json();
    let file = parse_str(&json).unwrap();
    let target = BenchTarget::new("normalize_lib", "large_200", 10, 100);
    report_target(target);
    c.bench_with_input(
        BenchmarkId::new("normalize", target.id()),
        &target,
        |b, _| b.iter(|| normalize_file(black_box(&file))),
    );
}

criterion_group!(parse_lib, bench_parse_simple, bench_parse_large);
criterion_group!(normalize_lib, bench_normalize_simple, bench_normalize_large);
criterion_main!(parse_lib, normalize_lib);
