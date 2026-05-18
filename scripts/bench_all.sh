#!/usr/bin/env bash
# Run every Criterion benchmark in the workspace, save the raw logs under
# docs/benchmarks/, and regenerate the BENCHMARKS.md overview diagram via
# the locally-built `excd` binary.
#
# Usage:
#   scripts/bench_all.sh           # full run
#   SKIP_BUILD=1 scripts/bench_all.sh   # reuse existing target/release/excd

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

OUT_DIR="docs/benchmarks"
mkdir -p "$OUT_DIR"

EXCD_BIN="target/release/excd"

if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
    echo "==> building release excd"
    cargo build --release --bin excd
fi

if [[ ! -x "$EXCD_BIN" ]]; then
    echo "error: $EXCD_BIN not found; run without SKIP_BUILD=1" >&2
    exit 1
fi

run_bench() {
    local crate="$1" bench="$2"
    local log="$OUT_DIR/${bench}.log"
    echo "==> bench $crate::$bench  -> $log"
    cargo bench -p "$crate" --bench "$bench" 2>&1 | tee "$log" >/dev/null
}

run_bench excalidraw-core   lib_bench
run_bench excalidraw-render render_bench
EXCD_RUN_CRITERION_BENCHES=1 run_bench excalidraw-mermaid mermaid_bench
run_bench excalidraw-cli    cli_e2e
run_bench excalidraw-cli    mcp_e2e

DIAGRAM_SRC="$OUT_DIR/benchmarks_overview.mmd"
if [[ -f "$DIAGRAM_SRC" ]]; then
    echo "==> regenerating overview diagram from $DIAGRAM_SRC"
    "$EXCD_BIN" mermaid "$DIAGRAM_SRC" "$OUT_DIR/benchmarks_overview.svg"
    "$EXCD_BIN" mermaid "$DIAGRAM_SRC" "$OUT_DIR/benchmarks_overview.png"
    "$EXCD_BIN" mermaid "$DIAGRAM_SRC" "$OUT_DIR/benchmarks_overview.excalidraw"
else
    echo "warn: $DIAGRAM_SRC missing, skipping diagram regen" >&2
fi

echo
echo "==> medians captured:"
grep -B1 'time:' "$OUT_DIR"/*.log | grep -v '^--$' || true

echo
echo "Done. See BENCHMARKS.md and $OUT_DIR/."
