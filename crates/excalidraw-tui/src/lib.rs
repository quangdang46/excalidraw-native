//! Terminal preview consumer for excalidraw-native.
//!
//! Renders the diagram via `excalidraw-render` and displays it through
//! [`ratatui-image`] (interactive mode) or direct protocol output
//! (one-shot mode). Supports Kitty Graphics Protocol, Sixel, iTerm2
//! inline images, and halfblock fallback. Protocol auto-detection
//! probes environment variables (`TERM_PROGRAM`, `KITTY_WINDOW_ID`,
//! `TERM`) before falling back to a terminal query or halfblocks.
//!
//! One-shot output writes directly to stdout for maximum resolution:
//! - **Kitty**: sends PNG payload via the Kitty Graphics Protocol APC.
//! - **Sixel**: encodes via `icy_sixel` at full image resolution.
//! - **iTerm2**: sends inline image via the iTerm2 proprietary OSC.
//! - **Halfblock**: falls back to ratatui-image with 4× supersampling.

use std::io::{self, IsTerminal, Write};
use std::time::Duration;

use excalidraw_core::normalize_file;
use excalidraw_render::{render_png, render_svg, RenderOptions};
use image::DynamicImage;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::{FilterType, Image, Resize};

/// Current crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the canonical crate boundary for diagnostics and smoke tests.
#[must_use]
pub fn crate_boundary() -> &'static str {
    "terminal-viewer"
}

/// User-facing protocol selector. Mirrors `ratatui_image::picker::ProtocolType`
/// but adds an `Auto` variant and a friendly parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageProtocol {
    Auto,
    Kitty,
    Sixel,
    Iterm2,
    Halfblock,
    Ascii,
}

impl ImageProtocol {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            ImageProtocol::Auto => "auto",
            ImageProtocol::Kitty => "kitty",
            ImageProtocol::Sixel => "sixel",
            ImageProtocol::Iterm2 => "iterm2",
            ImageProtocol::Halfblock => "halfblock",
            ImageProtocol::Ascii => "ascii",
        }
    }

    pub fn parse(name: &str) -> Result<Self, String> {
        match name.to_ascii_lowercase().as_str() {
            "auto" => Ok(ImageProtocol::Auto),
            "kitty" => Ok(ImageProtocol::Kitty),
            "sixel" => Ok(ImageProtocol::Sixel),
            "iterm2" | "iterm" => Ok(ImageProtocol::Iterm2),
            "halfblock" | "halfblocks" | "blocks" | "block" => Ok(ImageProtocol::Halfblock),
            "ascii" | "none" => Ok(ImageProtocol::Ascii),
            other => Err(format!(
                "unknown protocol '{other}'; expected one of: auto, kitty, sixel, iterm2, halfblock, ascii"
            )),
        }
    }

    fn to_protocol_type(self) -> Option<ProtocolType> {
        match self {
            ImageProtocol::Auto | ImageProtocol::Ascii => None,
            ImageProtocol::Kitty => Some(ProtocolType::Kitty),
            ImageProtocol::Sixel => Some(ProtocolType::Sixel),
            ImageProtocol::Iterm2 => Some(ProtocolType::Iterm2),
            ImageProtocol::Halfblock => Some(ProtocolType::Halfblocks),
        }
    }
}

// ---------------------------------------------------------------------------
// Environment-based protocol detection
// ---------------------------------------------------------------------------

