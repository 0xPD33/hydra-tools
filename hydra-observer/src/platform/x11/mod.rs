//! X11 platform implementation

use crate::config::Config;
use crate::core::ClaudeState;
use anyhow::Result;

/// X11 platform state
pub struct X11Platform {
    config: Config,
    claude_state: ClaudeState,
}

impl X11Platform {
    pub fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
            claude_state: ClaudeState::new(config),
        })
    }

    pub fn run(self) -> Result<()> {
        // TODO: Implement X11 backend
        // - Connect to X server
        // - Create override-redirect window
        // - Set up XComposite for transparency
        // - Set up XInput2 for pointer tracking
        // - Main event loop

        tracing::warn!("X11 backend not yet implemented");
        anyhow::bail!("X11 backend not yet implemented")
    }
}
