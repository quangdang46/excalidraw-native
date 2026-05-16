//! MCP server tools for excalidraw-native.
//!
//! Provides parse, describe, validate, to_svg, to_png, and render_file tools
//! through an rmcp stdio transport server.

use std::path::Path;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ServerInfo;
use rmcp::schemars::JsonSchema;
use rmcp::{tool, tool_handler, tool_router, ServerHandler, ServiceExt};
// JsonSchema comes from rmcp's schemars re-export
use serde::{Deserialize, Serialize};

/// Current crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the canonical crate boundary for diagnostics.
#[must_use]
pub fn crate_boundary() -> &'static str {
    "mcp-tools"
}

// ---- Tool parameter types ----

/// Parameters for tools that take a file path.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FilePathParams {
    /// Path to the .excalidraw file.
    pub path: String,
}

/// Parameters for tools that accept optional render options.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RenderParams {
    /// Path to the .excalidraw file.
    pub path: String,
    /// Render scale (default 1.0 for SVG, 2.0 for PNG).
    #[serde(default)]
    pub scale: Option<f64>,
    /// Padding around content (default 16.0).
    #[serde(default)]
    pub padding: Option<f64>,
    /// Render quality: full, fast-svg, or clean.
    #[serde(default)]
    pub quality: Option<String>,
}

/// Parameters for the validate tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidateParams {
    /// Path to the .excalidraw file.
    pub path: String,
}

