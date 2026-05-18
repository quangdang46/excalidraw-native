//! Terminal preview consumer for excalidraw-native.
//!
//! The TUI displays render outputs from `excalidraw-render`; it does not own
//! renderer semantics. It detects the terminal image protocol (Kitty, Sixel,
//! iTerm2, halfblock fallback) and renders the diagram for interactive viewing.

use std::io::{self, Write};

use excalidraw_core::normalize_file;
use excalidraw_render::{render_png, render_svg, RenderOptions};

/// Current crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the canonical crate boundary for diagnostics and smoke tests.
#[must_use]
pub fn crate_boundary() -> &'static str {
    "terminal-viewer"
}

/// Detected terminal image protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageProtocol {
    /// Kitty terminal graphics protocol.
    Kitty,
    /// Sixel graphics protocol.
    Sixel,
    /// iTerm2 inline image protocol.
    Iterm2,
    /// Halfblock character fallback.
    Halfblock,
    /// No image support detected; show ASCII placeholder.
    Ascii,
}

impl ImageProtocol {
    /// Short label suitable for status lines.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            ImageProtocol::Kitty => "kitty",
            ImageProtocol::Sixel => "sixel",
            ImageProtocol::Iterm2 => "iterm2",
            ImageProtocol::Halfblock => "halfblock",
            ImageProtocol::Ascii => "ascii",
        }
    }

    /// Parse a protocol name from CLI/env input.
    pub fn parse(name: &str) -> Result<Self, String> {
        match name.to_ascii_lowercase().as_str() {
            "auto" => Ok(detect_protocol()),
            "kitty" => Ok(ImageProtocol::Kitty),
            "sixel" => Ok(ImageProtocol::Sixel),
            "iterm2" | "iterm" => Ok(ImageProtocol::Iterm2),
            "halfblock" | "blocks" | "block" => Ok(ImageProtocol::Halfblock),
            "ascii" | "none" => Ok(ImageProtocol::Ascii),
            other => Err(format!(
                "unknown protocol '{other}'; expected one of: auto, kitty, sixel, iterm2, halfblock, ascii"
            )),
        }
    }
}

/// Resolve the protocol to use, honoring `EXCD_VIEW_PROTOCOL` when no
/// explicit override is supplied.
#[must_use]
pub fn resolve_protocol(force: Option<ImageProtocol>) -> ImageProtocol {
    if let Some(p) = force {
        return p;
    }
    if let Ok(env) = std::env::var("EXCD_VIEW_PROTOCOL") {
        if let Ok(p) = ImageProtocol::parse(&env) {
            return p;
        }
    }
    detect_protocol()
}

/// Detect the best available terminal image protocol.
///
/// Detection is intentionally conservative: protocols that the terminal
/// only "might" support (e.g. `xterm-256color`, which generally does NOT
/// natively support Sixel) fall through to halfblock so the user still
/// sees a rendered image instead of an empty placeholder.
pub fn detect_protocol() -> ImageProtocol {
    // Kitty: hard signal only.
    if std::env::var("TERM").is_ok_and(|t| t.starts_with("xterm-kitty"))
        || std::env::var("KITTY_WINDOW_ID").is_ok()
    {
        return ImageProtocol::Kitty;
    }

    // iTerm2: hard signal only.
    if std::env::var("TERM_PROGRAM").is_ok_and(|p| p == "iTerm.app") {
        return ImageProtocol::Iterm2;
    }

    // Sixel: only when TERM itself advertises sixel. Heuristics like
    // `TERM_PROGRAM=WezTerm`, `WT_SESSION`, or `VTE_VERSION` are NOT enough:
    // those terminals only render Sixel when explicitly enabled in config,
    // and a wrong guess paints the user an invisible blob of escape codes.
    // Force with `--protocol sixel` or `EXCD_VIEW_PROTOCOL=sixel` instead.
    if std::env::var("TERM").is_ok_and(|t| t.contains("sixel")) {
        return ImageProtocol::Sixel;
    }

    // Halfblock works on any truecolor / 256-color terminal and always
    // produces a visible image, so it is the safe default.
    if std::env::var("COLORTERM").is_ok()
        || std::env::var("TERM").is_ok_and(|t| t.contains("color") || t == "screen" || t == "tmux")
    {
        return ImageProtocol::Halfblock;
    }

    ImageProtocol::Ascii
}

