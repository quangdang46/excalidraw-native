//! Terminal preview consumer for excalidraw-native.
//!
//! Renders the diagram via `excalidraw-render` and displays it through
//! [`ratatui-image`], which unifies Kitty / Sixel / iTerm2 / halfblocks
//! across terminals (auto-detected via `Picker::from_query_stdio` with a
//! halfblocks fallback). The viewer offers a one-shot mode and an
//! interactive ratatui app with pan/zoom keybindings.

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

/// Build a [`Picker`] honoring the requested override (or auto-detect).
fn build_picker(force: ImageProtocol) -> Picker {
    if matches!(force, ImageProtocol::Ascii) {
        return Picker::halfblocks();
    }
    let mut picker = if io::stdout().is_terminal() {
        Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks())
    } else {
        Picker::halfblocks()
    };
    if let Some(pt) = force.to_protocol_type() {
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
            supersample: 2.0,
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
    let mut state = ViewState::default();
    state.supersample = supersample.max(1.0);
    if matches!(requested, ImageProtocol::Ascii) {
        let png = render_to_png(content, &state)?;
        eprintln!("excd view: protocol=ascii");
        println!("Terminal does not support image display.");
        println!("Rendered {} bytes of PNG data.", png.len());
        println!("Save with: excd to-png <file> -o output.png");
        return Ok(());
    }

    let picker = build_picker(requested);
    let label = picker_protocol_label(&picker, requested);
    eprintln!("excd view: protocol={label}");

    let img = render_to_image(content, &state)?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).map_err(|e| format!("Terminal error: {e}"))?;
    let area = terminal.size().map_err(|e| format!("Terminal error: {e}"))?;
    let area = Rect::new(0, 0, area.width, area.height.saturating_sub(1).max(1));

    let font_size = picker.font_size();
    let cols =
        (img.width() as u32).div_ceil(font_size.width as u32).max(1) as u16;
    let rows =
        (img.height() as u32).div_ceil(font_size.height as u32).max(1) as u16;
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
    // Move cursor below the image so the shell prompt does not overwrite.
    println!();
    let _ = io::stdout().flush();
    Ok(())
}

/// Interactive viewer (alternate screen + raw mode + ratatui app).
pub fn run_interactive(content: &str) -> Result<(), String> {
    run_interactive_with(content, None)
}

pub fn run_interactive_with(
    content: &str,
    force: Option<ImageProtocol>,
) -> Result<(), String> {
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

    let mut state = ViewState::default();
    state.supersample = supersample.max(1.0);
    let mut protocol = build_protocol(&mut picker, content, &state)?;

    loop {
        terminal
            .draw(|f| draw_interactive(f, &protocol, &state, label))
            .map_err(|e| format!("Draw error: {e}"))?;

        if event::poll(Duration::from_millis(100))
            .map_err(|e| format!("Event error: {e}"))?
        {
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
    let cols = (img.width() as u32)
        .div_ceil(font_size.width as u32)
        .max(1) as u16;
    let rows = (img.height() as u32)
        .div_ceil(font_size.height as u32)
        .max(1) as u16;
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
}
