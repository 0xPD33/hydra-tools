//! Application state machine and main loop

use crate::config::Config;
use crate::core::ClaudeState;
use crate::platform::PlatformType;
use anyhow::Result;
use tracing::info;

/// Run the application with the given configuration and platform
pub fn run(config: Config, platform_type: PlatformType) -> Result<()> {
    info!("Initializing application");

    // Initialize the Claude state
    let _state = ClaudeState::new(&config);

    match platform_type {
        PlatformType::Wayland => run_wayland(config),
        PlatformType::X11 => run_x11(config),
    }
}

#[cfg(feature = "wayland")]
fn run_wayland(config: Config) -> Result<()> {
    use crate::platform::wayland::WaylandPlatform;

    info!("Starting Wayland backend");
    let platform = WaylandPlatform::new(&config)?;
    platform.run()
}

#[cfg(not(feature = "wayland"))]
fn run_wayland(_config: Config) -> Result<()> {
    anyhow::bail!("Wayland support not compiled in")
}

#[cfg(feature = "x11")]
fn run_x11(config: Config) -> Result<()> {
    use crate::platform::x11::X11Platform;

    info!("Starting X11 backend");
    let platform = X11Platform::new(&config)?;
    platform.run()
}

#[cfg(not(feature = "x11"))]
fn run_x11(_config: Config) -> Result<()> {
    anyhow::bail!("X11 support not compiled in")
}
