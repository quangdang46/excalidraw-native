# Accepted Rendering Deviations

This document records known differences between `excalidraw-native` output and
the official Excalidraw web editor export.

## Text Rendering

- **Font metrics**: Text bounds use unicode-width estimation rather than
  browser text measurement. Line breaks and container padding will differ
  slightly from Excalidraw web.
- **Font fallback**: When system fonts don't match Excalidraw's bundled fonts
  (Excalifont, Nunito, Cascadia, Virgil), the renderer falls back to generic
  font families. This affects text width and visual appearance.

## Freedraw

- **Stroke outline**: Freedraw strokes use a simple polyline with round
  linecap/join rather than a perfect-freehand-compatible variable-width
  stroke outline. Variable pressure widths are not yet implemented.

## Image

- **Data URLs only**: Only `data:` URLs from the `files` map are supported.
  External URLs are not fetched. Missing image data produces a placeholder.
- **Crop**: Basic clipPath crop is supported. Complex transforms may differ.

## Frames

- **No clipping**: Frame child elements are not visually clipped to the frame
  bounds in v0.1. Frames render as labeled dashed borders.
- **No nesting protection**: Deeply nested frames are not specially handled.

## Rough Style

- **Roughness seed**: Rough path randomness uses `rough-rs` which may produce
  slightly different paths than Excalidraw's bundled roughjs.

## Unsupported Elements

- Embeddable, iframe, and unknown element types render as yellow placeholder
  rectangles with type labels. No interactive content is rendered.

## Missing Features (v0.1)

- No grid rendering
- No element locking visual
- No group rendering
- No library element expansion
- No Mermaid conversion
