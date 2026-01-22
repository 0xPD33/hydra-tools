// ═══════════════════════════════════════════════════════════════════════════
// Hydralph Configuration
// ═══════════════════════════════════════════════════════════════════════════

use std::path::PathBuf;
use std::time::Duration;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct HydralphConfig {
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,

    #[serde(default = "default_max_duration_hours")]
    pub max_duration_hours: u64,

    #[serde(default)]
    pub agent_cli: String,

    #[serde(default)]
    pub agent_flags: String,
}

fn default_max_iterations() -> u32 { 10 }
fn default_max_duration_hours() -> u64 { 4 }

impl Default for HydralphConfig {
    fn default() -> Self {
        Self {
            max_iterations: default_max_iterations(),
            max_duration_hours: default_max_duration_hours(),
            agent_cli: "claude".into(),
            agent_flags: "--dangerously-skip-permissions".into(),
        }
    }
}

impl HydralphConfig {
    pub fn path() -> PathBuf {
        PathBuf::from(".hydra/ralph/config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let config: HydralphConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        Ok(config)
    }

    pub fn max_duration(&self) -> Duration {
        Duration::from_secs(self.max_duration_hours * 3600)
    }
}