// ---- Response types ----

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct ParseResponse {
    element_count: usize,
    element_types: Vec<String>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct DescribeResponse {
    elements: Vec<ElementDescription>,
    bounds: BoundsDescription,
    background_color: String,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct ElementDescription {
    id: String,
    type_name: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct BoundsDescription {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct ValidateResponse {
    valid: bool,
    element_count: usize,
    errors: Vec<String>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct SvgResponse {
    svg: String,
    width: u32,
    height: u32,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct PngResponse {
    png_base64: String,
    width: u32,
    height: u32,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct RenderFileResponse {
    format: String,
    width: u32,
    height: u32,
    data_base64: Option<String>,
    svg: Option<String>,
    warnings: Vec<String>,
}

// ---- Server ----

/// MCP server for excalidraw-native rendering tools.
#[derive(Debug, Clone)]
pub struct ExcalidrawServer {
    tool_router: ToolRouter<Self>,
}

impl ExcalidrawServer {
    /// Create a new server instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for ExcalidrawServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for ExcalidrawServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::default()
    }
}

#[tool_router]
impl ExcalidrawServer {
    /// Parse an .excalidraw file and return element counts and types.
    #[tool(
        name = "parse_elements",
        description = "Parse an .excalidraw file and return element counts, types, and warnings"
    )]
    pub async fn parse_elements(
        &self,
        params: Parameters<FilePathParams>,
    ) -> Result<String, String> {
        let raw = read_file(&params.0.path)?;
        let file = excalidraw_core::parse_str(&raw).map_err(|e| format!("Parse error: {e}"))?;
        let scene = excalidraw_core::normalize_file(&file);
        let element_types: Vec<String> = scene
            .elements
            .iter()
            .map(|e| element_type_name(&e.element))
            .collect();
        let warnings: Vec<String> = scene.warnings.iter().map(ToString::to_string).collect();
        let resp = ParseResponse {
            element_count: scene.elements.len(),
            element_types,
            warnings,
        };
        serde_json::to_string_pretty(&resp).map_err(|e| e.to_string())
    }

    /// Describe all elements in an .excalidraw file with positions and dimensions.
    #[tool(
        name = "describe_scene",
        description = "Describe all elements with IDs, types, positions, and scene bounds"
    )]
    pub async fn describe_scene(
        &self,
        params: Parameters<FilePathParams>,
    ) -> Result<String, String> {
        let raw = read_file(&params.0.path)?;
        let file = excalidraw_core::parse_str(&raw).map_err(|e| format!("Parse error: {e}"))?;
        let scene = excalidraw_core::normalize_file(&file);
        let elements: Vec<ElementDescription> = scene
            .elements
            .iter()
            .map(|e| ElementDescription {
                id: element_id(&e.element),
                type_name: element_type_name(&e.element),
                x: e.bounds.x,
                y: e.bounds.y,
                width: e.bounds.width,
                height: e.bounds.height,
            })
            .collect();
        let warnings: Vec<String> = scene.warnings.iter().map(ToString::to_string).collect();
        let resp = DescribeResponse {
            elements,
            bounds: BoundsDescription {
                x: scene.content_bounds.x,
                y: scene.content_bounds.y,
                width: scene.content_bounds.width,
                height: scene.content_bounds.height,
            },
            background_color: format!(
                "rgb({},{},{})",
                scene.background_color.r, scene.background_color.g, scene.background_color.b
            ),
            warnings,
        };
        serde_json::to_string_pretty(&resp).map_err(|e| e.to_string())
    }

    /// Validate an .excalidraw file for structural correctness.
    #[tool(
        name = "validate",
        description = "Validate an .excalidraw file and return errors and warnings"
    )]
    pub async fn validate(&self, params: Parameters<ValidateParams>) -> Result<String, String> {
        let raw = read_file(&params.0.path)?;
        let report =
            excalidraw_core::validate_str(&raw, &excalidraw_core::ValidationLimits::default());
        let resp = ValidateResponse {
            valid: report.valid,
            element_count: report.element_count,
            errors: report.errors.iter().map(ToString::to_string).collect(),
            warnings: report.warnings.iter().map(ToString::to_string).collect(),
        };
        serde_json::to_string_pretty(&resp).map_err(|e| e.to_string())
    }

    /// Render an .excalidraw file to SVG.
    #[tool(
        name = "to_svg",
        description = "Render an .excalidraw file to SVG with optional scale, padding, and quality settings"
    )]
    pub async fn to_svg(&self, params: Parameters<RenderParams>) -> Result<String, String> {
        let raw = read_file(&params.0.path)?;
        let file = excalidraw_core::parse_str(&raw).map_err(|e| format!("Parse error: {e}"))?;
        let scene = excalidraw_core::normalize_file(&file);
        let opts = build_render_options(&params.0);
        let result = excalidraw_render::render_svg(&scene, &opts)
            .map_err(|e| format!("Render error: {e}"))?;
        let warnings: Vec<String> = result.warnings.iter().map(ToString::to_string).collect();
        let view_box = scene.content_bounds.padded(opts.padding.max(0.0));
        let width = (view_box.width * opts.scale).ceil() as u32;
        let height = (view_box.height * opts.scale).ceil() as u32;
        let resp = SvgResponse {
            svg: result.value,
            width,
            height,
            warnings,
        };
        serde_json::to_string_pretty(&resp).map_err(|e| e.to_string())
    }

    /// Render an .excalidraw file to PNG (base64-encoded).
    #[tool(
        name = "to_png",
        description = "Render an .excalidraw file to PNG and return base64-encoded data with dimensions"
    )]
    pub async fn to_png(&self, params: Parameters<RenderParams>) -> Result<String, String> {
        let raw = read_file(&params.0.path)?;
        let file = excalidraw_core::parse_str(&raw).map_err(|e| format!("Parse error: {e}"))?;
        let scene = excalidraw_core::normalize_file(&file);
        let mut opts = build_render_options(&params.0);
        if params.0.scale.is_none() {
            opts.scale = 2.0;
        }
        let result = excalidraw_render::render_png(&scene, &opts)
            .map_err(|e| format!("Render error: {e}"))?;
        let warnings: Vec<String> = result.warnings.iter().map(ToString::to_string).collect();
        let view_box = scene.content_bounds.padded(opts.padding.max(0.0));
        let width = (view_box.width * opts.scale).ceil() as u32;
        let height = (view_box.height * opts.scale).ceil() as u32;
        let resp = PngResponse {
            png_base64: base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &result.value,
            ),
            width,
            height,
            warnings,
        };
        serde_json::to_string_pretty(&resp).map_err(|e| e.to_string())
    }

    /// Render an .excalidraw file, returning both SVG and PNG base64.
    #[tool(
        name = "render_file",
        description = "Render an .excalidraw file to both SVG and PNG, returning SVG content and PNG base64 with dimensions and warnings"
    )]
    pub async fn render_file(&self, params: Parameters<RenderParams>) -> Result<String, String> {
        let raw = read_file(&params.0.path)?;
        let file = excalidraw_core::parse_str(&raw).map_err(|e| format!("Parse error: {e}"))?;
        let scene = excalidraw_core::normalize_file(&file);
        let mut opts = build_render_options(&params.0);
        if params.0.scale.is_none() {
            opts.scale = 2.0;
        }

        let svg_result = excalidraw_render::render_svg(&scene, &opts)
            .map_err(|e| format!("SVG render error: {e}"))?;
        let png_result = excalidraw_render::render_png(&scene, &opts)
            .map_err(|e| format!("PNG render error: {e}"))?;

        let mut all_warnings: Vec<String> = svg_result
            .warnings
            .iter()
            .chain(&png_result.warnings)
            .map(ToString::to_string)
            .collect();
        all_warnings.sort();
        all_warnings.dedup();

        let view_box = scene.content_bounds.padded(opts.padding.max(0.0));
        let width = (view_box.width * opts.scale).ceil() as u32;
        let height = (view_box.height * opts.scale).ceil() as u32;

        let resp = RenderFileResponse {
            format: "svg+png".to_owned(),
            width,
            height,
            data_base64: Some(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &png_result.value,
            )),
            svg: Some(svg_result.value),
            warnings: all_warnings,
        };
        serde_json::to_string_pretty(&resp).map_err(|e| e.to_string())
    }
}

