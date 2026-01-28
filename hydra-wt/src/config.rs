use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct WtConfig {
    pub ports: PortsConfig,
    pub env: EnvConfig,
    pub worktrees: WorktreesConfig,
    #[serde(default)]
    pub artifacts: ArtifactsConfig,
    #[serde(default)]
    pub hooks: HooksConfig,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ArtifactsConfig {
    #[serde(default)]
    pub symlink: Vec<String>,
    #[serde(default)]
    pub copy: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct HooksConfig {
    #[serde(default)]
    pub post_create: Vec<String>,
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
            artifacts: ArtifactsConfig::default(),
            hooks: HooksConfig::default(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WtConfig::default();
        assert_eq!(config.ports.range_start, 3001);
        assert_eq!(config.ports.range_end, 3099);
        assert_eq!(config.env.template, ".env.template");
        assert_eq!(config.env.output, ".env.local");
        assert_eq!(config.worktrees.directory, "../");
    }

    #[test]
    fn test_worktree_path() {
        let config = WtConfig::default();
        let path = config.worktree_path("feature-branch");
        assert_eq!(path, PathBuf::from("../feature-branch"));
    }

    #[test]
    fn test_worktree_dir() {
        let config = WtConfig::default();
        let dir = config.worktree_dir();
        assert_eq!(dir, PathBuf::from("../"));
    }

    #[test]
    fn test_config_serialization() {
        let config = WtConfig::default();
        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("range_start"));
        assert!(toml_str.contains("range_end"));
        assert!(toml_str.contains("template"));
    }

    #[test]
    fn test_config_deserialization() {
        let toml_str = r#"
            [ports]
            range_start = 4000
            range_end = 4100

            [env]
            template = ".env.example"
            output = ".env"

            [worktrees]
            directory = "../worktrees/"
        "#;
        let config: WtConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.ports.range_start, 4000);
        assert_eq!(config.ports.range_end, 4100);
        assert_eq!(config.env.template, ".env.example");
        assert_eq!(config.env.output, ".env");
        assert_eq!(config.worktrees.directory, "../worktrees/");
    }

    #[test]
    fn test_artifacts_config_default() {
        let artifacts = ArtifactsConfig::default();
        assert_eq!(artifacts.symlink.len(), 0);
        assert_eq!(artifacts.copy.len(), 0);
    }

    #[test]
    fn test_hooks_config_default() {
        let hooks = HooksConfig::default();
        assert_eq!(hooks.post_create.len(), 0);
    }

    #[test]
    fn test_load_missing_config_fails() {
        // Try to load from non-existent directory
        let orig_dir = std::env::current_dir().unwrap();
        let temp_dir = std::env::temp_dir().join(format!("hydra-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        let result = WtConfig::load();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Run 'hydra-wt init' first"));

        std::env::set_current_dir(orig_dir).unwrap();
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_init_without_hydra_dir_fails() {
        let orig_dir = std::env::current_dir().unwrap();
        let temp_dir = std::env::temp_dir().join(format!("hydra-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        let result = WtConfig::init();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(".hydra/ directory not found"));

        std::env::set_current_dir(orig_dir).unwrap();
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_config_with_invalid_toml() {
        let toml_str = r#"
            [ports]
            range_start = "not a number"
            range_end = 4100
        "#;
        let result: Result<WtConfig, _> = toml::from_str(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_with_missing_required_fields() {
        let toml_str = r#"
            [ports]
            range_start = 3000
            # Missing range_end
        "#;
        let result: Result<WtConfig, _> = toml::from_str(toml_str);
        assert!(result.is_err());
    }
}
