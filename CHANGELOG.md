# Changelog

All notable changes to `excalidraw-native` are documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
starting from `0.1.0`.

## [Unreleased]

### Added

- **`excalidraw-mermaid 0.2.0`** — bumped from the `v0.2 alpha` line. The
  crate now carries its own `version = "0.2.0"` (rest of the workspace
  stays on `0.1.0`) to reflect that every Tier 1 diagram closes the
  `excalidraw-native-PLAN.md` §21 checklist: parse, layout, conversion
  snapshot, SVG render validity, visual regression after render, and
  unsupported fallback are all backed by tests.
- **Visual regression snapshots for Mermaid** — new
  `crates/excalidraw-mermaid/tests/snapshots.rs` covers all 17 Tier 1
  fixtures with an `insta::assert_json_snapshot!` of the canonical
  `.excalidraw` JSON plus an `insta::assert_snapshot!` of the SVG
  rendered by `excalidraw-render`. Snapshots are byte-stable because
  element seeds are pinned to `STABLE_SEED = 1_337` and the renderer
  uses deterministic rough geometry.
- **`excalidraw-mermaid` crate (v0.2)** — workspace crate that converts
  Mermaid source text into Excalidraw scenes via `merman-core` parsing and
  `merman-render` layout. Public API: `parse_to_excalidraw`,
  `parse_to_excalidraw_file`, `parse_to_excalidraw_value`,
  `MermaidConvertOptions`, `FlowchartCurve`, `OnUnsupported`,
  `MermaidConvertError`.
- **Tier 1 Mermaid converters**: flowchart / graph, sequence, class, state,
  ER diagrams. Cluster / subgraph nodes are emitted as Excalidraw frames,
  arrows carry explicit arrowheads (`arrow`, `bar`, `circle`, `crowfoot`),
  edge labels are preserved, and pseudo-states (`[*]`) render as filled
  circles. Unsupported diagrams fall back to a placeholder rectangle by
  default or surface `MermaidConvertError::UnsupportedDiagram` when
  `OnUnsupported::Error` is requested.
- **CLI** — `excd mermaid-to-excalidraw` (file → file, file → stdout, stdin
  → stdout) and `excd mermaid` (renders directly to `.svg`, `.png` or
  `.excalidraw` based on the output extension). Both subcommands accept
  `--font-size`, `--curve linear|basis`, `--on-unsupported placeholder|error`,
  and `--max-edges`.
- **MCP** — new `mermaid_to_excalidraw` tool that accepts inline `source` or
  a `path`, plus the same option set as the CLI. Returns a JSON payload
  with the stringified `.excalidraw` document and an `element_count`
  summary for downstream chaining with `parse_elements` / `to_svg` /
  `render_file`.
- **Fixtures** — `tests/fixtures/mermaid/{flowchart_basic, flowchart_subgraph,
  sequence_basic, class_basic, state_basic, er_basic}.mmd` cover every Tier 1
  diagram type through the v0.1 SVG renderer (`crates/excalidraw-mermaid/tests/integration.rs`).
- **Docs** — README now has a dedicated *Mermaid → Excalidraw* section
  covering the pipeline, CLI flags, MCP tool, and Rust API. Known
  fidelity deviations are tracked in
  [`docs/deviations.md`](docs/deviations.md).

### Changed

- **`excd mermaid` output is now `-o`/`--output`**, matching `excd to-svg`,
  `excd to-png` and `excd mermaid-to-excalidraw`. The previous positional
  output path still works for backward compatibility with v0.2-alpha
  scripts but prints a deprecation warning. `--output` is required; the
  output format is still inferred from the file extension.
- **`rough-rs` is now a crates.io dependency (`0.1`)**. The previous local
  path dependency through `../rough-rs` is no longer required. Building
  the workspace no longer needs a sibling `rough-rs` checkout.
- **TUI stack upgraded** — `ratatui` 0.29 → 0.30 and `ratatui-image` 8 → 11
  with `default-features = false` to resolve the transitive
  `unicode-width` clash introduced when adding `merman-core`/`merman-render`.
- **Clippy** — codebase compiles cleanly under
  `cargo clippy --workspace --all-targets -- -D warnings`.

### Fidelity / known deviations

- See [`docs/deviations.md`](docs/deviations.md) for the documented gaps
  against Excalidraw web exports (rough hachure spacing, embedded font
  pixel metrics, image cache invalidation, Mermaid edge-routing
  approximations).

## [0.1.0] — pending crates.io publish

The first crates.io release will pin the workspace at `0.1.0`. Planned
publish order: `excalidraw-core` → `excalidraw-render` → `excalidraw-tui`
→ `excalidraw-mcp` → `excalidraw-cli`. The `excalidraw-mermaid` crate is
released alongside as `0.2.0` once the workspace publish lands, since the
`v0.2` Mermaid scope is now complete (see `excalidraw-native-PLAN.md`
§21 and the `### Added` block above).

### Highlights (carried over from pre-0.1 development)

- Native Rust parser/normalizer for `.excalidraw` documents with safety
  caps and structured warnings.
- SVG + PNG renderer with three quality tiers (`Full`, `Fast`, `Clean`)
  and a `rough-rs`-backed hand-drawn style.
- `excd` CLI: `to-svg`, `to-png`, `convert`, `info`, `validate`, `view`,
  `serve`, plus image-policy / background / unsupported-element knobs.
- `excalidraw-tui` interactive viewer with Kitty / Sixel / iTerm2 /
  halfblock fallback.
- `excalidraw-mcp` server tools: `render_file`, `to_svg`, `to_png`,
  `parse_elements`, `describe_scene`, `validate`.
- Workspace-wide `cargo test`, `cargo fmt`, `cargo clippy -D warnings`
  enforcement.