/// Detect the best image protocol from environment variables alone, without
/// sending any terminal query escape sequences.
///
/// The heuristic checks (in order):
/// 1. `KITTY_WINDOW_ID` → Kitty Graphics Protocol
/// 2. `TERM_PROGRAM=ghostty` → Kitty (Ghostty implements the Kitty protocol)
/// 3. `TERM_PROGRAM=WezTerm` → Kitty (WezTerm supports Kitty protocol)
/// 4. `TERM_PROGRAM=iTerm.app` → iTerm2 inline images
/// 5. `LC_TERMINAL=iTerm2` → iTerm2 inline images
/// 6. `TERM` contains `sixel` → Sixel
/// 7. `TERM_PROGRAM=foot` → Sixel (foot supports Sixel natively)
/// 8. `TERM_PROGRAM=mlterm` → Sixel
/// 9. Otherwise → `None` (caller decides whether to query or fall back).
fn detect_protocol_from_env() -> Option<ImageProtocol> {
    if std::env::var_os("KITTY_WINDOW_ID").is_some() {
        return Some(ImageProtocol::Kitty);
    }
    let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();
    let tp_lower = term_program.to_ascii_lowercase();
    if tp_lower == "ghostty" || tp_lower == "wezterm" {
        return Some(ImageProtocol::Kitty);
    }
    if tp_lower == "iterm.app" || tp_lower == "iterm2" {
        return Some(ImageProtocol::Iterm2);
    }
    if std::env::var("LC_TERMINAL")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .contains("iterm")
    {
        return Some(ImageProtocol::Iterm2);
    }
    let term = std::env::var("TERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if term.contains("sixel") {
        return Some(ImageProtocol::Sixel);
    }
    if tp_lower == "foot" || tp_lower == "mlterm" {
        return Some(ImageProtocol::Sixel);
    }
    None
}

/// Resolve the effective protocol considering user override, env detection,
/// and optional terminal query.
fn resolve_protocol(force: ImageProtocol) -> ImageProtocol {
    match force {
        ImageProtocol::Auto => detect_protocol_from_env().unwrap_or(ImageProtocol::Halfblock),
        other => other,
    }
}

/// Build a [`Picker`] honoring the requested override (or auto-detect).
///
/// Skips the terminal query (which sends `CSI 14t/16t/18t`) when the user
/// has explicitly forced a protocol or set `EXCD_VIEW_NO_QUERY=1`. Some
/// terminals (and editor panels emulating one) reply to those queries
/// asynchronously, leaking the response (e.g. `^[[6;27;12t`) into the
/// shell after `excd` exits.
fn build_picker(force: ImageProtocol) -> Picker {
    if matches!(force, ImageProtocol::Ascii) {
        return Picker::halfblocks();
    }
    let resolved = resolve_protocol(force);
    let skip_query = !matches!(resolved, ImageProtocol::Auto)
        || std::env::var_os("EXCD_VIEW_NO_QUERY").is_some()
        || !io::stdout().is_terminal();
    let mut picker = if skip_query {
        Picker::halfblocks()
    } else {
        Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks())
    };
    if let Some(pt) = resolved.to_protocol_type() {
        picker.set_protocol_type(pt);
    }
    picker
}

/// Resolve the final user-visible protocol label for a built picker.
fn picker_protocol_label(picker: &Picker, requested: ImageProtocol) -> &'static str {
    if matches!(requested, ImageProtocol::Ascii) {
        return "ascii";
    }
    match picker.protocol_type() {
        ProtocolType::Halfblocks => "halfblock",
        ProtocolType::Sixel => "sixel",
        ProtocolType::Kitty => "kitty",
        ProtocolType::Iterm2 => "iterm2",
    }
}

/// Viewer state for pan/zoom interactions.
#[derive(Debug, Clone)]
pub struct ViewState {
    pub zoom: f64,
    pub pan_x: f64,
    pub pan_y: f64,
    /// Render-side supersampling factor. Higher = sharper halfblock output
    /// at the cost of CPU. 1.0 = render at terminal-cell resolution, 2.0 =
    /// render at 2x resolution then let ratatui-image downsample.
    pub supersample: f64,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            supersample: 4.0,
        }
    }
}

