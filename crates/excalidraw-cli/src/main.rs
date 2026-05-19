use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "excd")]
#[command(about = "Native renderer for .excalidraw files", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Render .excalidraw to SVG
    ToSvg {
        /// Input .excalidraw file path
        input: PathBuf,
        /// Output SVG file path (defaults to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Render scale
        #[arg(long, default_value = "1.0")]
        scale: f64,
        /// Padding around content
        #[arg(long, default_value = "16.0")]
        padding: f64,
        /// Background mode
        #[arg(long, default_value = "from-file")]
        background: BackgroundArg,
        /// Render quality
        #[arg(long, default_value = "full")]
        quality: QualityArg,
        /// How to handle unsupported elements
        #[arg(long, default_value = "placeholder")]
        unsupported: UnsupportedArg,
        /// How to handle images
        #[arg(long, default_value = "embed")]
        image_policy: ImagePolicyArg,
        /// Warning output mode
        #[arg(long, default_value = "text")]
        warnings: WarningMode,
    },
    /// Render .excalidraw to PNG
    ToPng {
        /// Input .excalidraw file path
        input: PathBuf,
        /// Output PNG file path (defaults to input.png)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Render scale
        #[arg(long, default_value = "2.0")]
        scale: f64,
        /// Padding around content
        #[arg(long, default_value = "16.0")]
        padding: f64,
        /// Background mode
        #[arg(long, default_value = "from-file")]
        background: BackgroundArg,
        /// Render quality
        #[arg(long, default_value = "full")]
        quality: QualityArg,
        /// How to handle unsupported elements
        #[arg(long, default_value = "placeholder")]
        unsupported: UnsupportedArg,
        /// How to handle images
        #[arg(long, default_value = "embed")]
        image_policy: ImagePolicyArg,
        /// Warning output mode
        #[arg(long, default_value = "text")]
        warnings: WarningMode,
    },
    /// Convert .excalidraw file between formats (inferred from output extension)
    Convert {
        /// Input .excalidraw file path
        input: PathBuf,
        /// Output file path (extension determines format: .svg or .png)
        output: PathBuf,
        /// Render scale
        #[arg(long, default_value = "1.0")]
        scale: f64,
        /// Padding around content
        #[arg(long, default_value = "16.0")]
        padding: f64,
        /// Background mode
        #[arg(long, default_value = "from-file")]
        background: BackgroundArg,
        /// Render quality
        #[arg(long, default_value = "full")]
        quality: QualityArg,
        /// Warning output mode
        #[arg(long, default_value = "text")]
        warnings: WarningMode,
    },
    /// Show scene info and element summary
    Info {
        /// Input .excalidraw file path
        input: PathBuf,
        /// Output format
        #[arg(long, default_value = "text")]
        format: InfoFormat,
    },
    /// Describe elements in a .excalidraw file
    Describe {
        /// Input .excalidraw file path
        input: PathBuf,
        /// Output format
        #[arg(long, default_value = "text")]
        format: InfoFormat,
    },
    /// Start MCP stdio server
    Serve,
    /// View .excalidraw in terminal with pan/zoom
    View {
        /// Input .excalidraw file path
        input: PathBuf,
        /// Force one-shot non-interactive rendering (no pan/zoom keybindings).
        /// Auto-enabled when stdin or stdout is not a TTY.
        #[arg(long)]
        no_interactive: bool,
        /// Force a specific image protocol instead of auto-detect.
        /// Also honors EXCD_VIEW_PROTOCOL.
        #[arg(long, value_name = "PROTO")]
        protocol: Option<String>,
        /// Render-side supersampling factor for halfblock sharpness.
        /// 1.0 renders at terminal-cell resolution; higher values render
        /// the diagram at N x scale and downsample. Default 2.0.
        #[arg(long, value_name = "FACTOR", default_value = "2.0")]
        supersample: f64,
    },
    /// Validate a .excalidraw file
    Validate {
        /// Input .excalidraw file path
        input: PathBuf,
        /// Output format
        #[arg(long, default_value = "text")]
        format: InfoFormat,
    },
    /// Convert Mermaid source to a .excalidraw scene
    MermaidToExcalidraw {
        /// Input Mermaid file path (or `-` for stdin)
        input: PathBuf,
        /// Output .excalidraw file path (defaults to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Font size for generated labels
        #[arg(long, default_value = "16")]
        font_size: u32,
        /// Flowchart curve style
        #[arg(long, default_value = "linear")]
        curve: FlowchartCurveArg,
        /// Maximum edges allowed (safety cap)
        #[arg(long, default_value = "5000")]
        max_edges: usize,
        /// Strategy when an unsupported Mermaid diagram is encountered
        #[arg(long, default_value = "placeholder")]
        on_unsupported: OnUnsupportedArg,
    },
    /// Convert Mermaid source and render directly to SVG or PNG
    Mermaid {
        /// Input Mermaid file path (or `-` for stdin)
        input: PathBuf,
        /// Output file path (extension determines format: .svg, .png or .excalidraw)
        output: PathBuf,
        /// Render scale
        #[arg(long, default_value = "1.0")]
        scale: f64,
        /// Padding around content
        #[arg(long, default_value = "16.0")]
        padding: f64,
        /// Font size for generated labels
        #[arg(long, default_value = "16")]
        font_size: u32,
        /// Flowchart curve style
        #[arg(long, default_value = "linear")]
        curve: FlowchartCurveArg,
        /// Strategy when an unsupported Mermaid diagram is encountered
        #[arg(long, default_value = "placeholder")]
        on_unsupported: OnUnsupportedArg,
        /// Render quality
        #[arg(long, default_value = "full")]
        quality: QualityArg,
        /// Warning output mode
        #[arg(long, default_value = "text")]
        warnings: WarningMode,
    },
}

