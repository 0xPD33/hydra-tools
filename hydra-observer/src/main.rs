//! Hydra Observer - HydraMail integration for Mascots
//!
//! This crate integrates the Mascots desktop companion with HydraMail,
//! enabling the mascot to react to agent communications and Hydra ecosystem events.

mod hydra_provider;

use anyhow::Result;
use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "hydra-observer")]
#[command(about = "HydraMail-integrated desktop companion")]
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
        EnvFilter::new("hydra_observer=debug,mascots=debug,wgpu=warn")
    } else {
        EnvFilter::new("hydra_observer=info,mascots=info,wgpu=error")
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    info!("Starting Hydra Observer (powered by Mascots)");

    // TODO: Initialize Hydra provider for HydraMail integration
    // let _provider = hydra_provider::HydraProvider::new()?;

    // Load mascots configuration
    let config = mascots::Config::load(cli.config.as_deref())?;
    info!(?config, "Loaded configuration");

    // Detect platform
    let platform = detect_platform(cli.platform)?;
    info!(?platform, "Detected platform");

    // Run the mascots application
    mascots::run(config, platform)
}

fn detect_platform(forced: Option<PlatformChoice>) -> Result<mascots::PlatformType> {
    if let Some(choice) = forced {
        return Ok(match choice {
            PlatformChoice::Wayland => mascots::PlatformType::Wayland,
            PlatformChoice::X11 => mascots::PlatformType::X11,
        });
    }

    // Auto-detect based on environment
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        Ok(mascots::PlatformType::Wayland)
    } else if std::env::var("DISPLAY").is_ok() {
        Ok(mascots::PlatformType::X11)
    } else {
        anyhow::bail!("No display server detected. Set WAYLAND_DISPLAY or DISPLAY.")
    }
}
