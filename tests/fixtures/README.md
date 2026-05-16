# Fixtures

Synthetic `.excalidraw` files covering all v0.1 element categories. Each fixture
is hand-written JSON designed to exercise specific renderer paths.

## Fixture Index

| File | Elements | Source | Purpose |
|---|---|---|---|
| `simple_shapes.excalidraw` | Rectangle, Ellipse, Diamond | Synthetic | Basic shape rendering with fill styles (none, solid, hachure) and colors |
| `text_standalone.excalidraw` | Text x2 | Synthetic | Standalone text: single-line left-aligned, multi-line centered with monospace font |
| `text_containers.excalidraw` | Rectangle + bound Text | Synthetic | Text bound inside a container with center/middle alignment and 8px padding |
| `arrows_basic.excalidraw` | 2 Rectangles + Arrow | Synthetic | Basic arrow connecting two shapes with end arrowhead |
| `arrows_bound.excalidraw` | 2 Rectangles + Arrow + bound Text | Synthetic | Arrow with bindings and a text label bound to the arrow |
| `freedraw.excalidraw` | Freedraw x2 | Synthetic | Freedraw strokes: one with `simulatePressure: true`, one with explicit pressures |
| `image_embed.excalidraw` | Image x3 | Synthetic | Embedded image with data URL, missing-image placeholder, and scale flip |
| `frame_clip.excalidraw` | Frame, MagicFrame, collapsed Frame, Rectangle | Synthetic | Frame border/label, magicframe, collapsed frame indicator |
| `unsupported.excalidraw` | Embeddable, Iframe, Unknown | Synthetic | Unsupported element types and unknown custom widget as placeholder |
| `complex_diagram.excalidraw` | Mixed (Rect, Ellipse, Diamond, Arrow, Line, Freedraw, Text) | Synthetic | Combined diagram with varied fill styles, opacity, dash styles |
| `large_200_elements.excalidraw` | 200 elements (Rect, Ellipse, Arrow, Text, Diamond) | Synthetic | Performance and scale testing fixture |

## Source Classification

- **Synthetic**: Hand-written JSON to exercise specific paths. Not exported from Excalidraw web.
- **Real export** (future): Exported from the official Excalidraw web editor for fidelity testing.

## Golden Output

`tests/golden/svg/` and `tests/golden/png/` hold reference render output. They are
initially empty and will be populated by the oracle/regression harness.