#[derive(Debug, Clone, ValueEnum)]
enum FlowchartCurveArg {
    Linear,
    Basis,
}

#[derive(Debug, Clone, ValueEnum)]
enum OnUnsupportedArg {
    Placeholder,
    Error,
}

#[derive(Debug, Clone, ValueEnum)]
enum BackgroundArg {
    FromFile,
    Transparent,
    Override,
}

#[derive(Debug, Clone, ValueEnum)]
enum QualityArg {
    Full,
    FastSvg,
    Clean,
}

#[derive(Debug, Clone, ValueEnum)]
enum UnsupportedArg {
    Placeholder,
    Skip,
    Error,
}

#[derive(Debug, Clone, ValueEnum)]
enum ImagePolicyArg {
    Embed,
    Skip,
}

#[derive(Debug, Clone, ValueEnum)]
enum WarningMode {
    Text,
    Json,
    Silent,
}

#[derive(Debug, Clone, ValueEnum)]
enum InfoFormat {
    Text,
    Json,
}

fn read_input(path: &PathBuf) -> Result<String> {
    if path.to_string_lossy() == "-" {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .context("reading stdin")?;
        Ok(buf)
    } else {
        fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
    }
}

fn parse_and_normalize(
    input: &str,
) -> Result<(excalidraw_core::ExcalidrawFile, excalidraw_core::Scene)> {
    let file = excalidraw_core::parse_str(input).context("parsing .excalidraw file")?;
    let scene = excalidraw_core::normalize_file(&file);
    Ok((file, scene))
}

fn render_options(
    scale: f64,
    padding: f64,
    background: &BackgroundArg,
    quality: &QualityArg,
    unsupported: &UnsupportedArg,
    image_policy: &ImagePolicyArg,
) -> excalidraw_render::RenderOptions {
    excalidraw_render::RenderOptions {
        scale,
        padding,
        background: match background {
            BackgroundArg::FromFile => excalidraw_render::BackgroundMode::FromFile,
            BackgroundArg::Transparent => excalidraw_render::BackgroundMode::Transparent,
            BackgroundArg::Override => excalidraw_render::BackgroundMode::FromFile, // CLI doesn't pass color yet
        },
        quality: match quality {
            QualityArg::Full => excalidraw_render::RenderQuality::Full,
            QualityArg::FastSvg => excalidraw_render::RenderQuality::FastSvg,
            QualityArg::Clean => excalidraw_render::RenderQuality::Clean,
        },
        unsupported: match unsupported {
            UnsupportedArg::Placeholder => excalidraw_render::UnsupportedElementMode::Placeholder,
            UnsupportedArg::Skip => excalidraw_render::UnsupportedElementMode::Skip,
            UnsupportedArg::Error => excalidraw_render::UnsupportedElementMode::Error,
        },
        image_policy: match image_policy {
            ImagePolicyArg::Embed => excalidraw_render::ImagePolicy::Embed,
            ImagePolicyArg::Skip => excalidraw_render::ImagePolicy::Skip,
        },
        text_policy: excalidraw_render::TextPolicy::SvgText,
    }
}

