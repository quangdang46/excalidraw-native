# excalidraw-native — PLAN.md

> Native Rust renderer for `.excalidraw` files. No browser, no Node.js, no Puppeteer.
> Parse → normalize → render → SVG/PNG/terminal/MCP.
> CLI binary: `excd`.
> Rendering style is powered by **your `rough-rs` crate**. During development, `excalidraw-native` and `rough-rs` may be co-developed together; renderer requirements are allowed to drive `rough-rs` API improvements.

---

## Table of Contents

1. [Goals & Non-Goals](#1-goals--non-goals)
2. [Correctness Contract](#2-correctness-contract)
3. [Why excalidraw-native](#3-why-excalidraw-native)
4. [Performance Architecture](#4-performance-architecture)
5. [Workspace Layout](#5-workspace-layout)
6. [Data Flow](#6-data-flow)
7. [Crate: excalidraw-core](#7-crate-excalidraw-core)
8. [Crate: excalidraw-render](#8-crate-excalidraw-render)
9. [Crate: excalidraw-tui](#9-crate-excalidraw-tui)
10. [Crate: excalidraw-mcp](#10-crate-excalidraw-mcp)
11. [Crate: excalidraw-cli](#11-crate-excalidraw-cli)
12. [Element Type Specifications](#12-element-type-specifications)
13. [Rendering Pipeline Per Element](#13-rendering-pipeline-per-element)
14. [SVG Document Assembly](#14-svg-document-assembly)
15. [Terminal Display Strategy](#15-terminal-display-strategy)
16. [MCP Tool Definitions](#16-mcp-tool-definitions)
17. [Dependencies](#17-dependencies)
18. [Testing Strategy & Benchmarks](#18-testing-strategy--benchmarks)
19. [Implementation Phases](#19-implementation-phases)
20. [Design Decisions & Rationale](#20-design-decisions--rationale)
21. [v0.2 Roadmap — Mermaid → Excalidraw](#21-v02-roadmap--mermaid--excalidraw)

---

## 1. Goals & Non-Goals

### Goals

- **`.excalidraw` → SVG** — native Rust rendering with hand-drawn style preserved through `rough-rs`.
- **`.excalidraw` → PNG** — rasterized from generated SVG via `resvg`.
- **Terminal viewer** — `excd view file.excalidraw` displays rendered output in terminal.
- **MCP server** — AI agents can render diagrams, inspect elements, convert formats, and validate files.
- **Zero browser dependency** — no Chromium, no Node.js, no Puppeteer.
- **Headless-friendly** — works in WSL2, CI, remote Linux, and server environments.
- **crates.io publishable** — each crate independently usable.
- **Fast cold start** — eliminate browser startup overhead; CLI and library paths must be benchmarked separately.
- **Correctness-first renderer** — if implementation time increases, prioritize stable renderer semantics over shortcuts.

### Non-Goals

- Editing / interactive whiteboard behavior.
- Element CRUD through MCP / programmatic editing. Existing editing tools may generate `.excalidraw`; `excd` renders and validates.
- Mermaid → Excalidraw conversion in v0.1. Planned for v0.2.
- Browser-dependent content rendering for iframe/embeddable elements.
- Full pixel-perfect match to Excalidraw web export.
- Multiplayer / collaborative sync / Firebase behavior.
- Magic frame AI features.
- Browser canvas renderer.
- Font subsetting in v0.1. Use bundled known fonts plus fallback.

---

## 2. Correctness Contract

This project is not just “fast export”. The renderer must be correct enough to become a stable backend.

### 2.1 Fidelity Levels

Rendering fidelity is defined in tiers:

| Tier | Name | Requirement |
|---|---|---|
| F0 | Parse correctness | Accept real `.excalidraw` files across versions/tools without unnecessary failure. |
| F1 | Structural correctness | Correct element type, position, size, rotation, z-order, color, opacity, and bounding box. |
| F2 | Visual equivalence | Visually close to Excalidraw web export for normal diagrams. |
| F3 | Documented deviations | Any known difference from Excalidraw web is recorded as an accepted deviation. |
| F4 | Pixel parity | Not required. Useful only for selected future regression tests. |

### 2.2 Error Policy

Renderer behavior must be predictable:

- Valid supported elements should render.
- Unsupported elements should not panic.
- Unknown elements should preserve raw JSON.
- Missing image data should produce a placeholder and warning.
- Invalid/corrupt files should return structured validation errors.
- Rendering functions must return warnings alongside output where possible.

### 2.3 Consumer Boundary

`excalidraw-tui`, `excalidraw-mcp`, and `excalidraw-cli` are consumers. They must not implement their own render logic.

All rendering behavior belongs in:

- `excalidraw-core` — parse, validate, normalize, scene model
- `excalidraw-render` — SVG/PNG rendering and layout-neutral visual output

---

## 3. Why excalidraw-native

Existing approaches usually require a browser or Node.js:

- Puppeteer/Chromium export pays a cold-start cost even for tiny diagrams.
- Excalidraw itself is a React/browser app.
- Mermaid → Excalidraw tools usually convert but do not natively render Excalidraw.

The core advantage of native `.excalidraw` rendering:

- Excalidraw stores absolute coordinates.
- No graph layout engine is required for normal `.excalidraw` files.
- The renderer only parses, normalizes, and paints known geometry.
- Startup overhead is the main avoidable cost.

Use cases:

- CI/CD diagram rendering without Chromium.
- Neovim/Helix terminal preview.
- AI agent workflows through MCP.
- Rust applications that want to emit and render Excalidraw diagrams.
- Fast server-side preview generation.

---

## 4. Performance Architecture

### 4.1 Where the Speedup Comes From

| Source of speedup | Reason |
|---|---|
| No Chromium startup | Eliminates browser cold start. |
| No Node.js process | Avoids JS runtime spawn. |
| No layout engine for `.excalidraw` | Coordinates already exist. |
| Native parse/render path | Low overhead and easy library embedding. |
| Optional fast SVG mode | Skip embedded font CSS for lighter SVG. |
| Optional clean mode | Skip rough path generation when hand-drawn style is unnecessary. |

### 4.2 Benchmark Groups

Do not mix library benchmarks with CLI process benchmarks.

| Group | Measures |
|---|---|
| `parse_lib` | `parse_str` + validation + serde cost |
| `normalize_lib` | scene normalization, z-order, bounds, binding indexes |
| `render_svg_lib` | scene → SVG string in-process |
| `render_png_lib` | scene → SVG → PNG bytes in-process |
| `cli_e2e` | binary spawn + file IO + render + output write |
| `mcp_e2e` | stdio MCP request/response path |

### 4.3 Render Quality Modes

CLI examples:

```bash
excd to-svg file.excalidraw
excd to-svg file.excalidraw --fast-svg
excd to-svg file.excalidraw --clean
```

Library API:

```rust
pub enum RenderQuality {
    /// Highest fidelity: rough-rs strokes, embedded Excalifont where supported.
    Full,

    /// Rough-rs strokes but no embedded font data in SVG.
    FastSvg,

    /// Clean geometric primitives, no rough-rs path generation.
    Clean,
}
```

Important distinction:

- `RenderQuality::Clean` is a renderer mode.
- `roughness = 0` is an Excalidraw element property.
- These are not the same thing.

### 4.4 Realistic Targets

Targets should be tracked but not allowed to override correctness.

| Operation | Target | Fail Threshold |
|---|---:|---:|
| parse/simple | < 0.2ms | 1ms |
| parse/complex | < 1ms | 5ms |
| normalize/complex | < 2ms | 10ms |
| svg/simple_5elem | < 3ms | 10ms |
| svg/complex_50elem | < 20ms | 50ms |
| svg/large_200elem | < 80ms | 180ms |
| png/simple_1x | < 50ms | 120ms |
| cli_e2e/svg_small | < 80ms | 200ms |

---

## 5. Workspace Layout

```text
excalidraw-native/
├── Cargo.toml
├── PLAN.md
├── README.md
├── CHANGELOG.md
├── .github/
│   └── workflows/
│       ├── ci.yml
│       └── release.yml
├── crates/
│   ├── excalidraw-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── color.rs
│   │       ├── types.rs
│   │       ├── parse.rs
│   │       ├── validate.rs
│   │       ├── ir.rs
│   │       ├── bounds.rs
│   │       ├── zorder.rs
│   │       └── warnings.rs
│   │
│   ├── excalidraw-render/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── renderer.rs
│   │       ├── options.rs
│   │       ├── svg.rs
│   │       ├── png.rs
│   │       ├── rough.rs
│   │       ├── font.rs
│   │       ├── text_layout.rs
│   │       ├── transform.rs
│   │       ├── stroke.rs
│   │       ├── arrowhead.rs
│   │       ├── clip.rs
│   │       └── elements/
│   │           ├── mod.rs
│   │           ├── rectangle.rs
│   │           ├── ellipse.rs
│   │           ├── diamond.rs
│   │           ├── linear.rs
│   │           ├── text.rs
│   │           ├── freedraw.rs
│   │           ├── image.rs
│   │           ├── frame.rs
│   │           └── unsupported.rs
│   │
│   ├── excalidraw-tui/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── app.rs
│   │       ├── viewer.rs
│   │       └── protocol.rs
│   │
│   ├── excalidraw-mcp/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── server.rs
│   │       └── tools/
│   │           ├── mod.rs
│   │           ├── render.rs
│   │           ├── parse.rs
│   │           ├── describe.rs
│   │           ├── convert.rs
│   │           └── validate.rs
│   │
│   └── excalidraw-cli/
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
│
└── tests/
    ├── fixtures/
    │   ├── simple_shapes.excalidraw
    │   ├── text_standalone.excalidraw
    │   ├── text_containers.excalidraw
    │   ├── arrows_basic.excalidraw
    │   ├── arrows_bound.excalidraw
    │   ├── arrows_elbow.excalidraw
    │   ├── freedraw.excalidraw
    │   ├── image_embed.excalidraw
    │   ├── frame_clip.excalidraw
    │   ├── complex_diagram.excalidraw
    │   └── large_200_elements.excalidraw
    ├── golden/
    │   ├── svg/
    │   └── png/
    └── integration/
        ├── parse_compat.rs
        ├── render_smoke.rs
        ├── visual_regression.rs
        └── cli_tests.rs
```

No `src/` at workspace root. The root is a workspace only.

---

## 6. Data Flow

```text
.excalidraw UTF-8 JSON
        │
        ▼
excalidraw-core::parse_str()
        │
        ▼
ExcalidrawFile {
  file_type,
  version,
  source,
  elements,
  app_state,
  files,
  raw_unknown_fields
}
        │
        ▼
validate()
        │
        ▼
normalize()
        │
        ▼
Scene {
  elements,
  id_map,
  frame_children,
  bound_texts,
  bound_arrows,
  background_color,
  content_bounds,
  export_bounds,
  warnings
}
        │
        ▼
excalidraw-render::render_svg()
        │
        ├── SVG string
        │
        └── warnings
        │
        ▼
excalidraw-render::render_png()
        │
        └── PNG bytes through resvg
        │
        ▼
CLI / TUI / MCP
```

---

## 7. Crate: excalidraw-core

### 7.1 Responsibility

`excalidraw-core` owns:

- `.excalidraw` JSON parsing.
- Lenient compatibility with real files.
- Validation limits.
- Scene normalization.
- Z-order sorting.
- Binding index creation.
- Frame child grouping.
- Bounds calculation.
- Warning collection.

It must not depend on renderer-specific crates.

### 7.2 File Format

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ExcalidrawFile {
    #[serde(default = "default_file_type", rename = "type")]
    pub file_type: String,

    #[serde(default = "default_version")]
    pub version: u32,

    #[serde(default)]
    pub source: Option<String>,

    #[serde(default)]
    pub elements: Vec<Element>,

    #[serde(default, rename = "appState")]
    pub app_state: AppState,

    #[serde(default)]
    pub files: HashMap<String, FileData>,

    /// Unknown top-level fields are preserved for future compatibility.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppState {
    #[serde(default, rename = "viewBackgroundColor")]
    pub view_background_color: Option<String>,

    #[serde(default, rename = "gridSize")]
    pub grid_size: Option<u32>,

    #[serde(default)]
    pub theme: Option<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileData {
    #[serde(default, rename = "mimeType")]
    pub mime_type: String,

    #[serde(default)]
    pub id: String,

    #[serde(default, rename = "dataURL")]
    pub data_url: String,

    #[serde(default)]
    pub created: Option<u64>,

    #[serde(default)]
    pub last_retrieved: Option<u64>,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}
```

### 7.3 Base Element

Parser must be lenient. Optional or version-dependent fields must have defaults.

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct BaseElement {
    pub id: String,

    #[serde(default)]
    pub x: f64,

    #[serde(default)]
    pub y: f64,

    #[serde(default)]
    pub width: f64,

    #[serde(default)]
    pub height: f64,

    #[serde(default)]
    pub angle: f64,

    #[serde(default = "default_stroke_color", rename = "strokeColor")]
    pub stroke_color: String,

    #[serde(default = "default_background_color", rename = "backgroundColor")]
    pub background_color: String,

    #[serde(default = "default_fill_style", rename = "fillStyle")]
    pub fill_style: FillStyle,

    #[serde(default = "default_stroke_width", rename = "strokeWidth")]
    pub stroke_width: f64,

    #[serde(default = "default_stroke_style", rename = "strokeStyle")]
    pub stroke_style: StrokeStyle,

    #[serde(default = "default_roughness")]
    pub roughness: f64,

    #[serde(default = "default_opacity")]
    pub opacity: f64,

    #[serde(default)]
    pub seed: u64,

    #[serde(default, rename = "isDeleted")]
    pub is_deleted: bool,

    #[serde(default, rename = "groupIds")]
    pub group_ids: Vec<String>,

    #[serde(default, rename = "frameId")]
    pub frame_id: Option<String>,

    #[serde(default, rename = "boundElements")]
    pub bound_elements: Vec<BoundElement>,

    #[serde(default)]
    pub roundness: Option<Roundness>,

    #[serde(default)]
    pub version: u64,

    #[serde(default)]
    pub link: Option<String>,

    #[serde(default)]
    pub locked: bool,

    #[serde(default)]
    pub index: Option<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}
```

### 7.4 Enums

```rust
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FillStyle {
    Hachure,
    Solid,
    #[serde(rename = "cross-hatch")]
    CrossHatch,
    Dots,
    Dashed,
    #[serde(rename = "zigzag-line")]
    ZigzagLine,
    None,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StrokeStyle {
    Solid,
    Dashed,
    Dotted,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Roundness {
    #[serde(default, rename = "type")]
    pub roundness_type: u32,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BoundElement {
    pub id: String,

    #[serde(default, rename = "type")]
    pub element_type: String,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}
```

### 7.5 Element Types

Supported known types:

- `rectangle`
- `ellipse`
- `diamond`
- `arrow`
- `line`
- `freedraw`
- `text`
- `image`
- `frame`
- `magicframe`
- `embeddable`
- `iframe`

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ShapeElement {
    #[serde(flatten)]
    pub base: BaseElement,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LinearElement {
    #[serde(flatten)]
    pub base: BaseElement,

    #[serde(default)]
    pub points: Vec<[f64; 2]>,

    #[serde(default, rename = "startArrowhead")]
    pub start_arrowhead: Option<Arrowhead>,

    #[serde(default, rename = "endArrowhead")]
    pub end_arrowhead: Option<Arrowhead>,

    #[serde(default, rename = "startBinding")]
    pub start_binding: Option<ArrowBinding>,

    #[serde(default, rename = "endBinding")]
    pub end_binding: Option<ArrowBinding>,

    #[serde(default)]
    pub elbowed: Option<bool>,

    #[serde(default)]
    pub last_committed_point: Option<[f64; 2]>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Arrowhead {
    Arrow,
    Triangle,
    Bar,
    Dot,
    Circle,
    Diamond,
    #[serde(rename = "triangle_outline")]
    TriangleOutline,
    Crowfoot,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArrowBinding {
    #[serde(default, rename = "elementId")]
    pub element_id: String,

    #[serde(default, rename = "fixedPoint")]
    pub fixed_point: Option<[f64; 2]>,

    #[serde(default)]
    pub mode: Option<String>,

    #[serde(default)]
    pub focus: Option<f64>,

    #[serde(default)]
    pub gap: Option<f64>,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TextElement {
    #[serde(flatten)]
    pub base: BaseElement,

    #[serde(default)]
    pub text: String,

    #[serde(default, rename = "originalText")]
    pub original_text: Option<String>,

    #[serde(default = "default_font_size", rename = "fontSize")]
    pub font_size: f64,

    #[serde(default = "default_font_family", rename = "fontFamily")]
    pub font_family: u32,

    #[serde(default = "default_text_align", rename = "textAlign")]
    pub text_align: TextAlign,

    #[serde(default = "default_vertical_align", rename = "verticalAlign")]
    pub vertical_align: VerticalAlign,

    #[serde(default, rename = "containerId")]
    pub container_id: Option<String>,

    #[serde(default = "default_line_height", rename = "lineHeight")]
    pub line_height: f64,

    #[serde(default, rename = "autoResize")]
    pub auto_resize: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign {
    Left,
    Center,
    Right,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VerticalAlign {
    Top,
    Middle,
    Bottom,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FreedrawElement {
    #[serde(flatten)]
    pub base: BaseElement,

    #[serde(default)]
    pub points: Vec<[f64; 2]>,

    #[serde(default)]
    pub pressures: Vec<f64>,

    #[serde(default, rename = "simulatePressure")]
    pub simulate_pressure: bool,

    #[serde(default)]
    pub last_committed_point: Option<[f64; 2]>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageElement {
    #[serde(flatten)]
    pub base: BaseElement,

    #[serde(default, rename = "fileId")]
    pub file_id: Option<String>,

    #[serde(default)]
    pub status: String,

    #[serde(default)]
    pub scale: Option<[f64; 2]>,

    #[serde(default)]
    pub crop: Option<ImageCrop>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageCrop {
    #[serde(default)]
    pub x: f64,

    #[serde(default)]
    pub y: f64,

    #[serde(default)]
    pub width: f64,

    #[serde(default)]
    pub height: f64,

    #[serde(default, rename = "naturalWidth")]
    pub natural_width: f64,

    #[serde(default, rename = "naturalHeight")]
    pub natural_height: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FrameElement {
    #[serde(flatten)]
    pub base: BaseElement,

    #[serde(default)]
    pub name: Option<String>,

    #[serde(default, rename = "isCollapsed")]
    pub is_collapsed: Option<bool>,

    #[serde(default)]
    pub clip: Option<bool>,
}
```

### 7.6 Element Enum

Unknown elements preserve raw JSON.

```rust
#[derive(Debug, Clone)]
pub enum Element {
    Rectangle(ShapeElement),
    Ellipse(ShapeElement),
    Diamond(ShapeElement),
    Arrow(LinearElement),
    Line(LinearElement),
    Text(TextElement),
    Freedraw(FreedrawElement),
    Image(ImageElement),
    Frame(FrameElement),
    MagicFrame(FrameElement),
    Embeddable(UnsupportedElement),
    Iframe(UnsupportedElement),
    Unknown {
        element_type: String,
        raw: serde_json::Value,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct UnsupportedElement {
    #[serde(flatten)]
    pub base: BaseElement,

    #[serde(flatten)]
    pub raw: HashMap<String, serde_json::Value>,
}
```

### 7.7 Normalization

```rust
pub struct Scene {
    pub elements: Vec<NormalizedElement>,
    pub id_map: HashMap<String, usize>,
    pub frame_children: HashMap<String, Vec<String>>,
    pub bound_texts: HashMap<String, Vec<String>>,
    pub bound_arrows: HashMap<String, Vec<String>>,
    pub background_color: Color,
    pub content_bounds: Rect,
    pub export_bounds: Rect,
    pub warnings: Vec<SceneWarning>,
}

pub struct NormalizedElement {
    pub element: Element,
    pub original_order: usize,
    pub render_order: usize,
    pub abs_points: Option<Vec<Point>>,
    pub bounds: Rect,
    pub rotated_bounds: Rect,
    pub container_id: Option<String>,
    pub frame_id: Option<String>,
}
```

Normalization steps:

1. Filter `is_deleted == true`.
2. Preserve original array order.
3. Compute z-order:
   - If all visible elements have valid fractional indexes, use Excalidraw-compatible fractional ordering.
   - If indexes are missing/invalid, use stable original order fallback.
   - Never use naïve string sort unless verified compatible with Excalidraw fractional indexes.
4. Convert linear relative points to absolute points.
5. Build `id → element` lookup.
6. Build `containerId → text elements` lookup.
7. Build `boundElements` lookup for text and arrows.
8. Build `frameId → child elements` lookup.
9. Compute element bounds.
10. Compute scene bounds.

### 7.8 Bounds Rules

Bounds calculation must include:

- `x`, `y`, `width`, `height`
- linear element point arrays
- rotation around element center
- stroke width expansion
- roughness expansion safety margin
- arrowhead extents
- text measured bounds
- frame labels
- image visible crop bounds
- unsupported placeholder bounds

`content_bounds` means exact content coverage.  
`export_bounds` means content bounds plus render padding.

### 7.9 Validation

```rust
pub struct ValidationLimits {
    pub max_elements: usize,
    pub max_element_size_bytes: usize,
    pub max_payload_size_bytes: usize,
    pub max_text_length: usize,
    pub max_points_per_element: usize,
    pub max_files: usize,
    pub max_file_size_bytes: usize,
    pub max_image_data_url_bytes: usize,
}

pub struct ValidationReport {
    pub valid: bool,
    pub element_count: usize,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}
```

Validation policy:

- Fatal errors for non-JSON, invalid top-level payload, or hard limit violations.
- Warnings for unknown element types, unsupported element types, missing images, invalid colors, invalid indexes, or malformed optional fields.
- Validation should be available independently from rendering.

---

## 8. Crate: excalidraw-render

### 8.1 Responsibility

`excalidraw-render` owns:

- Scene → SVG rendering.
- Scene → PNG rendering through SVG + `resvg`.
- Text layout and measurement.
- Font registration.
- Rough path generation through `rough-rs`.
- Image embedding/cropping/flipping.
- Frame clipping.
- Unsupported placeholders.
- Render warnings.

### 8.2 Public API

```rust
pub struct RenderOptions {
    pub scale: f64,
    pub padding: f64,
    pub background: BackgroundMode,
    pub quality: RenderQuality,
    pub unsupported: UnsupportedElementMode,
    pub image_policy: ImagePolicy,
    pub text_policy: TextPolicy,
}

pub enum BackgroundMode {
    FromFile,
    Transparent,
    Override(Color),
}

pub enum RenderQuality {
    Full,
    FastSvg,
    Clean,
}

pub enum UnsupportedElementMode {
    Placeholder,
    Skip,
    Error,
}

pub enum ImagePolicy {
    EmbedDataUrls,
    PlaceholderOnly,
    ErrorOnMissing,
}

pub enum TextPolicy {
    SvgText,
    PathsFuture,
}

pub struct RenderOutput<T> {
    pub output: T,
    pub warnings: Vec<RenderWarning>,
}

pub fn render_svg(scene: &Scene, opts: &RenderOptions) -> Result<RenderOutput<String>, RenderError>;

pub fn render_png(scene: &Scene, opts: &RenderOptions) -> Result<RenderOutput<Vec<u8>>, RenderError>;
```

`dark_mode` does not belong in core render options. TUI may apply its own background or theme wrapper, but the renderer should not silently invert source colors.

### 8.3 rough-rs Integration

`rough-rs` is a first-class dependency and may be co-developed with this project.

Dependency modes:

```toml
# During active co-development:
rough-rs = { path = "../rough-rs" }

# For release:
rough-rs = "x.y.z"
```

Integration contract:

- All hand-drawn geometry goes through `rough-rs` in `RenderQuality::Full` and `RenderQuality::FastSvg`.
- `rough-rs` must expose deterministic output for a given seed.
- Renderer must pass Excalidraw-compatible roughness, bowing, stroke, fill, fill style, stroke width, and seed.
- If Excalidraw parity reveals missing rough behavior, update `rough-rs` rather than hacking around it in `excalidraw-render`, unless the behavior is Excalidraw-specific.

Required `rough-rs` primitives:

- rectangle
- ellipse
- polygon
- linear path
- line
- curve/path support for freedraw or future shape fidelity
- hachure/cross-hatch/dots/dashed/zigzag-line fills
- seeded deterministic randomness

### 8.4 Element Renderers

- **Rectangle**
  - rough rectangle in Full/FastSvg
  - clean `<rect>` in Clean
  - roundness types 1/2/3
  - fill styles
- **Ellipse**
  - rough ellipse or clean ellipse
- **Diamond**
  - polygon with top/right/bottom/left midpoints
- **Line/Arrow**
  - linear path based on stored points
  - no re-routing for elbow arrows
  - explicit arrowhead geometry by default
  - bindings used for metadata/description; stored points remain source of rendered geometry
- **Text**
  - SVG `<text>` output in v0.1
  - measured bounds
  - multi-line text
  - line height
  - horizontal and vertical alignment
  - bound text layout
- **Freedraw**
  - perfect-freehand-compatible stroke outline
  - pressure support
  - simulatePressure support
- **Image**
  - data URL passthrough
  - crop through clipPath and transform
  - scale flips
  - placeholder on missing data
- **Frame**
  - frame border and label
  - optional clipping for children
  - collapsed placeholder if needed
- **Unsupported**
  - placeholder rectangle with label and warning

### 8.5 Arrowheads

Arrowheads should be rendered as explicit SVG nodes in Full/FastSvg mode.

SVG markers are allowed only for future optimization or Clean mode if they remain visually correct.

Supported arrowhead types:

- `arrow`
- `triangle`
- `triangle_outline`
- `bar`
- `dot`
- `circle`
- `diamond`
- `crowfoot`

Arrowhead geometry follows:

- endpoint tangent direction
- parent stroke color
- parent opacity
- parent stroke width
- scale proportional to stroke width
- no clipping by line path

### 8.6 Text Rendering

Text is a core fidelity risk.

Requirements:

- Preserve `text` and `originalText`.
- Support `fontSize`, `fontFamily`, `lineHeight`.
- Support `textAlign`: left, center, right.
- Support `verticalAlign`: top, middle, bottom.
- Use shared font registration for SVG and PNG paths.
- SVG output may use `<text>`.
- PNG rasterization must load the same bundled fonts.
- Text bounds must be measured, not guessed.
- Bound text must use container geometry and alignment rules.
- Arrow labels are text elements with container relation to arrow.

Font family mapping is versioned compatibility, not a permanent assumption.

```rust
pub fn font_family_css(family: u32) -> &'static str {
    match family {
        1 => "Virgil, Excalifont, cursive",
        2 => "Helvetica, Arial, sans-serif",
        3 => "Cascadia Code, Courier New, monospace",
        5 => "Excalifont, cursive",
        6 => "Nunito, sans-serif",
        7 => "Lilita One, cursive",
        8 => "Comic Shanns, Comic Sans MS, cursive",
        _ => "Excalifont, cursive",
    }
}
```

### 8.7 Frame Behavior

Frame behavior:

- Frame itself renders as border + optional name label.
- Elements with `frameId` remain normal scene elements.
- If `frame.clip == true`, children inside that frame are clipped to frame bounds.
- If `frame.isCollapsed == true`, renderer may render a collapsed placeholder, but must not panic.
- Nested frames must not cause recursive render loops.

### 8.8 Image Behavior

Image rendering:

- If `fileId` exists and `files[fileId].dataURL` exists, render image.
- If `fileId` missing or dataURL missing, render placeholder with warning.
- Support crop through clipPath + adjusted image transform.
- Support `scale`: `[1, 1]`, `[-1, 1]`, `[1, -1]`, `[-1, -1]`.
- Validate mime type but do not reject unknown image types unless `resvg`/SVG path cannot handle them.
- Do not decode image bytes for SVG output unless needed for validation; pass data URL through.

### 8.9 SVG Builder

```rust
pub enum SvgNode {
    Path {
        d: String,
        stroke: Paint,
        stroke_width: f64,
        fill: Paint,
        opacity: f64,
        stroke_dasharray: Option<String>,
    },
    Text {
        x: f64,
        y: f64,
        runs: Vec<TextRun>,
        font_size: f64,
        font_family: String,
        fill: Paint,
        text_anchor: String,
        dominant_baseline: Option<String>,
        opacity: f64,
    },
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        stroke: Paint,
        stroke_width: f64,
        fill: Paint,
        rx: f64,
        ry: f64,
        opacity: f64,
        stroke_dasharray: Option<String>,
    },
    Image {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        href: String,
        clip_path: Option<String>,
        transform: Option<String>,
        opacity: f64,
    },
    Group {
        id: Option<String>,
        transform: Option<String>,
        clip_path: Option<String>,
        opacity: Option<f64>,
        children: Vec<SvgNode>,
    },
    Def(SvgDef),
}
```

### 8.10 Rotation

Rotation uses element center unless a specific Excalidraw behavior requires otherwise.

```rust
pub fn rotation_transform(base: &BaseElement) -> Option<String> {
    if base.angle.abs() < 1e-10 {
        return None;
    }

    let cx = base.x + base.width / 2.0;
    let cy = base.y + base.height / 2.0;
    let deg = base.angle.to_degrees();

    Some(format!("rotate({deg:.3} {cx:.3} {cy:.3})"))
}
```

---

## 9. Crate: excalidraw-tui

Terminal viewer using `ratatui` + `ratatui-image`.

Responsibility:

- Call `excalidraw-core` + `excalidraw-render`.
- Display PNG in terminal.
- Offer pan/zoom/save interactions.
- Do not implement renderer logic.

Protocol priority:

1. Kitty
2. Sixel
3. iTerm2
4. Halfblock fallback

Key bindings:

- `q` / `Esc` — quit
- `+` / `-` — zoom
- `hjkl` / arrows — pan
- `r` — reset
- `s` — save PNG
- `e` — save SVG

WSL2 notes:

- WezTerm → Kitty protocol recommended.
- Windows Terminal with Sixel support → Sixel.
- tmux may force fallback depending on terminal configuration.

---

## 10. Crate: excalidraw-mcp

MCP server through `rmcp`, stdio transport.

Responsibility:

- Expose core/render features to AI agents.
- Avoid renderer-specific forks.
- Return warnings in all render/validate responses.

Tools:

### `render_file`

Input:

```json
{ "path": "diagram.excalidraw", "scale": 2.0 }
```

Output:

```json
{
  "png_base64": "...",
  "width": 1200,
  "height": 800,
  "warnings": []
}
```

### `to_svg`

Input:

```json
{ "path": "diagram.excalidraw" }
```

Output:

```json
{
  "svg": "<svg...>",
  "warnings": []
}
```

### `to_png`

Input:

```json
{ "path": "diagram.excalidraw", "output": "diagram.png", "scale": 2.0 }
```

Output:

```json
{
  "saved_to": "diagram.png",
  "size_bytes": 12345,
  "warnings": []
}
```

### `parse_elements`

Input:

```json
{ "path": "diagram.excalidraw" }
```

Output:

```json
{
  "element_count": 12,
  "elements": [],
  "bounds": {},
  "warnings": []
}
```

### `describe_scene`

Parse-only summary for AI planning.

Output includes:

- element type counts
- text labels
- approximate positions
- arrow connections
- frames
- warnings

### `validate`

Input:

```json
{ "path": "diagram.excalidraw" }
```

or:

```json
{ "json": "{...}" }
```

Output:

```json
{
  "valid": true,
  "element_count": 12,
  "errors": [],
  "warnings": []
}
```

---

## 11. Crate: excalidraw-cli

Commands:

```bash
excd view <file>
excd convert <file> [output]
excd to-svg <file> [output]
excd to-png <file> [output]
excd info <file>
excd describe <file>
excd validate <file>
excd serve
```

Options:

```bash
--scale <N>
--padding <PX>
--background from-file|transparent|#RRGGBB
--quality full|fast-svg|clean
--unsupported placeholder|skip|error
--warnings json|text|silent
```

Behavior:

- `to-svg` defaults to stdout if output is omitted.
- `to-png` defaults to `<input>.png` if output is omitted.
- `convert` detects format from output extension.
- `info` is parse-only.
- `describe` is parse-only and useful for AI agents.
- `serve` starts MCP server on stdio.

---

## 12. Element Type Specifications

| Element | v0.1 | Behavior |
|---|---:|---|
| `rectangle` | ✅ | rough-rs rect, fill styles, roundness |
| `ellipse` | ✅ | rough-rs ellipse |
| `diamond` | ✅ | rough-rs polygon |
| `line` | ✅ | stored points, dash styles |
| `arrow` | ✅ | stored points, explicit arrowheads |
| `text` | ✅ | SVG text, font mapping, alignment |
| `freedraw` | ✅ | perfect-freehand-compatible outline |
| `image` | ✅ | data URL, crop, flip, placeholder on missing |
| `frame` | ✅ | border, label, optional clipping |
| `magicframe` | ✅ | render as frame-like placeholder or frame equivalent |
| `embeddable` | Placeholder | browser-dependent |
| `iframe` | Placeholder | browser-dependent |
| Unknown type | Placeholder | preserve raw JSON and warn |

Additional features:

| Feature | v0.1 | Behavior |
|---|---:|---|
| Bound text | ✅ | render with container geometry |
| Arrow label text | ✅ | render as text bound to arrow |
| Elbow arrows | ✅ | use stored points, no rerouting |
| Frame clipping | ✅ | clip children when frame clip is true |
| Z-order | ✅ | fractional index when valid, stable fallback |
| Rotation | ✅ | group transform around element center |
| Opacity | ✅ | apply consistently to child nodes |

---

## 13. Rendering Pipeline Per Element

```text
For each normalized element in render order:

1. Skip deleted elements; they should not be in Scene.
2. If element is text with container_id:
   - skip standalone render if it will be rendered by container
   - render standalone only if container is missing or policy says fallback
3. Resolve style:
   - stroke
   - fill
   - opacity
   - stroke width
   - stroke style
   - roughness
   - seed
4. Dispatch to renderer by element type.
5. Render bound texts for container elements.
6. Render arrow labels.
7. Apply frame clipping group if required.
8. Apply rotation transform.
9. Apply opacity.
10. Append SVG nodes.
11. Collect warnings, never silently drop important failures.
```

---

## 14. SVG Document Assembly

Example shape:

```xml
<svg xmlns="http://www.w3.org/2000/svg"
     width="1200"
     height="800"
     viewBox="0 0 1200 800">
  <defs>
    <style>
      @font-face {
        font-family: 'Excalifont';
        src: url('data:font/woff2;base64,...') format('woff2');
      }
    </style>
    <clipPath id="frame-clip-abc">
      <rect x="0" y="0" width="400" height="300"/>
    </clipPath>
  </defs>

  <rect width="100%" height="100%" fill="#ffffff"/>

  <g id="element-abc" transform="rotate(0 100 100)">
    <path d="M10 10 C..." stroke="#1e1e1e" stroke-width="2" fill="#a5d8ff"/>
  </g>
</svg>
```

SVG rules:

- All IDs must be escaped/sanitized for XML.
- Text must be XML-escaped.
- ClipPath IDs must be unique.
- Def references must never point to missing IDs.
- SVG should parse with `usvg`.
- SVG should remain self-contained when images/fonts are embedded.

---

## 15. Terminal Display Strategy

TUI renders PNG through the same core rendering pipeline.

Detection priority:

1. Kitty
2. Sixel
3. iTerm2
4. Halfblock

TUI-specific render options:

- background may be transparent or theme-adjusted
- zoom/pan are viewer transforms, not renderer transforms
- dark-mode display must not mutate source scene colors unless explicitly requested

---

## 16. MCP Tool Definitions

The MCP server registers:

- `render_file`
- `to_svg`
- `to_png`
- `parse_elements`
- `describe_scene`
- `validate`

Future v0.2 adds:

- `mermaid_to_excalidraw`

All tools should include:

- `warnings`
- structured errors
- stable JSON response fields
- no panics from malformed inputs

---

## 17. Dependencies

### 17.1 Workspace Dependencies

```toml
[workspace]
resolver = "2"
members = [
    "crates/excalidraw-core",
    "crates/excalidraw-render",
    "crates/excalidraw-tui",
    "crates/excalidraw-mcp",
    "crates/excalidraw-cli",
]

[workspace.dependencies]
# Core
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"

# Rendering
resvg = "=0.47.0"
usvg = "=0.47.0"
tiny-skia = "0.11"
fontdb = "0.23"
base64 = "0.22"

# Hand-drawn rendering: user-owned crate, co-developed with this project.
# During active local development:
rough-rs = { path = "../rough-rs" }
# For release, switch to the crates.io version:
# rough-rs = "x.y.z"

# Text measurement
unicode-width = "0.2"

# TUI
ratatui = "0.29"
ratatui-image = "8"
crossterm = "0.29"
image = "0.25"

# MCP
rmcp = { version = "=1.7.0", features = ["server", "transport-io"] }
tokio = { version = "1", features = ["full"] }

# CLI
clap = { version = "4", features = ["derive", "color"] }

# Utils
anyhow = "1"
```

### 17.2 Dependency Policy

- Pin pre-1.0 crates when their public API is part of this project’s implementation boundary.
- Keep `rough-rs` as the hand-drawn rendering source of truth.
- If `excalidraw-native` needs rough behavior missing from `rough-rs`, improve `rough-rs`.
- Do not vendor rough behavior directly into `excalidraw-render` unless it is Excalidraw-specific glue.
- Avoid dependency drift until tests and visual baselines are stable.

---

## 18. Testing Strategy & Benchmarks

### 18.1 Parse Compatibility Tests

Use real `.excalidraw` files from:

- official Excalidraw export
- generated files
- old files
- files with missing optional fields
- files with unknown fields
- files with images
- files with frames
- files with bound text
- files with arrows
- files with unsupported elements

### 18.2 Structural Snapshot Tests

Snapshot:

- parsed element count
- element type list
- z-order
- normalized bounds
- frame child mapping
- bound text mapping
- arrow binding mapping
- warnings

### 18.3 SVG Validity Tests

Every rendered SVG must:

- parse through `usvg`
- have valid XML
- have no missing defs references
- have valid path data
- escape text correctly
- embed images/fonts correctly when configured

### 18.4 Visual Regression Tests

Pipeline:

```text
fixture.excalidraw
  → render SVG
  → rasterize PNG
  → compare to golden PNG
```

Visual comparison policy:

- tolerate small antialiasing differences
- separate baselines per render quality
- preserve accepted deviations in docs
- use official Excalidraw export as oracle for selected fixtures

### 18.5 Benchmark Tests

Benchmarks:

- parse simple/complex
- normalize simple/complex
- SVG render simple/complex/large
- PNG render simple/complex
- CLI end-to-end
- MCP end-to-end

CI:

- unit tests and parse compatibility always run
- visual regression can run on main/PR with stored golden artifacts
- benchmark threshold can be soft warning at first, hard gate later

---

## 19. Implementation Phases

Time is not the source of truth. Correctness is. Phases define dependency order, not deadline.

### Phase 0 — Scaffold

- [ ] Workspace setup
- [ ] Crate skeletons
- [ ] CI: fmt, clippy, test
- [ ] Fixture directory
- [ ] Golden output directory
- [ ] README skeleton

### Phase 1 — Core Parser

- [ ] Lenient serde models
- [ ] Unknown raw JSON preservation
- [ ] Default functions
- [ ] Color parser
- [ ] Validation limits
- [ ] Parse compatibility tests

### Phase 2 — Normalization

- [ ] Deleted filtering
- [ ] Stable z-order
- [ ] Fractional index compatibility
- [ ] ID map
- [ ] Bound text map
- [ ] Frame child map
- [ ] Linear absolute points
- [ ] Bounds with rotation/stroke/text/arrowhead safety

### Phase 3 — SVG Foundation

- [ ] SVG node model
- [ ] XML escaping
- [ ] Defs and clipPath support
- [ ] Style serialization
- [ ] SVG validity smoke test

### Phase 4 — rough-rs Integration

- [ ] Wire `rough-rs`
- [ ] Render rectangle/ellipse/diamond
- [ ] Fill styles
- [ ] Deterministic seed tests
- [ ] Identify missing rough-rs APIs and update `rough-rs` as needed

### Phase 5 — Text

- [ ] Font mapping
- [ ] Font embedding
- [ ] Text measurement
- [ ] Multi-line layout
- [ ] Horizontal/vertical align
- [ ] Bound text
- [ ] Arrow labels

### Phase 6 — Linear Elements

- [ ] Lines
- [ ] Arrows
- [ ] Multi-point paths
- [ ] Dash/dot styles
- [ ] Explicit arrowhead geometry
- [ ] Bound arrows metadata
- [ ] Elbow arrows using stored points

### Phase 7 — Images, Frames, Freedraw

- [ ] Image data URL render
- [ ] Missing image placeholder
- [ ] Crop
- [ ] Flip
- [ ] Frame border and label
- [ ] Frame clipping
- [ ] Collapsed frame placeholder
- [ ] Freedraw stroke outline
- [ ] Pressure/simulatePressure

### Phase 8 — PNG

- [ ] `resvg` integration
- [ ] Fontdb setup
- [ ] PNG scaling
- [ ] Transparent background
- [ ] PNG tests

### Phase 9 — CLI

- [ ] `to-svg`
- [ ] `to-png`
- [ ] `convert`
- [ ] `info`
- [ ] `describe`
- [ ] `validate`
- [ ] output warnings

### Phase 10 — TUI

- [ ] Terminal protocol detection
- [ ] Image display
- [ ] Pan/zoom
- [ ] Save PNG/SVG
- [ ] WSL2/tmux behavior docs

### Phase 11 — MCP

- [ ] rmcp server setup
- [ ] stdio transport
- [ ] render/convert/parse/describe/validate tools
- [ ] warnings/errors schema
- [ ] integration test with MCP client

### Phase 12 — Fidelity Pass

- [ ] Compare against official Excalidraw exports
- [ ] Document deviations
- [ ] Patch renderer
- [ ] Patch `rough-rs` if rough output is the blocker
- [ ] Add fixtures for each bug found

### Phase 13 — Release

- [ ] README usage
- [ ] API docs
- [ ] CHANGELOG
- [ ] crates.io publish order:
  1. `excalidraw-core`
  2. `excalidraw-render`
  3. `excalidraw-tui`
  4. `excalidraw-mcp`
  5. `excalidraw-cli`
- [ ] GitHub release binaries

---

## 20. Design Decisions & Rationale

### DEC-001: Workspace root has no `src/`

The root is only a workspace. The binary is `excalidraw-cli`.

### DEC-002: `rough-rs` is the rough rendering source of truth

This project uses your `rough-rs` crate. During development, `excalidraw-native` may expose missing requirements in `rough-rs`. Those should be fixed in `rough-rs` when generally useful.

### DEC-003: Renderer is correctness-first

Runtime speed matters, but correctness wins. Performance optimizations must not fork behavior into hidden incompatible paths.

### DEC-004: Unknown elements preserve raw JSON

Future Excalidraw versions can add fields and types. Preserving raw JSON makes debugging and future support possible.

### DEC-005: Unsupported elements render placeholders by default

Placeholder is better than panic or invisible content. Users can choose `skip` or `error`.

### DEC-006: Explicit arrowhead geometry

Explicit geometry gives better control than SVG markers for bar, dot, circle, diamond, crowfoot, opacity, stroke width, and bounds.

### DEC-007: No rerouting of arrows

`.excalidraw` stores points. Renderer uses stored points. Rerouting belongs to an editor, not renderer.

### DEC-008: Text remains SVG text in v0.1

Text stays selectable and scalable. Future path conversion may be added for exact visual parity.

### DEC-009: Frame clipping is real renderer behavior

Frames are not just decorative. If `clip` is true, child elements should clip.

### DEC-010: CLI/TUI/MCP do not duplicate renderer logic

All output paths use core/render APIs.

### DEC-011: Visual regression is required

Valid SVG is not enough. A renderer must be visually tested.

### DEC-012: v0.2 Mermaid must not disturb v0.1

Mermaid conversion is a new crate and consumer of existing renderer. It does not rewrite core rendering.

---

## 21. v0.2 Roadmap — Mermaid → Excalidraw

Status: planned after v0.1 renderer stabilizes.

Goal:

```text
Mermaid text
  → merman-core parse
  → merman-render layout
  → excalidraw-mermaid convert layout to Excalidraw elements
  → excalidraw-render SVG/PNG
```

### 21.1 New Crate

```text
crates/
└── excalidraw-mermaid/
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        ├── engine.rs
        ├── error.rs
        ├── style.rs
        ├── fallback.rs
        └── convert/
            ├── mod.rs
            ├── flowchart.rs
            ├── sequence.rs
            ├── class.rs
            ├── state.rs
            └── er.rs
```

### 21.2 Dependency Policy

```toml
merman-core = "=0.4.0"
merman-render = "=0.4.0"
```

Pin exact versions because layout structs are the API boundary.

Do not depend directly on internal layout crates unless absolutely required.

### 21.3 Public API

```rust
pub struct MermaidConvertOptions {
    pub font_size: f64,
    pub flowchart_curve: FlowchartCurve,
    pub max_edges: usize,
    pub max_text_size: usize,
    pub on_unsupported: OnUnsupported,
}

pub enum FlowchartCurve {
    Linear,
    Basis,
}

pub enum OnUnsupported {
    Error,
    Placeholder,
}

pub fn parse_to_excalidraw(
    mermaid_text: &str,
    opts: &MermaidConvertOptions,
) -> Result<Vec<Element>, MermaidConvertError>;

pub fn parse_to_excalidraw_file(
    mermaid_text: &str,
    opts: &MermaidConvertOptions,
) -> Result<ExcalidrawFile, MermaidConvertError>;
```

### 21.4 Supported Diagram Types

Tier 1 in v0.2:

| Mermaid Type | Output |
|---|---|
| Flowchart | nodes, arrows, clusters as frames |
| Sequence | actors, lifelines, messages, activations, notes |
| Class | class boxes, members, relation arrows |
| State | states, start/end, transitions, composite frames |
| ER | entity boxes, crowfoot arrows |

Tier 2 placeholder:

- Gantt
- Pie
- Mindmap
- Sankey
- GitGraph
- Timeline
- Journey
- Kanban
- C4
- Block
- Radar
- Treemap
- XYChart
- Architecture
- Requirement
- QuadrantChart

### 21.5 Flowchart Mapping

| Mermaid shape | Excalidraw |
|---|---|
| rectangle | rectangle |
| rounded | rectangle + roundness |
| circle | ellipse |
| rhombus | diamond |
| subgraph | frame |
| unknown shape | rectangle best effort |

Edges:

- routed points → Excalidraw arrow points
- markers → arrowheads
- labels → text bound to edge
- dashed/dotted → stroke style

### 21.6 Sequence Mapping

- Actor → rectangle
- Lifeline → dashed vertical line
- Message → arrow + label
- Activation → thin rectangle
- Note → rectangle + text
- loop/alt/opt → frame

### 21.7 Class Mapping

- Class → rectangle with text block
- Extension `<|--` → triangle outline
- Composition `*--` → diamond
- Aggregation `o--` → diamond outline best effort
- Dependency `..>` → dotted arrow
- Namespace → frame

### 21.8 State Mapping

- State → rounded rectangle
- Start → filled ellipse
- End → concentric ellipse
- Composite state → frame
- Transition → arrow
- Choice → diamond
- Fork/join → thick bar line

### 21.9 ER Mapping

- Entity → rectangle + attribute text
- Relationship → arrow
- Cardinality → crowfoot arrowheads

### 21.10 MCP Tool Addition

Tool: `mermaid_to_excalidraw`

Input:

```json
{
  "mermaid": "graph TD\nA-->B",
  "font_size": 20.0,
  "flowchart_curve": "linear",
  "render": false
}
```

Output:

```json
{
  "excalidraw": "{...}",
  "element_count": 4,
  "diagram_type": "flowchart",
  "warnings": [],
  "png_base64": null
}
```

### 21.11 CLI Additions

```bash
excd mermaid-to-excalidraw <input.mmd> [output.excalidraw]
excd mermaid <input.mmd> [output.{svg,png,excalidraw}]
cat flow.mmd | excd mermaid - --format svg > flow.svg
```

### 21.12 v0.2 Gate

Do not begin full v0.2 conversion until a spike proves:

- `merman-core` parse works with target fixtures.
- `merman-render` layout works with target fixtures.
- text measurer implementation compiles.
- at least one flowchart converts into valid `.excalidraw`.
- generated `.excalidraw` renders through existing v0.1 renderer.
- v0.1 renderer API does not need breaking changes.

### 21.13 v0.2 Tests

Fixtures:

```text
flowchart_simple.mmd
flowchart_subgraphs.mmd
flowchart_styled.mmd
flowchart_shapes.mmd
flowchart_50nodes.mmd
sequence_simple.mmd
sequence_loops.mmd
sequence_activations.mmd
sequence_notes.mmd
class_inheritance.mmd
class_namespaces.mmd
state_simple.mmd
state_composite.mmd
state_choice.mmd
er_basic.mmd
er_cardinalities.mmd
unsupported_gantt.mmd
```

Tests:

- Mermaid parse
- layout extraction
- conversion snapshot
- render SVG validity
- visual regression after render
- unsupported fallback behavior

### 21.14 v0.2 Risks

| Risk | Mitigation |
|---|---|
| merman API churn | exact version pins |
| text measurer mismatch | spike first |
| layout structs underdocumented | inspect source and snapshot layouts |
| visual mismatch with Excalidraw web Mermaid import | structural equivalence is target |
| unsupported diagrams | placeholder mode |

---

## Final Rule

When in doubt, make the renderer more truthful to `.excalidraw` semantics, not faster or simpler.

`rough-rs` and `excalidraw-native` are allowed to evolve together. If renderer fidelity exposes a general rough rendering gap, fix it in `rough-rs`; if it is Excalidraw-specific, keep the glue in `excalidraw-render`.
