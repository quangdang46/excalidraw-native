# PROBLEMS.md — Known Issues & Bugs Found During Testing

## Summary

All 134 tests pass, all 12 Excalidraw fixtures render to valid SVG/PNG, and all 6 Mermaid fixtures render correctly. However, **3 critical issues** were found that make the output unusable for real-world scenarios.

---

## 🔴 CRITICAL — Arrow connections are NOT rendered

### Issue: `startBinding` / `endBinding` metadata is completely ignored by renderer

The renderer treats **all arrows as standalone straight lines**. Bound arrows (arrows connected to shapes) are rendered identically to free-standing arrows — no gap, no shape-boundary intersection, no connection point computation.

**Code paths involved:**

| File | Lines | Problem |
|---|---|---|
| `crates/excalidraw-render/src/lib.rs` | 733-777 | `render_linear()` only uses `abs_points` and arrowhead style — never reads `start_binding` or `end_binding` |
| `crates/excalidraw-core/src/types.rs` | 220-224 | `start_binding` / `end_binding` fields are parsed from JSON but never consumed |
| `crates/excalidraw-core/src/normalize.rs` | 133, 437-451 | `bound_arrows` HashMap is built during normalization but never referenced by renderer |
| `crates/excalidraw-core/src/types.rs` | 227 | `elbowed` field parsed but unused — right-angle connectors render as straight lines |

**What's missing:**

1. **Binding-aware endpoint computation** — Read `startBinding.elementId` / `endBinding.elementId`, look up the bound shape in `scene.id_map`, compute where the arrow intersects the shape boundary
2. **Gap application** — `ArrowBinding.gap` should shorten the arrow so the endpoint stops before the shape edge
3. **Focus handling** — `ArrowBinding.focus` controls which side/position on the shape edge the arrow connects to
4. **Elbowed arrow routing** — When `elbowed == true`, arrows should follow right-angle paths
5. **SVG semantic attributes** — Arrow groups should include `data-start-binding`, `data-end-binding` attributes for traceability
6. **Element IDs** — Rendered SVG groups should include `id` attributes matching Excalidraw element IDs

**Example of the bug (arrows_basic.excalidraw):**
```xml
<!-- Arrow rendered as plain line, no binding info -->
<g>
  <path d="M90 55 L150 55" stroke="#000000" stroke-width="2" fill="none"/>
  <path d="M138 60.4 L150 55 M150 55 L138 49.6" stroke="#000000" stroke-width="2" fill="none"/>
</g>
<!-- No gap from shape edges, no metadata showing src→dst connection -->
```

---

## 🔴 CRITICAL — `view` command cannot render to terminal

### Issue: `excd view` crashes with unhelpful error in non-interactive environments

**Error:** `Terminal error: No such device or address (os error 6)`

**Root cause:** `run_interactive()` at `crates/excalidraw-tui/src/lib.rs:191` unconditionally calls `crossterm::terminal::enable_raw_mode()` without checking if stdin is a TTY.

**Code paths:**

| File | Lines | What happens |
|---|---|---|
| `crates/excalidraw-cli/src/main.rs` | 547-549 | CLI `View` command calls `run_interactive()` unconditionally |
| `crates/excalidraw-tui/src/lib.rs` | 184-191 | `run_interactive()` calls `enable_raw_mode()` → fails with ENXIO |
| `crates/excalidraw-tui/src/lib.rs` | 163-181 | `view_file()` — non-interactive fallback **exists but is never called** |
| `crates/excalidraw-tui/src/lib.rs` | 37-70 | `detect_protocol()` — env-var based, doesn't verify TTY availability |

**Fix options:**
- **Option A:** Check `std::io::stdin().is_terminal()` before calling `run_interactive()`. If no TTY, fall back to `view_file()` for one-shot non-interactive rendering
- **Option B:** Improve error message: `"The 'view' command requires an interactive terminal. Use 'excd to-png' for non-interactive rendering."`

---

## 🔴 CRITICAL — SVG output lacks proper structure and element relationships

### Issue: SVG output is flat — no element IDs, no grouping by relationship, no semantic attributes

**Problems:**

1. **No element IDs** — SVG groups don't have `id` attributes matching Excalidraw element IDs. Makes it impossible to link SVG elements back to source elements.

2. **No binding metadata** — No `data-*` attributes showing which arrows connect to which shapes. The SVG is semantically dead.

3. **No clipPath for frames** — Frames use `data-frame` attribute but don't apply SVG `<clipPath>` to clip children inside the frame boundary.

4. **No semantic grouping** — Elements are rendered as flat siblings inside `<g id="excalidraw-content">`. Bound text isn't nested inside its container group. Arrows aren't grouped with their bound shapes.

**Example — what current SVG looks like:**
```xml
<g id="excalidraw-content">
  <g><!-- rectangle "src" --><path .../></g>
  <g><!-- rectangle "dst" --><path .../></g>
  <g><!-- arrow --><path d="M90 55 L150 55"/></g>
  <text><!-- bound label --></text>
</g>
```

**Example — what it should look like:**
```xml
<g id="excalidraw-content">
  <g id="src" data-type="rectangle">
    <path .../>
    <g id="label1" data-type="text" data-bound-to="src">
      <text ...>label</text>
    </g>
  </g>
  <g id="dst" data-type="rectangle">
    <path .../>
  </g>
  <g id="conn" data-type="arrow" data-start-binding="src" data-end-binding="dst">
    <path d="M92 55 L148 55"/>
    <path d="M136 60.4 L148 55 M148 55 L136 49.6"/>
  </g>
</g>
```

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