fn emit_warnings(warnings: &[excalidraw_render::RenderWarning], mode: &WarningMode) {
    if matches!(mode, WarningMode::Silent) || warnings.is_empty() {
        return;
    }
    match mode {
        WarningMode::Text => {
            for w in warnings {
                eprintln!("warning: {w}");
            }
        }
        WarningMode::Json => {
            if let Ok(json) = serde_json::to_string_pretty(&warnings) {
                eprintln!("{json}");
            }
        }
        WarningMode::Silent => {}
    }
}

fn emit_scene_warnings(warnings: &[excalidraw_core::SceneWarning], mode: &WarningMode) {
    if matches!(mode, WarningMode::Silent) || warnings.is_empty() {
        return;
    }
    match mode {
        WarningMode::Text => {
            for w in warnings {
                eprintln!("warning: {w}");
            }
        }
        WarningMode::Json => {
            if let Ok(json) = serde_json::to_string_pretty(&warnings) {
                eprintln!("{json}");
            }
        }
        WarningMode::Silent => {}
    }
}

fn write_output(path: &Option<PathBuf>, content: &[u8]) -> Result<()> {
    match path {
        Some(p) => fs::write(p, content).with_context(|| format!("writing {}", p.display()))?,
        None => io::stdout()
            .write_all(content)
            .context("writing to stdout")?,
    }
    Ok(())
}

fn mermaid_options(
    font_size: u32,
    curve: &FlowchartCurveArg,
    on_unsupported: &OnUnsupportedArg,
    max_edges: usize,
) -> excalidraw_mermaid::MermaidConvertOptions {
    excalidraw_mermaid::MermaidConvertOptions {
        font_size: font_size.max(1) as f64,
        flowchart_curve: match curve {
            FlowchartCurveArg::Linear => excalidraw_mermaid::FlowchartCurve::Linear,
            FlowchartCurveArg::Basis => excalidraw_mermaid::FlowchartCurve::Basis,
        },
        max_edges,
        max_text_size: 4_096,
        on_unsupported: match on_unsupported {
            OnUnsupportedArg::Placeholder => excalidraw_mermaid::OnUnsupported::Placeholder,
            OnUnsupportedArg::Error => excalidraw_mermaid::OnUnsupported::Error,
        },
        hachure_fill: false,
    }
}

fn element_id(element: &excalidraw_core::Element) -> &str {
    match element {
        excalidraw_core::Element::Rectangle(e) => &e.base.id,
        excalidraw_core::Element::Ellipse(e) => &e.base.id,
        excalidraw_core::Element::Diamond(e) => &e.base.id,
        excalidraw_core::Element::Arrow(e) => &e.base.id,
        excalidraw_core::Element::Line(e) => &e.base.id,
        excalidraw_core::Element::Text(e) => &e.base.id,
        excalidraw_core::Element::Freedraw(e) => &e.base.id,
        excalidraw_core::Element::Image(e) => &e.base.id,
        excalidraw_core::Element::Frame(e) => &e.base.id,
        excalidraw_core::Element::MagicFrame(e) => &e.base.id,
        excalidraw_core::Element::Embeddable(e) => &e.base.id,
        excalidraw_core::Element::Iframe(e) => &e.base.id,
        excalidraw_core::Element::Unknown { raw, .. } => {
            raw.get("id").and_then(|v| v.as_str()).unwrap_or("?")
        }
    }
}