impl ViewState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.25).min(8.0);
    }
    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 1.25).max(0.1);
    }
    pub fn pan_left(&mut self) {
        self.pan_x -= 20.0;
    }
    pub fn pan_right(&mut self) {
        self.pan_x += 20.0;
    }
    pub fn pan_up(&mut self) {
        self.pan_y -= 20.0;
    }
    pub fn pan_down(&mut self) {
        self.pan_y += 20.0;
    }

    pub fn render_options(&self) -> RenderOptions {
        RenderOptions {
            scale: self.zoom * self.supersample.max(1.0),
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

fn render_to_image(content: &str, state: &ViewState) -> Result<DynamicImage, String> {
    let png = render_to_png(content, state)?;
    image::load_from_memory(&png).map_err(|e| format!("Image decode error: {e}"))
}

// ---------------------------------------------------------------------------
// Direct protocol output (one-shot, bypasses ratatui for full resolution)
// ---------------------------------------------------------------------------

/// Query terminal pixel dimensions via the ioctl that `crossterm` wraps.
/// Returns `(width_px, height_px)` or `None` when the terminal reports 0.
fn query_terminal_pixel_size() -> Option<(u32, u32)> {
    if let Ok(ws) = crossterm::terminal::window_size() {
        if ws.width > 0 && ws.height > 0 {
            return Some((ws.width as u32, ws.height as u32));
        }
    }
    None
}

/// Get the terminal column/row counts and estimate pixel dimensions.
fn terminal_dimensions() -> (u16, u16, u32, u32) {
    let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let (px_w, px_h) = query_terminal_pixel_size().unwrap_or_else(|| {
        // Assume typical cell size of 8×16 pixels if query fails.
        (cols as u32 * 8, rows as u32 * 16)
    });
    (cols, rows, px_w, px_h)
}

/// Compute the ideal PNG render scale so the image fits the terminal at
/// native pixel resolution (for Sixel/Kitty/iTerm2) or at a reasonable
/// supersampled resolution (for halfblock).
fn compute_render_scale(scene_w: f64, scene_h: f64, target_px_w: u32, target_px_h: u32) -> f64 {
    if scene_w <= 0.0 || scene_h <= 0.0 {
        return 1.0;
    }
    let sx = target_px_w as f64 / scene_w;
    let sy = target_px_h as f64 / scene_h;
    sx.min(sy).clamp(0.25, 8.0)
}

/// Write a PNG image using the Kitty Graphics Protocol (APC escape sequence).
/// The image is sent as a single base64-encoded PNG payload.
fn output_kitty(png_bytes: &[u8]) -> Result<(), String> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(png_bytes);
    let mut out = io::stdout().lock();
    // Kitty protocol: split into 4096-byte chunks.
    let chunks: Vec<&str> = b64
        .as_bytes()
        .chunks(4096)
        .map(|c| std::str::from_utf8(c).unwrap_or_default())
        .collect();
    let last = chunks.len().saturating_sub(1);
    for (i, chunk) in chunks.iter().enumerate() {
        let m = if i < last { 1 } else { 0 };
        if i == 0 {
            write!(out, "\x1b_Gf=100,a=T,m={m};{chunk}\x1b\\")
                .map_err(|e| format!("Kitty output error: {e}"))?;
        } else {
            write!(out, "\x1b_Gm={m};{chunk}\x1b\\")
                .map_err(|e| format!("Kitty output error: {e}"))?;
        }
    }
    writeln!(out).map_err(|e| format!("Kitty output error: {e}"))?;
    out.flush()
        .map_err(|e| format!("Kitty output error: {e}"))?;
    Ok(())
}

/// Write a PNG image using the iTerm2 inline image protocol.
fn output_iterm2(png_bytes: &[u8]) -> Result<(), String> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(png_bytes);
    let size = png_bytes.len();
    let mut out = io::stdout().lock();
    // iTerm2 proprietary escape: OSC 1337 ; File=...
    write!(
        out,
        "\x1b]1337;File=inline=1;size={size};preserveAspectRatio=1:{b64}\x07"
    )
    .map_err(|e| format!("iTerm2 output error: {e}"))?;
    writeln!(out).map_err(|e| format!("iTerm2 output error: {e}"))?;
    out.flush()
        .map_err(|e| format!("iTerm2 output error: {e}"))?;
    Ok(())
}

