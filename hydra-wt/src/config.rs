use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct WtConfig {
    pub ports: PortsConfig,
    pub env: EnvConfig,
    pub worktrees: WorktreesConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PortsConfig {
    pub range_start: u16,
    pub range_end: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnvConfig {
    pub template: String,
    pub output: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorktreesConfig {
    pub directory: String,
}

impl Default for WtConfig {
    fn default() -> Self {
        Self {
            ports: PortsConfig {
                range_start: 3001,
                range_end: 3099,
            },
            env: EnvConfig {
                template: ".env.template".to_string(),
                output: ".env.local".to_string(),
            },
            worktrees: WorktreesConfig {
                directory: "../".to_string(),
            },
        }
    }
}

impl WtConfig {
    pub fn config_path() -> PathBuf {
        PathBuf::from(".hydra/wt.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if !path.exists() {
            bail!("Config not found at {}. Run 'hydra-wt init' first.", path.display());
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let config: WtConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize config")?;
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write {}", path.display()))?;
        Ok(())
    }

    pub fn init() -> Result<()> {
        let hydra_dir = Path::new(".hydra");
        if !hydra_dir.exists() {
            bail!(".hydra/ directory not found. Run 'hydra-mail init' first.");
        }

        let config_path = Self::config_path();
        if config_path.exists() {
            bail!("Config already exists at {}", config_path.display());
        }

        let config = WtConfig::default();
        config.save()?;
        println!("Created {}", config_path.display());
        Ok(())
    }

    pub fn worktree_dir(&self) -> PathBuf {
        PathBuf::from(&self.worktrees.directory)
    }

    pub fn worktree_path(&self, branch: &str) -> PathBuf {
        self.worktree_dir().join(branch)
    }
}

pub fn get_project_uuid() -> Result<String> {
    let hydra_config_path = Path::new(".hydra/config.toml");
    if !hydra_config_path.exists() {
        bail!(".hydra/config.toml not found. Run 'hydra-mail init' first.");
    }
    let content = std::fs::read_to_string(hydra_config_path)
        .context("Failed to read .hydra/config.toml")?;

    #[derive(Deserialize)]
    struct HydraConfig {
        project_uuid: String,
    }

    let config: HydraConfig = toml::from_str(&content)
        .context("Failed to parse .hydra/config.toml")?;
    Ok(config.project_uuid)
}

pub fn get_repo_root() -> Result<PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("Failed to run git rev-parse")?;

    if !output.status.success() {
        bail!("Not in a git repository");
    }

    let path = String::from_utf8(output.stdout)
        .context("Invalid UTF-8 in git output")?;
    Ok(PathBuf::from(path.trim()))
}
