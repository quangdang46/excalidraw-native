# Documented fidelity deviations

`excalidraw-native` is correctness-first, not pixel-perfect. The renderer
aims for **structural correctness** (F0/F1) and **visual equivalence** (F2)
against Excalidraw's own web export. This document captures the known
F3 deviations as of the v0.1 → v0.2-alpha cut so contributors and
downstream agents can reason about diff output instead of treating each
diff as a regression.

The list is intentionally short — only behaviours we have validated via
the v0.1 fixtures (`tests/fixtures/*.excalidraw`) and the Tier 1 Mermaid
fixtures (`tests/fixtures/mermaid/*.mmd`) appear here. When a deviation
is fixed, remove the entry rather than expanding it.

## Renderer

### R1 — Rough hachure spacing

- **Symptom:** `fillStyle: hachure` / `cross-hatch` regions render with a
  slightly tighter stroke pitch than Excalidraw web for elements wider
  than ~400px.
- **Cause:** `rough-rs 0.1` follows the canonical `roughjs` hachure
  algorithm but uses a deterministic seed derived from the element id
  rather than the canvas-pixel grid `roughjs` uses on the web.
- **Workaround:** Use `quality = clean` (`excd to-svg --quality clean`)
  to drop the rough overlay entirely; `quality = fast-svg` keeps the
  rough geometry but skips embedded font data.
- **Tracking:** rough-rs follow-up. No structural impact — fills cover
  the same region and respect the same opacity.

### R2 — Embedded font pixel metrics

- **Symptom:** Standalone and bound text elements are typeset using
  fontdue glyph metrics. For the bundled *Excalifont* / *Cascadia* /
  *Helvetica* fallbacks the advance widths can be off by up to ±1px per
  glyph relative to a Chromium canvas render.
- **Cause:** Excalidraw web measures text through the DOM
  (`measureText`); `excalidraw-native` measures through fontdue's TTF
  metrics. Both produce stable output, but they disagree on subpixel
  rounding for long ligatures.
- **Workaround:** Bound text is re-aligned to the container after
  measurement so containers never overflow. The cumulative error on a
  single line is bounded by `len(text) * 1px`.
- **Tracking:** Acceptable for v0.1. Pixel-parity (F4) will require an
  alternative shaping pipeline (e.g. `cosmic-text` or `swash`).

### R3 — Image cache invalidation

- **Symptom:** When the same `fileId` appears with different
  `dataURL` payloads in a single `.excalidraw` document, the first
  occurrence wins for the SVG `<image>` `xlink:href`. Excalidraw web
  treats every occurrence independently.
- **Cause:** The render-time image cache keys on `fileId` only.
- **Workaround:** Authoring tools should not reuse `fileId`s across
  payload variants. The CLI emits a `duplicate-file-id` warning in
  `--warnings json`/`text` modes.
- **Tracking:** Low priority — no observed real-world Excalidraw export
  exhibits this case.

### R4 — `magicframe` and embeddable/iframe elements

- **Symptom:** `magicframe` renders as a regular frame (border + label),
  `embeddable` and `iframe` render as the configured
  `--unsupported placeholder` rectangle.
- **Cause:** Explicitly out of v0.1 scope per the PLAN.
- **Workaround:** `--unsupported skip` removes them; `--unsupported
  error` aborts the render.
- **Tracking:** v0.3+ candidate.

### R5 — Arrow endpoint inset

- **Symptom:** Arrow shafts are inset by `arrowhead_length` (configurable
  per arrowhead kind) so the tip of an explicit-geometry arrowhead
  meets the line cleanly. Excalidraw web inlines arrowheads with the
  canvas stroke and does not inset.
- **Cause:** Full-quality mode emits arrowheads as explicit SVG
  geometry, not SVG markers. The inset prevents a visible "ridge"
  where the shaft overshoots the head fill.
- **Workaround:** `--quality clean` falls back to SVG `marker-end`
  semantics; `fast-svg` keeps the explicit geometry but drops
  per-element font embedding.
- **Tracking:** Documented behaviour; not a bug.

## Mermaid → Excalidraw (`excalidraw-mermaid`)

### M1 — Flowchart edge routing

- **Symptom:** `merman-render` emits straight polylines for flowchart
  edges. Excalidraw's official `parseMermaid` helper uses
  `dagre-d3`-style cubic curves.
- **Cause:** `merman-render 0.4.0` only ships linear/basis polyline
  output; spline edges are on the merman roadmap.
- **Workaround:** Pass `--curve basis` to use merman's basis-spline
  sampling. The resulting arrows are still polylines but follow a
  smoothed control polygon.
- **Tracking:** Will revisit when merman ships spline edges.

### M2 — Sequence-diagram activations

- **Symptom:** Sequence activations (the slim activation bars overlaid
  on lifelines) are not yet emitted as separate rectangles. Lifelines
  are full-height rectangles and messages connect their centres.
- **Cause:** Activation geometry is present in the merman semantic
  model but not yet in the layout output; the converter currently
  ignores it.
- **Workaround:** Visually adequate for Tier 1. Activations will land
  in a follow-up v0.2.x release.

### M3 — Class member visibility glyphs

- **Symptom:** Class members render as `+method()`, `-field: T`, etc.
  using the literal visibility prefix. Excalidraw web styles them as
  spans with colour-coded glyphs.
- **Cause:** Excalidraw text elements are single-style; multi-coloured
  spans would require splitting members into multiple text elements.
- **Workaround:** Plain-text output keeps copy-paste round-trips clean.

### M4 — ER cardinality glyphs

- **Symptom:** Cardinality markers map to the Excalidraw arrowhead set
  (`circle`, `bar`, `crowfoot`, `arrow`) which is a superset of the
  Mermaid theme but renders with a slightly different visual
  weighting than the SVG glyphs Mermaid normally uses.
- **Cause:** Reusing native Excalidraw arrowheads keeps the output
  editable in Excalidraw web; bespoke ER glyphs would require custom
  paths.
- **Workaround:** Documented in the README *Mermaid* table.

### M5 — Unsupported diagram fallback

- **Symptom:** Diagram types outside Tier 1 (gantt, pie, mindmap,
  journey, requirement, timeline, gitGraph, sankey, quadrant chart,
  xy chart, block, c4) render as a single placeholder rectangle
  containing the raw Mermaid source.
- **Cause:** Intentional Tier 2 fallback. Set
  `OnUnsupported::Error` (CLI: `--on-unsupported error`) to fail
  loudly instead.
- **Tracking:** Tier 2 diagrams are scheduled for v0.3.

## How to file a new deviation

1. Add a `tests/fixtures/*.excalidraw` (or `*.mmd`) reproducer that the
   v0.1 renderer accepts.
2. Render through `excd to-svg` / `excd mermaid` and capture the
   output.
3. Compare against an Excalidraw web export (or the Mermaid live
   editor for Mermaid sources).
4. If the difference is structural (wrong element type, wrong z-order,
   wrong bounds), treat it as a bug and add a regression test.
5. If the difference is visual but acceptable, append a numbered
   entry to this file with: symptom, cause, workaround, and (if
   relevant) the upstream tracking issue.

Pull requests that change rendering output **must** either update the
fidelity fixtures **or** add an entry here justifying the new
behaviour.