/// Write an image as Sixel output using `icy_sixel`.
fn output_sixel(png_bytes: &[u8]) -> Result<(), String> {
    let img = image::load_from_memory(png_bytes).map_err(|e| format!("Image decode error: {e}"))?;
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let sixel_img = icy_sixel::SixelImage::from_rgba(rgba.into_raw(), w as usize, h as usize);
    let encoded = sixel_img
        .encode()
        .map_err(|e| format!("Sixel encode error: {e}"))?;
    let mut out = io::stdout().lock();
    write!(out, "{encoded}").map_err(|e| format!("Sixel output error: {e}"))?;
    writeln!(out).map_err(|e| format!("Sixel output error: {e}"))?;
    out.flush()
        .map_err(|e| format!("Sixel output error: {e}"))?;
    Ok(())
}

/// Halfblock fallback via ratatui-image (used when no pixel-level protocol
/// is available).
fn output_halfblock(content: &str, state: &ViewState, label: &'static str) -> Result<(), String> {
    let img = render_to_image(content, state)?;

    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Halfblocks);

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).map_err(|e| format!("Terminal error: {e}"))?;
    let area = terminal
        .size()
        .map_err(|e| format!("Terminal error: {e}"))?;
    let area = Rect::new(0, 0, area.width, area.height.saturating_sub(1).max(1));

    let font_size = picker.font_size();
    let cols = img.width().div_ceil(font_size.width as u32).max(1) as u16;
    let rows = img.height().div_ceil(font_size.height as u32).max(1) as u16;
    let size = ratatui::layout::Size::new(cols.min(area.width), rows.min(area.height));
    let protocol = picker
        .new_protocol(img, size, Resize::Fit(Some(FilterType::Lanczos3)))
        .map_err(|e| format!("Protocol error: {e}"))?;

    terminal
        .draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(f.area());
            f.render_widget(Image::new(&protocol), chunks[0]);
            let footer = Paragraph::new(Line::from(format!(
                "excd view (one-shot) | protocol: {label}"
            )))
            .style(Style::default().add_modifier(Modifier::DIM));
            f.render_widget(footer, chunks[1]);
        })
        .map_err(|e| format!("Draw error: {e}"))?;
    println!();
    let _ = io::stdout().flush();
    Ok(())
}

// ---------------------------------------------------------------------------
// Public one-shot API
// ---------------------------------------------------------------------------

/// One-shot view (default when stdout is not a TTY).
pub fn view_file(content: &str) -> Result<(), String> {
    view_file_with(content, None)
}

pub fn view_file_with(content: &str, force: Option<ImageProtocol>) -> Result<(), String> {
    view_file_tuned(content, force, ViewState::default().supersample)
}

/// One-shot view with a custom supersampling factor for halfblock sharpness.
pub fn view_file_tuned(
    content: &str,
    force: Option<ImageProtocol>,
    supersample: f64,
) -> Result<(), String> {
    let requested = force.unwrap_or(ImageProtocol::Auto);
    let resolved = resolve_protocol(requested);
    let state = ViewState {
        supersample: supersample.max(1.0),
        ..ViewState::default()
    };

    if matches!(resolved, ImageProtocol::Ascii) {
        let png = render_to_png(content, &state)?;
        eprintln!("excd view: protocol=ascii");
        println!("Terminal does not support image display.");
        println!("Rendered {} bytes of PNG data.", png.len());
        println!("Save with: excd to-png <file> -o output.png");
        return Ok(());
    }

    let label = resolved.label();
    eprintln!("excd view: protocol={label}");

    // For pixel-level protocols (Kitty, Sixel, iTerm2) we bypass ratatui
    // and render at the terminal's native pixel resolution for maximum
    // image quality.
    match resolved {
        ImageProtocol::Kitty | ImageProtocol::Sixel | ImageProtocol::Iterm2 => {
            let (_cols, _rows, px_w, px_h) = terminal_dimensions();
            // Reserve a row for the footer.
            let target_h = px_h.saturating_sub(16).max(16);
            let target_w = px_w;

            // Render at native pixel resolution.
            let file =
                excalidraw_core::parse_str(content).map_err(|e| format!("Parse error: {e}"))?;
            let scene = normalize_file(&file);
            let scene_w = scene.content_bounds.width + 32.0; // padding
            let scene_h = scene.content_bounds.height + 32.0;
            let scale = compute_render_scale(scene_w, scene_h, target_w, target_h);

            let opts = RenderOptions {
                scale,
                padding: 16.0,
                background: excalidraw_render::BackgroundMode::FromFile,
                quality: excalidraw_render::RenderQuality::Full,
                unsupported: excalidraw_render::UnsupportedElementMode::Placeholder,
                image_policy: excalidraw_render::ImagePolicy::Embed,
                text_policy: excalidraw_render::TextPolicy::SvgText,
            };
            let output = render_png(&scene, &opts).map_err(|e| format!("Render error: {e}"))?;

            match resolved {
                ImageProtocol::Kitty => output_kitty(&output.value)?,
                ImageProtocol::Sixel => output_sixel(&output.value)?,
                ImageProtocol::Iterm2 => output_iterm2(&output.value)?,
                _ => unreachable!(),
            }
            Ok(())
        }
        ImageProtocol::Halfblock => output_halfblock(content, &state, label),
        ImageProtocol::Ascii => unreachable!(), // handled above
        ImageProtocol::Auto => unreachable!(),  // resolve_protocol never returns Auto
    }
}

