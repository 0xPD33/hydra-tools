// ═══════════════════════════════════════════════════════════════════════════
// Hydra-Mail Client
// ═══════════════════════════════════════════════════════════════════════════

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use base64::{engine::general_purpose, Engine as _};
use serde_json::json;
use toon_format::{encode, EncodeOptions};
use toon_format::types::KeyFoldingMode;
use uuid::Uuid;
use tokio::net::UnixStream;
use tokio::io::{BufReader, AsyncBufReadExt, AsyncWriteExt};
use serde_json::Value;
use anyhow::{Result, Context};

/// Message received from hydra-mail
#[derive(Debug, Clone)]
pub struct MailMessage {
    pub channel: String,
    pub payload: String,  // TOON-formatted YAML-like string
}

/// Client for hydra-mail pub/sub system
pub struct HydraMailClient {
    project_path: std::path::PathBuf,
    socket_path: std::path::PathBuf,
}

impl HydraMailClient {
    /// Connect to hydra-mail daemon
    pub fn connect(project_root: &Path) -> Result<Self> {
        // Use hydra-mail config to find socket
        let config_path = project_root.join(".hydra/config.toml");
        if !config_path.exists() {
            anyhow::bail!("Hydra not initialized. Run: hydra-mail init");
        }

        // Parse TOML to get socket path
        let config_str = std::fs::read_to_string(&config_path)
            .context("Failed to read config.toml")?;
        let config: Value = toml::from_str(&config_str)
            .context("Failed to parse config.toml")?;

        let socket_path = config["socket_path"].as_str()
            .context("Missing socket_path in config")?;
        let socket_path = std::path::PathBuf::from(socket_path);

        // Check if daemon is running
        if !socket_path.exists() {
            anyhow::bail!("Hydra daemon not running. Run: hydra-mail start");
        }

        Ok(Self {
            project_path: project_root.to_path_buf(),
            socket_path,
        })
    }

    /// Subscribe to a channel (async - returns a receiver stream)
    pub async fn subscribe(&self, channel: &str) -> Result<tokio::sync::mpsc::Receiver<MailMessage>> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .await
            .context("Failed to connect to hydra-mail socket")?;

        // Send subscribe command
        let cmd = serde_json::json!({
            "cmd": "subscribe",
            "channel": channel
        });
        let cmd_str = cmd.to_string();

        stream.write_all(cmd_str.as_bytes()).await
            .context("Failed to write subscribe command")?;
        stream.write_all(b"\n").await
            .context("Failed to write newline")?;
        stream.flush().await
            .context("Failed to flush")?;

        // Create channel for messages
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Spawn task to receive messages
        let channel_name = channel.to_string();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stream).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                // Messages come as TOON-formatted YAML-like strings
                tx.send(MailMessage {
                    channel: channel_name.clone(),
                    payload: line,
                }).await.ok();
            }
        });

        Ok(rx)
    }

    /// Emit a message to a channel (synchronous, best-effort)
    pub fn emit(&self, channel: &str, payload: &str) -> Result<()> {
        use std::os::unix::net::UnixStream as StdUnixStream;
        use std::io::Write;

        let mut stream = StdUnixStream::connect(&self.socket_path)
            .context("Failed to connect to hydra-mail socket")?;

        let data_json: Value = serde_json::from_str(payload)
            .unwrap_or_else(|_| Value::String(payload.to_string()));
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let pulse_json = json!({
            "id": Uuid::new_v4(),
            "timestamp": timestamp,
            "type": "status",
            "channel": channel,
            "data": data_json,
            "metadata": null
        });

        let encode_opts = EncodeOptions::new()
            .with_key_folding(KeyFoldingMode::Safe);
        let toon_str = encode(&pulse_json, &encode_opts)
            .context("Failed to encode to TOON")?;
        let encoded_data = general_purpose::STANDARD.encode(toon_str.as_bytes());

        let cmd = json!({
            "cmd": "emit",
            "channel": channel,
            "format": "toon",
            "data": encoded_data
        });
        let cmd_str = cmd.to_string();

        stream.write_all(cmd_str.as_bytes())
            .context("Failed to write emit command")?;
        stream.write_all(b"\n")
            .context("Failed to write newline")?;
        stream.flush()
            .context("Failed to flush")?;

        // Note: We don't read response in this sync version
        // The emit is best-effort - hydra-mail will log errors

        Ok(())
    }

    /// Try to get the project path
    pub fn project_path(&self) -> &Path {
        &self.project_path
    }
}
