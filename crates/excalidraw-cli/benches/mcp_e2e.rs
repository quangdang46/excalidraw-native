//! End-to-end MCP benchmarks over the `excd serve` stdio transport.

use std::path::{Path, PathBuf};

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rmcp::model::CallToolRequestParams;
use rmcp::transport::TokioChildProcess;
use rmcp::ServiceExt;

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

async fn call_stdio_tool(tool: &'static str, input: &Path) {
    let mut command = tokio::process::Command::new("../../target/release/excd");
    command.arg("serve");

    let transport = TokioChildProcess::new(command).expect("spawn excd serve");
    let client = ().serve(transport).await.expect("start MCP client");

    let arguments = serde_json::json!({ "path": input })
        .as_object()
        .unwrap()
        .clone();
    let result = client
        .peer()
        .call_tool(CallToolRequestParams::new(tool).with_arguments(arguments))
        .await
        .expect("call MCP tool");

    assert_ne!(result.is_error, Some(true), "MCP tool returned error");
    let value: serde_json::Value = result.into_typed().expect("decode MCP response JSON");
    black_box(value);

    client.cancel().await.expect("stop MCP client");
}

fn bench_mcp_stdio_roundtrips(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut group = c.benchmark_group("mcp_e2e");
    group.sample_size(10);

    for (case, tool, file, target_ms, fail_ms) in [
        (
            "simple_parse_stdio",
            "parse_elements",
            "simple_shapes.excalidraw",
            150,
            1500,
        ),
        (
            "complex_describe_stdio",
            "describe_scene",
            "complex_diagram.excalidraw",
            180,
            1800,
        ),
        (
            "large_validate_stdio",
            "validate",
            "large_200_elements.excalidraw",
            250,
            2500,
        ),
    ] {
        let input = fixture(file);
        let target = BenchTarget::new("mcp_e2e", case, target_ms, fail_ms);
        report_target(target);
        group.bench_with_input(BenchmarkId::new(tool, target.id()), &target, |b, _| {
            b.to_async(&runtime)
                .iter(|| call_stdio_tool(tool, black_box(&input)))
        });
    }

    group.finish();
}

criterion_group!(mcp_e2e, bench_mcp_stdio_roundtrips);
criterion_main!(mcp_e2e);
