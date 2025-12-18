//! Configuration loading and defaults

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub appearance: AppearanceConfig,

    #[serde(default)]
    pub behavior: BehaviorConfig,

    #[serde(default)]
    pub terminal_detection: TerminalDetectionConfig,

    #[serde(default)]
    pub advanced: AdvancedConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceConfig {
    /// Size multiplier for the Claude icon
    #[serde(default = "default_scale")]
    pub scale: f32,

    /// Glow intensity (0.0 - 1.0)
    #[serde(default = "default_glow")]
    pub glow_intensity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorConfig {
    /// Cursor following smoothness (higher = more responsive)
    #[serde(default = "default_smoothing")]
    pub smoothing: f32,

    /// How fast excitement ramps up/down
    #[serde(default = "default_transition_speed")]
    pub hover_transition_speed: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalDetectionConfig {
    /// Enable terminal detection and reactions
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Additional terminal app patterns to match
    #[serde(default)]
    pub additional_patterns: Vec<String>,

    /// Patterns to exclude from terminal detection
    #[serde(default)]
    pub excluded_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedConfig {
    /// Target frame rate (0 = vsync)
    #[serde(default)]
    pub frame_rate: u32,

    /// Multi-monitor behavior: "all", "primary", or "cursor"
    #[serde(default = "default_multi_monitor")]
    pub multi_monitor: String,
}

// Default value functions
fn default_scale() -> f32 {
    0.5
}
fn default_glow() -> f32 {
    0.5
}
fn default_smoothing() -> f32 {
    10.0
}
fn default_transition_speed() -> f32 {
    5.0
}
fn default_true() -> bool {
    true
}
fn default_multi_monitor() -> String {
    "all".to_string()
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            scale: default_scale(),
            glow_intensity: default_glow(),
        }
    }
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            smoothing: default_smoothing(),
            hover_transition_speed: default_transition_speed(),
        }
    }
}

impl Default for TerminalDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            additional_patterns: Vec::new(),
            excluded_patterns: Vec::new(),
        }
    }
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            frame_rate: 0,
            multi_monitor: default_multi_monitor(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            appearance: AppearanceConfig::default(),
            behavior: BehaviorConfig::default(),
            terminal_detection: TerminalDetectionConfig::default(),
            advanced: AdvancedConfig::default(),
        }
    }
}

impl Config {
    /// Load configuration from file or use defaults
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let config_path = path.map(PathBuf::from).or_else(Self::default_config_path);

        if let Some(ref path) = config_path {
            if path.exists() {
                let contents = std::fs::read_to_string(path)?;
                let config: Config = toml::from_str(&contents)?;
                return Ok(config);
            }
        }

        Ok(Config::default())
    }

    /// Get the default config file path
    fn default_config_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "hydra-observer")
            .map(|dirs| dirs.config_dir().join("config.toml"))
    }

    /// Get all terminal patterns (built-in + user-defined)
    pub fn terminal_patterns(&self) -> Vec<&str> {
        let mut patterns: Vec<&str> = BUILTIN_TERMINAL_PATTERNS.to_vec();

        // Add user patterns
        for p in &self.terminal_detection.additional_patterns {
            patterns.push(p.as_str());
        }

        // Remove excluded patterns
        patterns.retain(|p| {
            !self
                .terminal_detection
                .excluded_patterns
                .iter()
                .any(|e| e == *p)
        });

        patterns
    }
}

/// Built-in terminal application patterns
const BUILTIN_TERMINAL_PATTERNS: &[&str] = &[
    // App IDs (WM_CLASS on X11)
    "alacritty",
    "kitty",
    "wezterm",
    "foot",
    "gnome-terminal",
    "konsole",
    "xterm",
    "urxvt",
    "st",
    "terminator",
    "tilix",
    "hyper",
    "tabby",
    "iterm2",
    "terminal",
    "warp",
    // Title patterns (fallback)
    "— fish",
    "— bash",
    "— zsh",
];
