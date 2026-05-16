use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "excd")]
#[command(about = "Native renderer for .excalidraw files")]
struct Cli {
    #[arg(long)]
    version_info: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.version_info {
        println!(
            "excd {} core={} render={} tui={} mcp={}",
            env!("CARGO_PKG_VERSION"),
            excalidraw_core::VERSION,
            excalidraw_render::VERSION,
            excalidraw_tui::VERSION,
            excalidraw_mcp::VERSION
        );
    }

    Ok(())
}
