//! Hydra Observer - Animated Claude overlay that follows cursor and reacts to terminals

mod app;
mod config;
mod core;
mod input;
mod platform;
mod renderer;

use anyhow::Result;
use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "hydra-observer")]
#[command(about = "Animated Claude overlay that follows cursor and reacts to terminals")]
#[command(version)]
struct Cli {
    /// Config file path (defaults to XDG config)
    #[arg(short, long)]
    config: Option<std::path::PathBuf>,

    /// Force a specific platform backend
    #[arg(long, value_enum)]
    platform: Option<PlatformChoice>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum PlatformChoice {
    Wayland,
    X11,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.verbose {
        EnvFilter::new("hydra_observer=debug,wgpu=warn")
    } else {
        EnvFilter::new("hydra_observer=info,wgpu=error")
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    info!("Starting hydra-observer");

    // Load configuration
    let config = config::Config::load(cli.config.as_deref())?;
    info!(?config, "Loaded configuration");

    // Detect and initialize platform
    let platform = detect_platform(cli.platform)?;
    info!(?platform, "Detected platform");

    // Run the application
    app::run(config, platform)
}

fn detect_platform(forced: Option<PlatformChoice>) -> Result<platform::PlatformType> {
    if let Some(choice) = forced {
        return Ok(match choice {
            PlatformChoice::Wayland => platform::PlatformType::Wayland,
            PlatformChoice::X11 => platform::PlatformType::X11,
        });
    }

    // Auto-detect based on environment
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        Ok(platform::PlatformType::Wayland)
    } else if std::env::var("DISPLAY").is_ok() {
        Ok(platform::PlatformType::X11)
    } else {
        anyhow::bail!("No display server detected. Set WAYLAND_DISPLAY or DISPLAY.")
    }
}
