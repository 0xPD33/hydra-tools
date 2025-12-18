pub mod config;
pub mod channels;
pub mod constants;

// Removed modules (dead code):
// - schema: Pulse struct was never used, main.rs builds JSON directly
// - toon: MessageFormat enum was never used, main.rs does string comparison

// Future modules (commented to avoid missing file errors)
// pub mod broadcast;
// pub mod mpsc;

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use tempfile::TempDir;
    use uuid::Uuid;

    #[test]
    fn test_config_init() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let project_path = temp_dir.path();

        let config = config::Config::init(project_path)?;

        let hydra_dir = project_path.join(".hydra");
        assert!(hydra_dir.exists());
        assert!(hydra_dir.is_dir());

        let config_path = hydra_dir.join("config.toml");
        assert!(config_path.exists());
        assert!(config_path.is_file());

        let loaded = config::Config::load(project_path)?;
        assert_eq!(config.project_uuid, loaded.project_uuid);
        assert_eq!(config.socket_path, loaded.socket_path);
        assert_eq!(config.default_topics, loaded.default_topics);

        // Check UUID is valid (non-nil)
        assert_ne!(config.project_uuid, Uuid::nil());

        // Socket path should be reasonable (check as string since PathBuf::ends_with checks path components)
        let socket_str = config.socket_path.to_string_lossy();
        assert!(socket_str.ends_with(".sock"), "Socket path '{}' should end with .sock", socket_str);

        Ok(())
    }

    #[test]
    fn test_config_load_after_init() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let project_path = temp_dir.path();

        let config = config::Config::init(project_path)?;
        let loaded = config::Config::load(project_path)?;

        assert_eq!(config.project_uuid, loaded.project_uuid);
        assert_eq!(config.default_topics.len(), 2); // repo:delta, agent:presence

        Ok(())
    }

    #[tokio::test]
    async fn test_broadcast_channel() -> Result<()> {
        let project_uuid = Uuid::new_v4();
        let topic = "test:channel";

        let tx1 = channels::get_or_create_broadcast_tx(project_uuid, topic).await;
        let mut rx1 = tx1.subscribe();

        let msg = "test message".to_string();
        tx1.send(msg.clone()).expect("Send failed");

        let received: String = rx1.recv().await.expect("Recv failed");
        assert_eq!(received, msg);

        // Same tx for same key - test by subscribing again and sending
        let tx2 = channels::get_or_create_broadcast_tx(project_uuid, topic).await;
        let mut rx2 = tx2.subscribe();
        tx2.send("second".to_string()).expect("Send failed");
        let _ = rx2.recv().await.expect("Recv failed");

        Ok(())
    }

}
