# excalidraw-native

[![CI](https://github.com/quangdang46/excalidraw-native/actions/workflows/ci.yml/badge.svg)](https://github.com/quangdang46/excalidraw-native/actions/workflows/ci.yml)

> Native Rust renderer for `.excalidraw` files. No browser. No Node.js. No Puppeteer.

`excalidraw-native` parses Excalidraw JSON and renders it to **SVG**, **PNG**, terminal previews, and MCP tool outputs while preserving the hand-drawn Excalidraw style through [`rough-rs`](https://crates.io/crates/rough-rs).

The goal is not to replace the Excalidraw app. The goal is to provide a fast, headless, embeddable rendering backend for tools, CI pipelines, editors, and AI agents.

```text
.excalidraw JSON
        ↓
parse / validate / normalize
        ↓
render with Rust + rough-rs
        ↓
SVG / PNG / terminal / MCP
```

---

## Project status

`excalidraw-native` is currently in active development.

The current focus is **renderer correctness first**:

- parse real `.excalidraw` files safely;
- preserve element geometry, z-order, colors, opacity, text, images, frames, and arrowheads;
- render stable SVG/PNG without a browser;
- document known deviations from Excalidraw web export;
- expose a clean Rust API that CLI, TUI, and MCP layers can reuse.

APIs may change before the first stable release.

---

## Why this exists

Existing Excalidraw export workflows usually depend on browser automation. That works, but it has real costs:

- Chromium/Puppeteer startup is expensive.
- Node.js is not ideal for minimal CI or server environments.
- Headless rendering is harder to embed in Rust applications.
- AI agents need quick render/preview loops without launching a browser.

Excalidraw files already contain absolute coordinates. Unlike Mermaid or Graphviz, a renderer does not need to compute graph layout for normal `.excalidraw` files. It can parse the scene and paint the stored geometry directly.

That makes a native renderer practical.

---

## What it does

Target v0.1 features:

| Feature | Target |
|---|---|
| Parse `.excalidraw` JSON | Yes |
| Validate malformed or suspicious files | Yes |
| Normalize scene data | Yes |
| Render SVG | Yes |
| Render PNG through `resvg` | Yes |
| Preserve hand-drawn rough style | Yes, through `rough-rs` |
| CLI binary | `excd` |
| Terminal preview | Planned |
| MCP server | Planned |
| Mermaid → Excalidraw | v0.2 |

---

## What it does not do

`excalidraw-native` is a renderer, not an editor.

Non-goals:

- no interactive whiteboard;
- no collaborative sync;
- no Firebase integration;
- no browser canvas runtime;
- no element editing UI;
- no iframe or embeddable web-content rendering;
- no promise of pixel-perfect parity with Excalidraw web export.

The target is **structural correctness** and **visual equivalence**, not a full browser clone.

---

## Quick start

### Install from source

```bash
git clone https://github.com/quangdang46/excalidraw-native.git
cd excalidraw-native
cargo build --release
```

The CLI binary is planned as:

```bash
./target/release/excd --help
```

### Convert to SVG

```bash
excd to-svg diagram.excalidraw diagram.svg
```

### Convert to PNG

```bash
excd to-png diagram.excalidraw diagram.png --scale 2
```

### Inspect a file

```bash
excd info diagram.excalidraw
excd validate diagram.excalidraw
```

### Preview in terminal

```bash
excd view diagram.excalidraw
```

Terminal preview is designed for Kitty, Sixel, iTerm2, and halfblock fallback modes.

---

## CLI overview

Planned commands:

```bash
excd view <file>              # open a terminal viewer
excd to-svg <file> [output]   # render SVG
excd to-png <file> [output]   # render PNG
excd convert <file> [output]  # infer output format from extension
excd info <file>              # print scene summary
excd validate <file>          # validate Excalidraw JSON
excd serve                    # start MCP server over stdio
```

Render quality modes:

```bash
excd to-svg file.excalidraw              # full quality
excd to-svg file.excalidraw --fast-svg   # skip embedded font data in SVG
excd to-svg file.excalidraw --clean      # clean geometric output, no rough style
```

---

## Rust API sketch

The public API is designed around a simple pipeline:

```rust
use excalidraw_core::parse_str;
use excalidraw_render::{render_svg, RenderOptions, RenderQuality};

fn main() -> anyhow::Result<()> {
    let input = std::fs::read_to_string("diagram.excalidraw")?;

    let file = parse_str(&input)?;
    let scene = file.normalize()?;

    let svg = render_svg(
        &scene,
        &RenderOptions {
            quality: RenderQuality::Full,
            ..Default::default()
        },
    )?;

    std::fs::write("diagram.svg", svg)?;
    Ok(())
}
```

Exact APIs may change before the first stable release.

---

## Workspace layout

```text
excalidraw-native/
├── Cargo.toml
├── README.md
├── PLAN.md
├── CHANGELOG.md
├── crates/
│   ├── excalidraw-core/      # parse, validate, normalize, scene model
│   ├── excalidraw-render/    # scene → SVG/PNG
│   ├── excalidraw-cli/       # excd binary
│   ├── excalidraw-tui/       # terminal viewer
│   └── excalidraw-mcp/       # MCP tools for AI agents
└── tests/
    ├── fixtures/             # real and synthetic .excalidraw files
    ├── oracle/               # Excalidraw web-export comparison fixtures
    └── integration/          # CLI and renderer integration tests
```

### Crates

| Crate | Responsibility |
|---|---|
| `excalidraw-core` | File format types, parsing, validation, normalization, scene model |
| `excalidraw-render` | SVG/PNG renderer, text, shapes, arrows, images, frames |
| `excalidraw-cli` | User-facing `excd` binary |
| `excalidraw-tui` | Terminal image preview and pan/zoom UI |
| `excalidraw-mcp` | MCP server tools for agents |

Consumer crates must not duplicate rendering logic. Rendering behavior belongs in `excalidraw-core` and `excalidraw-render`.

The workspace root intentionally has no `src/` directory. It only coordinates
the publishable crates and shared dependencies. During local co-development,
`excalidraw-render` depends on `rough-rs` through `../rough-rs`; release work
switches that dependency to the published crate version.

---

## Rendering fidelity contract

This project is correctness-first.

Fidelity tiers:

| Tier | Meaning |
|---|---|
| F0 — Parse correctness | Accept real `.excalidraw` files across versions and generators. |
| F1 — Structural correctness | Correct element type, position, size, rotation, z-order, color, opacity, and bounds. |
| F2 — Visual equivalence | Output looks close to Excalidraw web export for normal diagrams. |
| F3 — Documented deviations | Known differences are recorded and tested. |
| F4 — Pixel parity | Not required for v0.1, but useful for selected future regression tests. |

Renderer rules:

- Supported elements should render.
- Unsupported elements should not panic.
- Unknown elements should preserve raw JSON.
- Missing images should render placeholders with warnings.
- Rendering APIs should return structured errors and warnings.
- Bounds must include rotation, stroke width, roughness margin, arrowheads, text, and frame labels.

---

## Supported elements

Target v0.1 support:

| Element | Status target | Notes |
|---|---|---|
| `rectangle` | v0.1 | rough style, fill, stroke, rounded corners |
| `ellipse` | v0.1 | rough style, fill, stroke |
| `diamond` | v0.1 | rough polygon |
| `line` | v0.1 | multi-point, dash styles |
| `arrow` | v0.1 | explicit arrowhead geometry in full mode |
| `text` | v0.1 | standalone and bound text |
| `freedraw` | v0.1 | pressure-aware path generation |
| `image` | v0.1 | embedded data URLs, crop, flip, placeholder on missing data |
| `frame` | v0.1 | border, label, optional clipping |
| `magicframe` | later | placeholder or frame-like rendering |
| `embeddable` | unsupported | placeholder only |
| `iframe` | unsupported | placeholder only |

---

## Arrowhead policy

Full mode renders arrowheads as explicit SVG geometry, not SVG markers.

This gives the renderer full control over:

- endpoint placement;
- line shortening;
- stroke width;
- opacity;
- filled vs outlined shapes;
- special arrowheads like bar, dot, circle, diamond, and crowfoot.

Fast or clean modes may use SVG markers where acceptable.

Supported arrowheads:

| Excalidraw arrowhead | Rendering strategy |
|---|---|
| `arrow` | open chevron |
| `triangle` | filled triangle |
| `triangle_outline` | outlined triangle |
| `bar` | perpendicular stroke |
| `dot` | filled circle |
| `circle` | outlined circle |
| `diamond` | diamond shape |
| `crowfoot` | three-prong crowfoot |

---

## Relationship with rough-rs

[`rough-rs`](https://crates.io/crates/rough-rs) is the rough rendering engine used by `excalidraw-native`.

The two projects are expected to evolve together.

If Excalidraw fidelity requires a new rough rendering behavior, the preferred workflow is:

1. add or fix the primitive in `rough-rs`;
2. add parity tests in `rough-rs`;
3. consume the updated API from `excalidraw-native`;
4. add an Excalidraw fixture that proves the behavior.

This keeps the rough path engine reusable beyond Excalidraw while allowing `excalidraw-native` to drive real-world requirements.

---

## MCP tools

The MCP server is designed for AI agent workflows.

Planned tools:

| Tool | Purpose |
|---|---|
| `render_file` | Render `.excalidraw` to base64 PNG |
| `to_svg` | Convert file to SVG |
| `to_png` | Convert file to PNG |
| `parse_elements` | Return structured element data |
| `describe_scene` | Summarize scene without rendering |
| `validate` | Validate file or JSON input |

Example agent workflow:

```text
agent creates .excalidraw JSON
        ↓
validate
        ↓
describe_scene
        ↓
render_file
        ↓
agent reviews or attaches PNG/SVG output
```

---

## Mermaid roadmap

Mermaid support is planned for v0.2.

The goal is:

```text
Mermaid text
    → merman parse/layout
    → Excalidraw elements
    → excalidraw-native renderer
    → SVG/PNG
```

v0.2 should be implemented as a separate crate, for example:

```text
crates/excalidraw-mermaid/
```

v0.1 should remain focused on rendering existing `.excalidraw` files correctly.

---

## Testing strategy

Correctness requires more than “valid SVG”.

Test layers:

1. **Parse compatibility tests**
   - real `.excalidraw` files;
   - missing optional fields;
   - old and new element shapes;
   - unknown elements;
   - images and frames.

2. **Structural snapshot tests**
   - normalized scene JSON;
   - element count and type summary;
   - z-order;
   - bounds;
   - binding resolution.

3. **SVG validity tests**
   - parse generated SVG through `usvg`;
   - verify path data;
   - verify defs, clip paths, and references.

4. **Visual regression tests**
   - render SVG to PNG;
   - compare against golden images;
   - allow a documented tolerance.

5. **Oracle fixtures**
   - compare selected outputs against official Excalidraw web exports;
   - document accepted deviations.

6. **Performance benchmarks**
   - benchmark library paths separately from CLI paths;
   - never let benchmark targets override renderer correctness.

---

## Development

### Requirements

- Rust stable
- Cargo
- Optional: a terminal with Kitty or Sixel support for TUI preview

### Build

```bash
cargo build
```

### Test

```bash
cargo test
```

### Format and lint

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

### Run CLI locally

```bash
cargo run -p excalidraw-cli -- to-svg tests/fixtures/simple_shapes.excalidraw out.svg
```

---

## Contributing

Contributors are welcome.

Good first contribution areas:

- add real `.excalidraw` fixtures;
- improve parser tolerance;
- implement one element renderer;
- add SVG validity tests;
- add oracle comparison fixtures;
- improve CLI errors and help text;
- document known visual deviations;
- improve `rough-rs` primitives needed by Excalidraw rendering.

### How to add a new element renderer

1. Add or update the type in `excalidraw-core`.
2. Add parse tests for real JSON examples.
3. Normalize the element if needed.
4. Implement rendering in `excalidraw-render`.
5. Add a minimal fixture.
6. Add an SVG validity test.
7. Add a visual regression fixture if the element has complex geometry.
8. Document known deviations from Excalidraw web export.

### Contribution principles

- Prefer correctness over shortcuts.
- Preserve unknown input when possible.
- Do not panic on unsupported real-world files.
- Keep CLI/TUI/MCP as consumers, not renderer forks.
- Add tests with every renderer behavior change.
- If a fix belongs in `rough-rs`, fix it there first.

---

## Security and robustness

`.excalidraw` files are JSON and may contain large embedded images or many points.

The parser and validator should enforce limits for:

- payload size;
- element count;
- points per element;
- text length;
- embedded file count;
- embedded file size;
- malformed image data;
- deeply invalid or unknown fields.

Invalid files should return structured errors, not crashes.

---

## Versioning

Before `1.0`, APIs may change while renderer semantics are still being stabilized.

Expected release line:

| Version | Focus |
|---|---|
| `0.1.x` | Native `.excalidraw` renderer, CLI, SVG/PNG |
| `0.2.x` | Mermaid → Excalidraw pipeline |
| `0.3.x` | Fidelity improvements, more element coverage, stronger visual tests |
| `1.0.0` | Stable public API and CLI contract |

---

## License

License is to be decided before publishing to crates.io.

Recommended options:

- MIT;
- Apache-2.0;
- MIT OR Apache-2.0.

Make sure bundled fonts, fixtures, and test assets are compatible with the chosen license.

---

## Acknowledgements

`excalidraw-native` is inspired by the Excalidraw ecosystem and by the need for fast, browserless rendering in CI, editor, server, and AI-agent workflows.

The hand-drawn rendering layer is powered by [`rough-rs`](https://crates.io/crates/rough-rs).
