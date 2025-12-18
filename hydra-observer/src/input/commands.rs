//! IPC commands for external control

use anyhow::Result;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::mpsc;

/// Commands that can be sent via IPC
#[derive(Debug, Clone)]
pub enum Command {
    Show,
    Hide,
    Toggle,
    Poke,
    SetExcitement(f32),
    Quit,
    Status,
}

impl Command {
    /// Parse a command from a string
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_lowercase();
        match s.as_str() {
            "show" => Some(Command::Show),
            "hide" => Some(Command::Hide),
            "toggle" => Some(Command::Toggle),
            "poke" => Some(Command::Poke),
            "quit" => Some(Command::Quit),
            "status" => Some(Command::Status),
            _ if s.starts_with("set-excitement ") => {
                let val = s.strip_prefix("set-excitement ")?.trim().parse().ok()?;
                Some(Command::SetExcitement(val))
            }
            _ => None,
        }
    }
}

/// IPC server for receiving external commands
pub struct IpcServer {
    socket_path: PathBuf,
    command_tx: mpsc::Sender<Command>,
}

impl IpcServer {
    /// Create a new IPC server
    pub fn new(command_tx: mpsc::Sender<Command>) -> Result<Self> {
        let socket_path = Self::socket_path()?;

        // Remove existing socket if present
        if socket_path.exists() {
            std::fs::remove_file(&socket_path)?;
        }

        Ok(Self {
            socket_path,
            command_tx,
        })
    }

    /// Get the socket path
    fn socket_path() -> Result<PathBuf> {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir());

        Ok(runtime_dir.join("hydra-observer.sock"))
    }

    /// Run the IPC server (call from async context)
    pub async fn run(&self) -> Result<()> {
        let listener = UnixListener::bind(&self.socket_path)?;
        tracing::info!(path = ?self.socket_path, "IPC server listening");

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let tx = self.command_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, tx).await {
                            tracing::warn!("IPC connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    tracing::warn!("IPC accept error: {}", e);
                }
            }
        }
    }

    async fn handle_connection(
        stream: tokio::net::UnixStream,
        tx: mpsc::Sender<Command>,
    ) -> Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        while reader.read_line(&mut line).await? > 0 {
            if let Some(cmd) = Command::parse(&line) {
                let response = match &cmd {
                    Command::Status => {
                        // TODO: Return actual status
                        "{\"visible\": true}\n".to_string()
                    }
                    _ => "ok\n".to_string(),
                };

                tx.send(cmd).await?;
                writer.write_all(response.as_bytes()).await?;
            } else {
                writer.write_all(b"error: unknown command\n").await?;
            }
            line.clear();
        }

        Ok(())
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}