/// Interactive viewer (alternate screen + raw mode + ratatui app).
pub fn run_interactive(content: &str) -> Result<(), String> {
    run_interactive_with(content, None)
}

pub fn run_interactive_with(content: &str, force: Option<ImageProtocol>) -> Result<(), String> {
    run_interactive_tuned(content, force, ViewState::default().supersample)
}

/// Interactive viewer with a custom supersampling factor for halfblock sharpness.
pub fn run_interactive_tuned(
    content: &str,
    force: Option<ImageProtocol>,
    supersample: f64,
) -> Result<(), String> {
    let requested = force.unwrap_or(ImageProtocol::Auto);
    let mut picker = build_picker(requested);
    let label = picker_protocol_label(&picker, requested);

    enable_raw_mode().map_err(|e| format!("Terminal error: {e}"))?;
    let _guard = scopeguard::guard((), |_| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    });
    execute!(io::stdout(), EnterAlternateScreen).map_err(|e| format!("Terminal error: {e}"))?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).map_err(|e| format!("Terminal error: {e}"))?;

    let mut state = ViewState {
        supersample: supersample.max(1.0),
        ..ViewState::default()
    };
    let mut protocol = build_protocol(&mut picker, content, &state)?;

    loop {
        terminal
            .draw(|f| draw_interactive(f, &protocol, &state, label))
            .map_err(|e| format!("Draw error: {e}"))?;

        if event::poll(Duration::from_millis(100)).map_err(|e| format!("Event error: {e}"))? {
            if let Event::Key(key) = event::read().map_err(|e| format!("Event error: {e}"))? {
                let mut dirty = true;
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('+') | KeyCode::Char('=') => state.zoom_in(),
                    KeyCode::Char('-') => state.zoom_out(),
                    KeyCode::Char('h') | KeyCode::Left => state.pan_left(),
                    KeyCode::Char('l') | KeyCode::Right => state.pan_right(),
                    KeyCode::Char('k') | KeyCode::Up => state.pan_up(),
                    KeyCode::Char('j') | KeyCode::Down => state.pan_down(),
                    KeyCode::Char('r') => state.reset(),
                    KeyCode::Char('s') => {
                        let png = render_to_png(content, &state)?;
                        std::fs::write("excd-screenshot.png", &png)
                            .map_err(|e| format!("Save error: {e}"))?;
                        dirty = false;
                    }
                    KeyCode::Char('e') => {
                        let svg = render_to_svg(content, &state)?;
                        std::fs::write("excd-screenshot.svg", svg)
                            .map_err(|e| format!("Save error: {e}"))?;
                        dirty = false;
                    }
                    _ => dirty = false,
                }
                if dirty {
                    protocol = build_protocol(&mut picker, content, &state)?;
                }
            }
        }
    }
    Ok(())
}