fn element_type_name(element: &excalidraw_core::Element) -> String {
    match element {
        excalidraw_core::Element::Rectangle(_) => "rectangle".into(),
        excalidraw_core::Element::Ellipse(_) => "ellipse".into(),
        excalidraw_core::Element::Diamond(_) => "diamond".into(),
        excalidraw_core::Element::Arrow(_) => "arrow".into(),
        excalidraw_core::Element::Line(_) => "line".into(),
        excalidraw_core::Element::Text(_) => "text".into(),
        excalidraw_core::Element::Freedraw(_) => "freedraw".into(),
        excalidraw_core::Element::Image(_) => "image".into(),
        excalidraw_core::Element::Frame(_) => "frame".into(),
        excalidraw_core::Element::MagicFrame(_) => "magicframe".into(),
        excalidraw_core::Element::Embeddable(_) => "embeddable".into(),
        excalidraw_core::Element::Iframe(_) => "iframe".into(),
        excalidraw_core::Element::Unknown { element_type, .. } => element_type.clone(),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ToSvg {
            input,
            output,
            scale,
            padding,
            background,
            quality,
            unsupported,
            image_policy,
            warnings,
        } => {
            let raw = read_input(&input)?;
            let (_file, scene) = parse_and_normalize(&raw)?;
            let opts = render_options(
                scale,
                padding,
                &background,
                &quality,
                &unsupported,
                &image_policy,
            );
            let result = excalidraw_render::render_svg(&scene, &opts)?;
            emit_warnings(&result.warnings, &warnings);
            write_output(&output, result.value.as_bytes())?;
        }
        Commands::ToPng {
            input,
            output,
            scale,
            padding,
            background,
            quality,
            unsupported,
            image_policy,
            warnings,
        } => {
            let raw = read_input(&input)?;
            let (_file, scene) = parse_and_normalize(&raw)?;
            let output = output.unwrap_or_else(|| input.with_extension("png"));
            let opts = render_options(
                scale,
                padding,
                &background,
                &quality,
                &unsupported,
                &image_policy,
            );
            let result = excalidraw_render::render_png(&scene, &opts)?;
            emit_warnings(&result.warnings, &warnings);
            write_output(&Some(output), &result.value)?;
        }
        Commands::Convert {
            input,
            output,
            scale,
            padding,
            background,
            quality,
            warnings,
        } => {
            let raw = read_input(&input)?;
            let (_file, scene) = parse_and_normalize(&raw)?;
            let opts = render_options(
                scale,
                padding,
                &background,
                &quality,
                &UnsupportedArg::Placeholder,
                &ImagePolicyArg::Embed,
            );
            let ext = output
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            match ext.as_str() {
                "svg" => {
                    let result = excalidraw_render::render_svg(&scene, &opts)?;
                    emit_warnings(&result.warnings, &warnings);
                    fs::write(&output, result.value.as_bytes())?;
                }
                "png" => {
                    let result = excalidraw_render::render_png(&scene, &opts)?;
                    emit_warnings(&result.warnings, &warnings);
                    fs::write(&output, &result.value)?;
                }
                _ => {
                    eprintln!("error: unsupported output format '.{ext}' (use .svg or .png)");
                    process::exit(1);
                }
            }
        }
        Commands::Info { input, format } => {
            let raw = read_input(&input)?;
            let (file, scene) = parse_and_normalize(&raw)?;
            emit_scene_warnings(&scene.warnings, &WarningMode::Text);
            match format {
                InfoFormat::Text => {
                    let mut counts = std::collections::HashMap::<String, usize>::new();
                    for elem in &file.elements {
                        *counts.entry(element_type_name(elem)).or_default() += 1;
                    }
                    println!("Elements: {}", scene.elements.len());
                    println!(
                        "Bounds: {:.1} x {:.1}",
                        scene.content_bounds.width, scene.content_bounds.height
                    );
                    println!("Background: {:?}", scene.background_color);
                    if !counts.is_empty() {
                        let mut types: Vec<_> = counts.iter().collect();
                        types.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
                        for (t, c) in types {
                            println!("  {t}: {c}");
                        }
                    }
                    if !scene.warnings.is_empty() {
                        println!("Warnings: {}", scene.warnings.len());
                    }
                }
                InfoFormat::Json => {
                    let info = serde_json::json!({
                        "element_count": scene.elements.len(),
                        "bounds": {
                            "x": scene.content_bounds.x,
                            "y": scene.content_bounds.y,
                            "width": scene.content_bounds.width,
                            "height": scene.content_bounds.height,
                        },
                        "warnings": scene.warnings.len(),
                    });
                    println!("{}", serde_json::to_string_pretty(&info)?);
                }
            }
        }
        Commands::Describe {
            input,
            format: _fmt,
        } => {
            let raw = read_input(&input)?;
            let (_file, scene) = parse_and_normalize(&raw)?;
            for elem in &scene.elements {
                let id = element_id(&elem.element);
                let etype = element_type_name(&elem.element);
                println!(
                    "{id:>20} {etype:>12} {:.0}x{:.0} at ({:.0},{:.0})",
                    elem.bounds.width, elem.bounds.height, elem.bounds.x, elem.bounds.y
                );
            }
        }
        Commands::View {
            input,
            no_interactive,
            protocol,
            supersample,
        } => {
            let raw = read_input(&input)?;
            let has_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
            let force = match protocol.as_deref() {
                Some(name) => Some(
                    excalidraw_tui::ImageProtocol::parse(name)
                        .map_err(|e| anyhow::anyhow!("{}", e))?,
                ),
                None => None,
            };
            let ss = supersample.clamp(1.0, 8.0);
            if no_interactive || !has_tty {
                excalidraw_tui::view_file_tuned(&raw, force, ss)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
            } else {
                excalidraw_tui::run_interactive_tuned(&raw, force, ss)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
            }
        }
        Commands::Serve => {
            tokio::runtime::Runtime::new()?
                .block_on(excalidraw_mcp::run_server())
                .map_err(|e| anyhow::anyhow!("MCP error: {e}"))?;
        }
        Commands::MermaidToExcalidraw {
            input,
            output,
            font_size,
            curve,
            max_edges,
            on_unsupported,
        } => {
            let raw = read_input(&input)?;
            let options = mermaid_options(font_size, &curve, &on_unsupported, max_edges);
            let value = excalidraw_mermaid::parse_to_excalidraw_value(&raw, &options)
                .context("converting Mermaid to Excalidraw")?;
            let serialized = serde_json::to_string_pretty(&value)?;
            match output {
                Some(p) => fs::write(&p, serialized.as_bytes())
                    .with_context(|| format!("writing {}", p.display()))?,
                None => {
                    io::stdout()
                        .write_all(serialized.as_bytes())
                        .context("writing to stdout")?;
                    io::stdout().write_all(b"\n").ok();
                }
            }
        }
        Commands::Mermaid {
            input,
            output,
            scale,
            padding,
            font_size,
            curve,
            on_unsupported,
            quality,
            warnings,
        } => {
            let raw = read_input(&input)?;
            let options = mermaid_options(font_size, &curve, &on_unsupported, 5000);
            let ext = output
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if ext == "excalidraw" {
                let value = excalidraw_mermaid::parse_to_excalidraw_value(&raw, &options)
                    .context("converting Mermaid to Excalidraw")?;
                fs::write(&output, serde_json::to_string_pretty(&value)?.as_bytes())?;
            } else {
                let file = excalidraw_mermaid::parse_to_excalidraw_file(&raw, &options)
                    .context("converting Mermaid to Excalidraw")?;
                let scene = excalidraw_core::normalize_file(&file);
                let opts = render_options(
                    scale,
                    padding,
                    &BackgroundArg::FromFile,
                    &quality,
                    &UnsupportedArg::Placeholder,
                    &ImagePolicyArg::Embed,
                );
                match ext.as_str() {
                    "svg" => {
                        let result = excalidraw_render::render_svg(&scene, &opts)?;
                        emit_warnings(&result.warnings, &warnings);
                        fs::write(&output, result.value.as_bytes())?;
                    }
                    "png" => {
                        let result = excalidraw_render::render_png(&scene, &opts)?;
                        emit_warnings(&result.warnings, &warnings);
                        fs::write(&output, &result.value)?;
                    }
                    _ => {
                        eprintln!(
                            "error: unsupported output format '.{ext}' (use .svg, .png or .excalidraw)"
                        );
                        process::exit(1);
                    }
                }
            }
        }
        Commands::Validate { input, format } => {
            let raw = read_input(&input)?;
            let report =
                excalidraw_core::validate_str(&raw, &excalidraw_core::ValidationLimits::default());
            match format {
                InfoFormat::Text => {
                    if report.valid {
                        println!("valid");
                    } else {
                        println!("invalid");
                        for err in &report.errors {
                            println!("  error: {err}");
                        }
                    }
                    for warn in &report.warnings {
                        println!("  warning: {warn}");
                    }
                    println!("Elements: {}", report.element_count);
                    if !report.valid {
                        process::exit(1);
                    }
                }
                InfoFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                    if !report.valid {
                        process::exit(1);
                    }
                }
            }
        }
    }

    Ok(())
}
