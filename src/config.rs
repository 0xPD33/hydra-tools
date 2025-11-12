use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub project_uuid: Uuid,
    pub socket_path: PathBuf,
    pub default_topics: Vec<String>,
}

impl Config {
    pub fn init(project_root: &Path) -> Result<Self> {
        let hydra_dir = project_root.join(".hydra");
        fs::create_dir_all(&hydra_dir).context("Failed to create .hydra directory")?;
        fs::set_permissions(&hydra_dir, fs::Permissions::from_mode(0o700))
            .context("Failed to set .hydra permissions")?;

        let project_uuid = Uuid::new_v4();
        // Use absolute path for socket to avoid issues with daemon cwd
        let socket_path = hydra_dir.canonicalize()
            .unwrap_or_else(|_| hydra_dir.to_path_buf())
            .join("hydra.sock");

        let config = Config {
            project_uuid,
            socket_path,
            default_topics: vec![
                "repo:delta".to_string(),
                "agent:presence".to_string(),
            ],
        };

        let config_path = hydra_dir.join("config.toml");
        let toml_str = toml::to_string(&config).context("Failed to serialize config to TOML")?;
        let mut file = File::create(&config_path).context("Failed to create config.toml")?;
        file.write_all(toml_str.as_bytes()).context("Failed to write config.toml")?;

        Ok(config)
    }

    pub fn load(project_root: &Path) -> Result<Self> {
        let config_path = project_root.join(".hydra").join("config.toml");
        let config_str = fs::read_to_string(&config_path).context("Failed to read config.toml")?;
        toml::from_str(&config_str).context("Failed to parse config.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_serialize_deserialize() {
        let config = Config {
            project_uuid: Uuid::parse_str("a1b2c3d4-e5f6-7890-abcd-ef1234567890").unwrap(),
            socket_path: PathBuf::from(".hydra/hydra.sock"),
            default_topics: vec!["repo:delta".to_string(), "agent:presence".to_string()],
        };

        let toml_str = toml::to_string(&config).unwrap();
        let loaded: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(config.project_uuid, loaded.project_uuid);
        assert_eq!(config.socket_path, loaded.socket_path);
        assert_eq!(config.default_topics, loaded.default_topics);
    }

    #[test]
    fn test_init_load() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();

        let config = Config::init(project_root).unwrap();
        assert!(project_root.join(".hydra").exists());

        let loaded = Config::load(project_root).unwrap();
        assert_eq!(config.project_uuid, loaded.project_uuid);
        assert!(config.socket_path.starts_with(project_root.join(".hydra")));
    }
}