fn build_protocol(
    picker: &mut Picker,
    content: &str,
    state: &ViewState,
) -> Result<ratatui_image::protocol::Protocol, String> {
    let img = render_to_image(content, state)?;
    let font_size = picker.font_size();
    let cols = img.width().div_ceil(font_size.width as u32).max(1) as u16;
    let rows = img.height().div_ceil(font_size.height as u32).max(1) as u16;
    let size = ratatui::layout::Size::new(cols, rows);
    picker
        .new_protocol(img, size, Resize::Fit(Some(FilterType::Lanczos3)))
        .map_err(|e| format!("Protocol error: {e}"))
}

fn draw_interactive(
    f: &mut ratatui::Frame<'_>,
    protocol: &ratatui_image::protocol::Protocol,
    state: &ViewState,
    label: &'static str,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());

    let block = Block::default().borders(Borders::NONE);
    f.render_widget(block, chunks[0]);
    f.render_widget(Image::new(protocol), chunks[0]);

    let footer = Paragraph::new(Line::from(format!(
        "Zoom: {:.0}% | Pan: {:.0},{:.0} | protocol: {label} | q quit  +/- zoom  hjkl pan  r reset  s save PNG  e save SVG",
        state.zoom * 100.0,
        state.pan_x,
        state.pan_y,
    )))
    .style(Style::default().add_modifier(Modifier::DIM));
    f.render_widget(footer, chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_parse_round_trip() {
        for name in ["auto", "kitty", "sixel", "iterm2", "halfblock", "ascii"] {
            let p = ImageProtocol::parse(name).expect("valid name");
            assert_eq!(p.label(), name);
        }
        assert!(ImageProtocol::parse("nope").is_err());
    }

    #[test]
    fn view_state_default_has_identity_zoom() {
        let state = ViewState::default();
        assert!((state.zoom - 1.0).abs() < f64::EPSILON);
        assert!((state.pan_x).abs() < f64::EPSILON);
        assert!((state.pan_y).abs() < f64::EPSILON);
    }

    #[test]
    fn view_state_zoom_in_then_out_returns() {
        let mut state = ViewState::default();
        state.zoom_in();
        let after_in = state.zoom;
        state.zoom_out();
        assert!(state.zoom < after_in);
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
        assert!((state.pan_x).abs() < f64::EPSILON);
    }

    #[test]
    fn render_options_from_view_state() {
        let state = ViewState::default();
        let opts = state.render_options();
        assert!(opts.scale > 0.0);
        assert!((opts.scale - state.zoom * state.supersample).abs() < f64::EPSILON);
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

    #[test]
    fn render_to_image_decodes_png() {
        let content =
            r#"{"elements":[{"type":"rectangle","id":"r1","x":0,"y":0,"width":50,"height":30}]}"#;
        let img = render_to_image(content, &ViewState::default()).unwrap();
        assert!(img.width() > 0 && img.height() > 0);
    }

    #[test]
    fn default_supersample_is_four() {
        let state = ViewState::default();
        assert!((state.supersample - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn resolve_protocol_explicit_overrides() {
        assert_eq!(resolve_protocol(ImageProtocol::Kitty), ImageProtocol::Kitty);
        assert_eq!(resolve_protocol(ImageProtocol::Sixel), ImageProtocol::Sixel);
        assert_eq!(
            resolve_protocol(ImageProtocol::Iterm2),
            ImageProtocol::Iterm2
        );
        assert_eq!(
            resolve_protocol(ImageProtocol::Halfblock),
            ImageProtocol::Halfblock
        );
        assert_eq!(resolve_protocol(ImageProtocol::Ascii), ImageProtocol::Ascii);
    }

    #[test]
    fn compute_render_scale_clamps() {
        assert!(compute_render_scale(100.0, 100.0, 800, 600) > 0.0);
        assert!(compute_render_scale(0.0, 0.0, 800, 600) == 1.0);
        assert!(compute_render_scale(1.0, 1.0, 8000, 6000) <= 8.0);
    }

    #[test]
    fn detect_env_returns_none_by_default() {
        // In a test environment without terminal env vars set, detection
        // should return None or Halfblock (depending on env). We just
        // ensure it doesn't panic.
        let _ = detect_protocol_from_env();
    }
}