// ---- Helpers ----

fn read_file(path: &str) -> Result<String, String> {
    if path == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("stdin read error: {e}"))?;
        Ok(buf)
    } else {
        std::fs::read_to_string(Path::new(path))
            .map_err(|e| format!("file read error for {path}: {e}"))
    }
}

fn build_render_options(params: &RenderParams) -> excalidraw_render::RenderOptions {
    excalidraw_render::RenderOptions {
        scale: params.scale.unwrap_or(1.0),
        padding: params.padding.unwrap_or(16.0),
        background: excalidraw_render::BackgroundMode::FromFile,
        quality: match params.quality.as_deref() {
            Some("clean") => excalidraw_render::RenderQuality::Clean,
            Some("fast-svg") => excalidraw_render::RenderQuality::FastSvg,
            _ => excalidraw_render::RenderQuality::Full,
        },
        unsupported: excalidraw_render::UnsupportedElementMode::Placeholder,
        image_policy: excalidraw_render::ImagePolicy::Embed,
        text_policy: excalidraw_render::TextPolicy::SvgText,
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

fn element_id(element: &excalidraw_core::Element) -> String {
    match element {
        excalidraw_core::Element::Rectangle(e) => e.base.id.clone(),
        excalidraw_core::Element::Ellipse(e) => e.base.id.clone(),
        excalidraw_core::Element::Diamond(e) => e.base.id.clone(),
        excalidraw_core::Element::Arrow(e) => e.base.id.clone(),
        excalidraw_core::Element::Line(e) => e.base.id.clone(),
        excalidraw_core::Element::Text(e) => e.base.id.clone(),
        excalidraw_core::Element::Freedraw(e) => e.base.id.clone(),
        excalidraw_core::Element::Image(e) => e.base.id.clone(),
        excalidraw_core::Element::Frame(e) => e.base.id.clone(),
        excalidraw_core::Element::MagicFrame(e) => e.base.id.clone(),
        excalidraw_core::Element::Embeddable(e) => e.base.id.clone(),
        excalidraw_core::Element::Iframe(e) => e.base.id.clone(),
        excalidraw_core::Element::Unknown { raw, .. } => raw
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_owned(),
    }
}

/// Run the MCP server on stdio.
pub async fn run_server() -> Result<(), String> {
    let server = ExcalidrawServer::new();
    let transport = rmcp::transport::io::stdio();
    let service = server
        .serve(transport)
        .await
        .map_err(|e| format!("MCP serve error: {e}"))?;
    service
        .waiting()
        .await
        .map_err(|e| format!("MCP waiting error: {e}"))?;
    Ok(())
}