/// Viewer state for pan/zoom interactions.
#[derive(Debug, Clone)]
pub struct ViewState {
    pub zoom: f64,
    pub pan_x: f64,
    pub pan_y: f64,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
        }
    }
}

impl ViewState {
    /// Reset to default view.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Zoom in.
    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.25).min(8.0);
    }

    /// Zoom out.
    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 1.25).max(0.1);
    }

    /// Pan left.
    pub fn pan_left(&mut self) {
        self.pan_x -= 20.0;
    }

    /// Pan right.
    pub fn pan_right(&mut self) {
        self.pan_x += 20.0;
    }

    /// Pan up.
    pub fn pan_up(&mut self) {
        self.pan_y -= 20.0;
    }

    /// Pan down.
    pub fn pan_down(&mut self) {
        self.pan_y += 20.0;
    }

    /// Build render options from current view state.
    pub fn render_options(&self) -> RenderOptions {
        RenderOptions {
            scale: self.zoom,
            padding: 16.0,
            background: excalidraw_render::BackgroundMode::FromFile,
            quality: excalidraw_render::RenderQuality::Full,
            unsupported: excalidraw_render::UnsupportedElementMode::Placeholder,
            image_policy: excalidraw_render::ImagePolicy::Embed,
            text_policy: excalidraw_render::TextPolicy::SvgText,
        }
    }
}

/// Render an excalidraw file to PNG bytes using the given view state.
pub fn render_to_png(content: &str, state: &ViewState) -> Result<Vec<u8>, String> {
    let file = excalidraw_core::parse_str(content).map_err(|e| format!("Parse error: {e}"))?;
    let scene = normalize_file(&file);
    let opts = state.render_options();
    let output = render_png(&scene, &opts).map_err(|e| format!("Render error: {e}"))?;
    Ok(output.value)
}

/// Render an excalidraw file to SVG string.
pub fn render_to_svg(content: &str, state: &ViewState) -> Result<String, String> {
    let file = excalidraw_core::parse_str(content).map_err(|e| format!("Parse error: {e}"))?;
    let scene = normalize_file(&file);
    let opts = state.render_options();
    let output = render_svg(&scene, &opts).map_err(|e| format!("Render error: {e}"))?;
    Ok(output.value)
}

/// Run the interactive terminal viewer.
///
/// Reads the excalidraw file, renders to PNG, and outputs it to the terminal
/// using the best available protocol. For interactive mode with pan/zoom,
/// use the `view` CLI command.
pub fn view_file(content: &str) -> Result<(), String> {
    view_file_with(content, None)
}

/// Same as [`view_file`] but allows forcing a specific protocol.
pub fn view_file_with(content: &str, force: Option<ImageProtocol>) -> Result<(), String> {
    let protocol = resolve_protocol(force);
    let state = ViewState::default();
    let png_data = render_to_png(content, &state)?;
    eprintln!("excd view: protocol={}", protocol.label());

    match protocol {
        ImageProtocol::Kitty => output_kitty(&png_data),
        ImageProtocol::Sixel => output_sixel(&png_data),
        ImageProtocol::Iterm2 => output_iterm2(&png_data),
        ImageProtocol::Halfblock => output_halfblock(&png_data),
        ImageProtocol::Ascii => {
            println!("Terminal does not support image display.");
            println!("Rendered {} bytes of PNG data.", png_data.len());
            println!("Save to file with: excd to-png <file> -o output.png");
        }
    }

    Ok(())
}

/// Run interactive viewer with keyboard controls.
pub fn run_interactive(content: &str) -> Result<(), String> {
    run_interactive_with(content, None)
}

