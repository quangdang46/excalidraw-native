# PROBLEMS.md — Known Issues & Bugs Found During Testing

## Summary

All 134 tests pass, all 12 Excalidraw fixtures render to valid SVG/PNG, and all 6 Mermaid fixtures render correctly. However, **3 critical issues** were found that make the output unusable for real-world scenarios.

---

## ✅ FIXED — Arrow connections now respect bindings

### Resolved in: [PR #?](../../pulls) — binding-aware endpoint computation, gap, and SVG metadata

`render_linear()` (`crates/excalidraw-render/src/lib.rs`) now reads `start_binding` and `end_binding`, looks up the bound shape via `scene.id_map`, computes the intersection of the arrow direction with the shape edge (rectangle, ellipse, or diamond), and shrinks the endpoint by `binding.gap`. Arrow groups now carry `id`, `data-start-binding`, and `data-end-binding` attributes.

Verified by the integration test `svg_validity_arrows_bound`: for the `arrows_bound.excalidraw` fixture (two rects at x=10..90 and x=150..230, arrow raw points (90,55)→(150,55), gap=1), the rendered line is `M91 55 L149 55` and the group is `<g id="conn" data-start-binding="src" data-end-binding="dst">`.

**Still open (lower priority):**
- **Focus handling** — `ArrowBinding.focus` is parsed but not yet used to bias the connection point along the shape edge.
- **Elbowed routing** — `elbowed: true` arrows still render as straight lines.
- **Per-element `id` on non-arrow shapes** — only arrow groups carry `id`/binding attributes today; rectangles/ellipses still render as anonymous `<g>` wrappers (this is the remaining piece of issue #3 below).

---

## ✅ FIXED — `view` command renders in non-interactive environments

### Resolved in: [PR #?](../../pulls)

`crates/excalidraw-cli/src/main.rs` now imports `std::io::IsTerminal` and the `View` subcommand checks `io::stdin().is_terminal() && io::stdout().is_terminal()` before entering the interactive TUI. When either side is not a TTY (or the user passes `--no-interactive`), it falls back to `view_file()` which renders one shot to stdout. This makes `excd view foo.excalidraw | cat` and pipelines work without `enable_raw_mode()` errors.

Additionally, `output_sixel()` previously only emitted a placeholder DCS header. It now decodes the rendered PNG and encodes it with the pure-Rust `icy_sixel` crate, falling back to halfblock if Sixel encoding fails. `detect_protocol()` is now conservative — it no longer claims Sixel support for vanilla `xterm-256color` terminals, which would otherwise produce invisible output. Color-capable terminals without explicit Sixel/Kitty/iTerm2 signals fall back to halfblock, which renders a real visible image everywhere.

---

## 🟡 PARTIALLY ADDRESSED — SVG output structure

Arrow groups now carry `id` and `data-start-binding` / `data-end-binding`. The remaining structural improvements (per-shape `id`, nested bound-text grouping, `<clipPath>` for frames) are out of scope for this fix and tracked here.

**Still open:**

1. **Per-shape element IDs** — Rectangles, ellipses, diamonds, etc. still render as anonymous `<g>` wrappers. Only arrows currently emit `id` attributes.
2. **No clipPath for frames** — Frames use `data-frame` attribute but don't apply SVG `<clipPath>` to clip children inside the frame boundary.
3. **No semantic grouping** — Bound text isn't nested inside its container group; arrows aren't grouped with their bound shapes.

---

## 🟡 Visual / Fidelity Issues

### 4. Freedraw strokes render as smooth polylines, not rough/sketchy
- **Fixture:** `tests/fixtures/freedraw.excalidraw`
- **Cause:** `rough-rs` integration is applied to shapes but not to freedraw strokes
- **Severity:** Medium — fidelity gap vs. Excalidraw web

### 5. Text uses fallback fonts, not Excalifont
- **Fixture:** `tests/fixtures/text_standalone.excalidraw`
- **Cause:** `font-family="Virgil, Excalifont, cursive"` references fonts that may not be available
- **Severity:** Low — text readable, but visual fidelity depends on system font availability

### 6. Image placeholders are plain empty boxes
- **Fixture:** `tests/fixtures/image_embed.excalidraw`
- **Cause:** No visual indicator (icon/label) for missing image data
- **Severity:** Low — functional but less user-friendly than Excalidraw web

### 7. Unsupported element placeholders are generic rectangles
- **Fixture:** `tests/fixtures/unsupported.excalidraw`
- **Cause:** No type label on placeholder rectangles
- **Severity:** Low — functional but less informative

---

## 🟠 CLI Usability Issues

### 8. Inconsistent output argument patterns across subcommands
- `to-svg` → `-o output.svg` (flag)
- `mermaid` → `input.mmd output.svg` (positional)
- `mermaid-to-excalidraw` → `-o output.ex` (flag)
- **Severity:** Low — confusing for users

---

## 🔵 Test Infrastructure Gaps

### 9. No visual regression tests
- Tests validate XML correctness and pixel dimensions but not visual appearance
- Visual bugs like missing arrow bindings would not be caught
- **Severity:** Medium

### 10. `view` command has zero test coverage
- TUI viewer cannot be tested in CI/non-interactive environments
- **Severity:** Medium

### 11. Weak arrow test assertions
- `svg_validity_arrows_basic` only checks `<path` exists — doesn't verify binding or connection
- `svg_validity_arrows_bound` asserts SVG contains `"label"` but the fixture has no label element (may match unrelated string)
- **Severity:** Medium

---

## ✅ What Works Correctly

- All 134 unit/integration tests pass
- `cargo fmt`, `cargo clippy`, `cargo test` all clean
- SVG output is valid XML for all 12 fixtures
- PNG output is valid for all 12 fixtures (correct dimensions, RGBA)
- Mermaid → SVG conversion works for all 6 Tier-1 diagram types
- Shape rendering (rectangle, ellipse, diamond) with rough-style paths
- Frame rendering with labels and borders
- Text container rendering (bound text positioning)
- Complex diagram (8 elements) renders without errors
- Large 200-element stress test renders without errors
- Release binary builds successfully