/// Same as [`run_interactive`] but allows forcing a specific protocol.
pub fn run_interactive_with(
    content: &str,
    force: Option<ImageProtocol>,
) -> Result<(), String> {
    use crossterm::event::{self, Event, KeyCode};
    use crossterm::terminal;

    let mut state = ViewState::default();
    let protocol = resolve_protocol(force);

    terminal::enable_raw_mode().map_err(|e| format!("Terminal error: {e}"))?;
    let _guard = scopeguard::guard((), |_| {
        let _ = terminal::disable_raw_mode();
    });

    let png_data = render_to_png(content, &state)?;
    display_image(&png_data, protocol, &state);

    loop {
        if event::poll(std::time::Duration::from_millis(100))
            .map_err(|e| format!("Event error: {e}"))?
        {
            if let Event::Key(key) = event::read().map_err(|e| format!("Event error: {e}"))? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        state.zoom_in();
                        let png = render_to_png(content, &state)?;
                        display_image(&png, protocol, &state);
                    }
                    KeyCode::Char('-') => {
                        state.zoom_out();
                        let png = render_to_png(content, &state)?;
                        display_image(&png, protocol, &state);
                    }
                    KeyCode::Char('h') | KeyCode::Left => {
                        state.pan_left();
                        let png = render_to_png(content, &state)?;
                        display_image(&png, protocol, &state);
                    }
                    KeyCode::Char('l') | KeyCode::Right => {
                        state.pan_right();
                        let png = render_to_png(content, &state)?;
                        display_image(&png, protocol, &state);
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        state.pan_up();
                        let png = render_to_png(content, &state)?;
                        display_image(&png, protocol, &state);
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        state.pan_down();
                        let png = render_to_png(content, &state)?;
                        display_image(&png, protocol, &state);
                    }
                    KeyCode::Char('r') => {
                        state.reset();
                        let png = render_to_png(content, &state)?;
                        display_image(&png, protocol, &state);
                    }
                    KeyCode::Char('s') => {
                        let png = render_to_png(content, &state)?;
                        let path = "excd-screenshot.png";
                        std::fs::write(path, &png).map_err(|e| format!("Save error: {e}"))?;
                        eprintln!("Saved: {path}");
                    }
                    KeyCode::Char('e') => {
                        let svg = render_to_svg(content, &state)?;
                        let path = "excd-screenshot.svg";
                        std::fs::write(path, svg).map_err(|e| format!("Save error: {e}"))?;
                        eprintln!("Saved: {path}");
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn display_image(png_data: &[u8], protocol: ImageProtocol, state: &ViewState) {
    // Clear screen and show image
    print!("\x1b[2J\x1b[H");
    match protocol {
        ImageProtocol::Kitty => output_kitty(png_data),
        ImageProtocol::Sixel => output_sixel(png_data),
        ImageProtocol::Iterm2 => output_iterm2(png_data),
        ImageProtocol::Halfblock => output_halfblock(png_data),
        ImageProtocol::Ascii => {
            println!(
                "Zoom: {:.0}% | Pan: {:.0},{:.0}",
                state.zoom * 100.0,
                state.pan_x,
                state.pan_y
            );
            println!("q: quit | +/-: zoom | h/j/k/l: pan | r: reset | s: save PNG | e: save SVG");
        }
    }
    println!(
        "\nZoom: {:.0}% | Pan: {:.0},{:.0} | protocol: {}",
        state.zoom * 100.0,
        state.pan_x,
        state.pan_y,
        protocol.label()
    );
    println!("q: quit | +/-: zoom | h/j/k/l: pan | r: reset | s: save PNG | e: save SVG");
    io::stdout().flush().ok();
}

fn output_kitty(data: &[u8]) {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(data);
    // Kitty graphics protocol: transmit+display
    let chunk_size = 4096;
    for (i, chunk) in encoded.as_bytes().chunks(chunk_size).enumerate() {
        let m = if i == 0 && chunk.len() < chunk_size {
            1
        } else {
            0
        };
        print!("\x1b_Ga=T,f=100,m={m},t=f;");
        io::stdout().write_all(chunk).ok();
        print!("\x1b\\");
    }
    io::stdout().flush().ok();
}

fn output_sixel(data: &[u8]) {
    // Decode the PNG, downscale so we don't flood the terminal, then encode
    // as SIXEL via icy_sixel. On any failure we fall back to halfblock so the
    // user still sees the rendered diagram instead of an empty escape sequence.
    let img = match image::load_from_memory(data) {
        Ok(img) => img,
        Err(error) => {
            eprintln!("Sixel decode error: {error}");
            output_halfblock(data);
            return;
        }
    };
    let img = img.resize(800, 600, image::imageops::FilterType::Triangle);
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let image = match icy_sixel::SixelImage::try_from_rgba(
        rgba.into_raw(),
        width as usize,
        height as usize,
    ) {
        Ok(image) => image,
        Err(error) => {
            eprintln!("Sixel encode error: {error}");
            output_halfblock(data);
            return;
        }
    };
    match image.encode() {
        Ok(sixel) => {
            print!("{sixel}");
            io::stdout().flush().ok();
        }
        Err(error) => {
            eprintln!("Sixel encode error: {error}");
            output_halfblock(data);
        }
    }
}

fn output_iterm2(data: &[u8]) {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(data);
    print!("\x1b]1337;File=inline=1;size={}:{}", data.len(), encoded);
    print!("\x07");
    io::stdout().flush().ok();
}

fn output_halfblock(data: &[u8]) {
    // Decode PNG and render as halfblock characters. Each terminal cell
    // shows two pixels (upper/lower) via U+2580 with foreground+background.
    // We size the image to the actual terminal viewport so we never
    // overflow the screen on small panes.
    let img = match image::load_from_memory(data) {
        Ok(img) => img,
        Err(e) => {
            println!("Image decode error: {e}");
            return;
        }
    };
    let (cols, rows) = match crossterm::terminal::size() {
        Ok((c, r)) => (c.max(20) as u32, r.max(8) as u32),
        Err(_) => (120, 60),
    };
    // Reserve the bottom 2 rows for the status line.
    let target_w = cols;
    let target_h = rows.saturating_sub(2).max(4) * 2; // 2 pixels per row
    let img = img.resize(target_w, target_h, image::imageops::FilterType::Triangle);
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let mut out = io::stdout().lock();
    for y in (0..h).step_by(2) {
        for x in 0..w {
            let upper = rgb.get_pixel(x, y);
            let lower = rgb.get_pixel(x, (y + 1).min(h - 1));
            let _ = write!(
                out,
                "\x1b[38;2;{};{};{};48;2;{};{};{}m\u{2580}",
                upper[0], upper[1], upper[2], lower[0], lower[1], lower[2],
            );
        }
        let _ = writeln!(out, "\x1b[0m");
    }
    let _ = out.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_protocol_returns_a_variant() {
        let protocol = detect_protocol();
        // Just verify it doesn't panic and returns a valid variant
        match protocol {
            ImageProtocol::Kitty
            | ImageProtocol::Sixel
            | ImageProtocol::Iterm2
            | ImageProtocol::Halfblock
            | ImageProtocol::Ascii => {}
        }
    }

    #[test]
    fn view_state_default_has_identity_zoom() {
        let state = ViewState::default();
        assert!((state.zoom - 1.0).abs() < f64::EPSILON);
        assert!((state.pan_x).abs() < f64::EPSILON);
        assert!((state.pan_y).abs() < f64::EPSILON);
    }

    #[test]
    fn view_state_zoom_increases() {
        let mut state = ViewState::default();
        let before = state.zoom;
        state.zoom_in();
        assert!(state.zoom > before);
    }

    #[test]
    fn view_state_zoom_decreases() {
        let mut state = ViewState::default();
        state.zoom_in();
        let before = state.zoom;
        state.zoom_out();
        assert!(state.zoom < before);
    }

    #[test]
    fn view_state_pan_moves() {
        let mut state = ViewState::default();
        state.pan_right();
        state.pan_down();
        assert!(state.pan_x > 0.0);
        assert!(state.pan_y > 0.0);
        state.pan_left();
        state.pan_up();
        assert!((state.pan_x).abs() < f64::EPSILON);
        assert!((state.pan_y).abs() < f64::EPSILON);
    }

    #[test]
    fn view_state_reset_returns_to_default() {
        let mut state = ViewState::default();
        state.zoom_in();
        state.pan_right();
        state.reset();
        assert!((state.zoom - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn render_options_from_view_state() {
        let state = ViewState::default();
        let opts = state.render_options();
        assert!((opts.scale - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn render_to_png_produces_valid_output() {
        let content =
            r#"{"elements":[{"type":"rectangle","id":"r1","x":0,"y":0,"width":50,"height":30}]}"#;
        let png = render_to_png(content, &ViewState::default()).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
        assert!(png.len() > 100);
    }

    #[test]
    fn render_to_svg_produces_valid_output() {
        let content =
            r#"{"elements":[{"type":"rectangle","id":"r1","x":0,"y":0,"width":50,"height":30}]}"#;
        let svg = render_to_svg(content, &ViewState::default()).unwrap();
        assert!(svg.contains("<svg"));
    }
}
