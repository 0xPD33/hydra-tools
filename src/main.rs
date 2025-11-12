use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use hydra_mail::{config::Config, channels, schema::Pulse, toon::{MessageFormat}};
use serde_json::{json, Value};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::str::FromStr;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::io::Read;
use tokio::net::{UnixListener, UnixStream};
use uuid::Uuid;
use base64;
use toon_format::{encode_default, decode_default};

#[derive(Parser)]
#[command(name = "hydra-mail")]
#[command(about = "Lightweight in-memory pub/sub for local agent collaboration")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize Hydra in the current project
    Init {
        /// Spawn daemon after init
        #[arg(long)]
        daemon: bool,
    },
    /// Start the persistent daemon
    Start {
        /// Project path (default: .)
        #[arg(short, long, default_value = ".")]
        project: String,
    },
    /// Emit a pulse to a channel
    Emit {
        /// Project path (default: .)
        #[arg(short, long, default_value = ".")]
        project: String,
        /// Pulse type (e.g., delta, ack)
        #[arg(short, long)]
        r#type: String,
        /// JSON data (use --data @- for stdin)
        #[arg(short, long)]
        data: Option<String>,
        /// Channel/topic
        #[arg(short, long)]
        channel: String,
        /// Message format (toon)
        #[arg(short = 'F', long, default_value = "toon")]
        format: String,
        /// Target agent ID for mpsc (optional)
        #[arg(short, long)]
        target: Option<String>,
    },
    /// Subscribe to a channel
    Subscribe {
        /// Project path (default: .)
        #[arg(short, long, default_value = ".")]
        project: String,
        /// Channel/topic
        #[arg(short, long)]
        channel: String,
        /// Output format (toon)
        #[arg(short, long, default_value = "toon")]
        format: String,
        /// Callback script to pipe output
        #[arg(short, long)]
        callback: Option<String>,
        /// Get one message and exit
        #[arg(short, long)]
        once: bool,
    },
    /// Show daemon status
    Status {
        /// Project path (default: .)
        #[arg(short, long, default_value = ".")]
        project: String,
    },
    /// Stop the daemon
    Stop {
        /// Project path (default: .)
        #[arg(short, long, default_value = ".")]
        project: String,
    },
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { daemon } => {
            let project_path = Path::new(".");
            let hydra_dir = project_path.join(".hydra");
            
            // Check if already initialized
            if hydra_dir.exists() {
                match Config::load(project_path) {
                    Ok(config) => {
                        println!("Hydra is already initialized in {:?}", project_path);
                        println!("Project UUID: {}", config.project_uuid);
                        println!("Socket path: {:?}", config.socket_path);
                        println!("Default topics: {}", config.default_topics.join(", "));
                        
                        // Check if daemon is running
                        let pid_path = hydra_dir.join("daemon.pid");
                        if pid_path.exists() {
                            if let Ok(pid_str) = fs::read_to_string(&pid_path) {
                                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                                    // Check if process is still alive
                                    use std::process::Command as CheckCmd;
                                    let check = CheckCmd::new("ps")
                                        .arg("-p")
                                        .arg(pid.to_string())
                                        .output();
                                    if let Ok(output) = check {
                                        if output.status.success() {
                                            println!("Daemon is running with PID: {}", pid);
                                        } else {
                                            println!("Daemon PID file exists but process not found (PID: {})", pid);
                                        }
                                    }
                                }
                            }
                        } else {
                            println!("Daemon is not running (no daemon.pid found)");
                        }
                        
                        println!("\nTo start the daemon, run: hydra-mail start");
                        println!("To see status, run: hydra-mail status");
                        return Ok(());
                    }
                    Err(e) => {
                        eprintln!("Warning: .hydra directory exists but config.toml is invalid: {}", e);
                        eprintln!("If you want to reinitialize, remove .hydra directory first");
                        return Err(anyhow::anyhow!("Cannot initialize: existing .hydra with invalid config"));
                    }
                }
            }
            
            // Initialize new project
            let config = Config::init(project_path)?;
            println!("Hydra initialized in {:?} with UUID: {}", project_path, config.project_uuid);
            println!("Socket path: {:?}", config.socket_path);
            if daemon {
                // Copy current binary to .hydra/hydra-daemon for reliable spawn
                let exe = std::env::current_exe()
                    .context("Failed to get current executable path")?;
                let daemon_binary = hydra_dir.join("hydra-daemon");
                fs::copy(&exe, &daemon_binary)
                    .context("Failed to copy binary for daemon")?;
                fs::set_permissions(&daemon_binary, fs::Permissions::from_mode(0o700))
                    .context("Failed to set daemon binary permissions")?;
                
                // Spawn daemon using the copied binary
                let mut child = Command::new(&daemon_binary)
                    .arg("start")
                    .arg("--project")
                    .arg(".")
                    .spawn()
                    .context("Failed to spawn daemon process")?;
                let pid = child.id();
                let pid_path = hydra_dir.join("daemon.pid");
                fs::write(&pid_path, pid.to_string().as_bytes())
                    .context("Failed to write daemon.pid")?;
                println!("Daemon spawned with PID: {}", pid);
            } else {
                println!("To start the daemon, run: hydra-mail start");
            }
        }
        Commands::Start { project } => {
            let project_path = Path::new(&project);
            let config = Config::load(project_path)?;
            println!("Starting daemon for project {:?} (UUID: {}, socket: {:?})", project_path, config.project_uuid, config.socket_path);
            
            // Remove existing socket if present
            let _ = fs::remove_file(&config.socket_path);
            
            let listener = UnixListener::bind(&config.socket_path)
                .context("Failed to bind Unix socket")?;
            
            // Set socket permissions to 0600
            fs::set_permissions(&config.socket_path, fs::Permissions::from_mode(0o600))
                .context("Failed to set socket permissions")?;
            
            println!("Daemon listening on {:?}", config.socket_path);
            
            loop {
                let (stream, _) = listener.accept().await?;
                tokio::spawn(handle_conn(stream, config.project_uuid));
            }
        }
        Commands::Emit { project, r#type, data, channel, format, target: _ } => {
            let project_path = Path::new(&project);
            let config = Config::load(project_path)?;
            
            let mut stream = UnixStream::connect(&config.socket_path)
                .await
                .context("Failed to connect to daemon socket. Is the daemon running? Run: hydra-mail start")?;
            
            // Read data from stdin if --data not provided
            let data_json: Value = if let Some(data_str) = data {
                serde_json::from_str(&data_str).context("Failed to parse --data JSON")?
            } else {
                // Read stdin synchronously (stdin is blocking anyway)
                let mut full_data = String::new();
                std::io::stdin().read_to_string(&mut full_data).context("Failed to read stdin")?;
                serde_json::from_str(&full_data).context("Failed to parse stdin JSON")?
            };

            // Create a Pulse with the provided data
            let pulse = Pulse::new(r#type, channel.clone(), data_json);

            // Validate pulse size
            pulse.validate_size().context("Pulse validation failed")?;

            // Always encode as TOON
            let toon_str = encode_default(&pulse)
                .context("Failed to encode pulse as TOON")?;
            let encoded_data = toon_str.into_bytes();

            let cmd_json = json!({
                "cmd": "emit",
                "channel": channel,
                "format": "toon",
                "data": base64::encode(&encoded_data)
            });

            let cmd_str = serde_json::to_string(&cmd_json).context("Failed to serialize command")?;
            
            // Split stream for read/write
            let (reader_side, mut writer) = stream.split();
            writer.write_all(cmd_str.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
            
            // Read response
            let mut reader = BufReader::new(reader_side).lines();
            if let Some(resp_line) = reader.next_line().await.context("Failed to read response")? {
                let resp: Value = serde_json::from_str(&resp_line).context("Failed to parse response")?;
                if resp["status"] == "error" {
                    eprintln!("Emit failed: {}", resp["msg"].as_str().unwrap_or("Unknown error"));
                    std::process::exit(1);
                } else {
                    println!("Emit successful");
                }
            }
        }
        Commands::Subscribe { project, channel, format: _, callback: _, once } => {
            let project_path = Path::new(&project);
            let config = Config::load(project_path)?;
            
            let mut stream = UnixStream::connect(&config.socket_path)
                .await
                .context("Failed to connect to daemon socket. Is the daemon running? Run: hydra-mail start")?;
            
            let (reader_side, mut writer) = stream.split();
            let mut reader = BufReader::new(reader_side).lines();
            
            let cmd_json = json!({
                "cmd": "subscribe",
                "channel": channel
            });
            
            let cmd_str = serde_json::to_string(&cmd_json).context("Failed to serialize command")?;
            writer.write_all(cmd_str.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
            
            // Stream messages
            let mut message_count = 0;
            while let Some(line) = reader.next_line().await.context("Failed to read from daemon")? {
                message_count += 1;
                println!("{}", line);
                
                if once {
                    break;
                }
            }
            
            if message_count == 0 {
                println!("No messages received (channel may be empty)");
            }
        }
        Commands::Status { project } => {
            let project_path = Path::new(&project);
            let hydra_dir = project_path.join(".hydra");
            
            if !hydra_dir.exists() {
                println!("No .hydra in {:?}. Run: hydra-mail init", project_path);
                return Ok(());
            }
            
            let config = Config::load(project_path)?;
            println!("Hydra Status for {:?}", project_path);
            println!("Project UUID: {}", config.project_uuid);
            println!("Socket path: {:?}", config.socket_path);
            
            // Check if socket exists
            if config.socket_path.exists() {
                println!("Socket: ✓ exists");
            } else {
                println!("Socket: ✗ missing (daemon not running?)");
            }
            
            // Check daemon PID
            let pid_path = hydra_dir.join("daemon.pid");
            if pid_path.exists() {
                if let Ok(pid_str) = fs::read_to_string(&pid_path) {
                    if let Ok(pid) = pid_str.trim().parse::<u32>() {
                        // Check if process is alive
                        let check = Command::new("ps")
                            .arg("-p")
                            .arg(pid.to_string())
                            .output();
                        
                        match check {
                            Ok(output) if output.status.success() => {
                                println!("Daemon: ✓ running (PID: {})", pid);
                                
                                // List active channels (if daemon is running, we can query it)
                                // For now, just show config defaults
                                println!("Default topics: {}", config.default_topics.join(", "));
                            }
                            _ => {
                                println!("Daemon: ✗ PID file exists but process not found (PID: {})", pid);
                                println!("  (Stale PID file - daemon may have crashed)");
                            }
                        }
                    }
                }
            } else {
                println!("Daemon: ✗ not running (no daemon.pid)");
            }
            
            // Try to connect and list channels if daemon is running
            if config.socket_path.exists() {
                match UnixStream::connect(&config.socket_path).await {
                    Ok(_) => {
                        // Daemon is responsive - could add RPC command to list channels
                        // For MVP, just show defaults
                        println!("\nNote: Use 'hydra-mail subscribe --channel <TOPIC>' to listen");
                    }
                    Err(_) => {
                        println!("\nWarning: Socket exists but cannot connect (daemon may be stuck)");
                    }
                }
            }
        }
        Commands::Stop { project } => {
            let project_path = Path::new(&project);
            let hydra_dir = project_path.join(".hydra");
            
            // Read PID
            let pid_path = hydra_dir.join("daemon.pid");
            if !pid_path.exists() {
                println!("No daemon.pid found in {:?}. Daemon not running?", project_path);
                return Ok(());
            }
            
            let pid_str = fs::read_to_string(&pid_path)
                .context("Failed to read daemon.pid")?;
            let pid: u32 = pid_str.trim().parse()
                .context("Invalid PID in daemon.pid")?;
            
            // Kill the process
            let kill_output = Command::new("kill")
                .arg("-TERM")
                .arg(pid.to_string())
                .output();
            
            match kill_output {
                Ok(output) if output.status.success() => {
                    println!("Daemon (PID: {}) terminated gracefully", pid);
                }
                Ok(_) | Err(_) => {
                    println!("Daemon (PID: {}) may not be running or already terminated", pid);
                }
            }
            
            // Clean up files
            let _ = fs::remove_file(&pid_path);
            let _ = fs::remove_file(hydra_dir.join("hydra-daemon"));
            let _ = fs::remove_file(hydra_dir.join("hydra.sock"));
            println!("Cleaned up daemon files in {:?}", project_path);
        }
    }

    Ok(())
}

async fn handle_conn(mut stream: UnixStream, project_uuid: Uuid) -> Result<()> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader).lines();

    while let Some(line) = reader.next_line().await? {
        let cmd: Value = serde_json::from_str(&line).context("Failed to parse JSON command")?;
        
        match cmd["cmd"].as_str() {
            Some("emit") => {
                let channel = cmd["channel"].as_str().context("Missing channel")?.to_string();

                // Get the base64 encoded TOON data
                let encoded_data = cmd["data"].as_str().context("Missing data")?;
                let decoded_bytes = base64::decode(encoded_data)
                    .context("Failed to decode base64 data")?;

                // Decode the TOON pulse
                let toon_str = String::from_utf8(decoded_bytes)
                    .context("Invalid UTF-8 in TOON data")?;
                let json_value = decode_default(&toon_str)
                    .context("Failed to decode TOON pulse")?;
                let pulse: Pulse = serde_json::from_value(json_value)
                    .context("Failed to convert TOON JSON to Pulse")?;

                // Validate the pulse
                pulse.validate_size().context("Pulse validation failed")?;

                // Use the original TOON data for internal storage (no re-encoding needed)
                let internal_data = toon_str.into_bytes();

                let tx = channels::get_or_create_broadcast_tx(project_uuid, &channel).await;
                // Broadcast send returns Ok(n) where n is number of receivers (can be 0)
                // or Err if channel is closed. For pub/sub, 0 receivers is fine.
                match tx.send(String::from_utf8_lossy(&internal_data).to_string()) {
                    Ok(_) => {
                        let ok_resp = json!({"status": "ok", "format": "toon", "size": internal_data.len()});
                        writer.write_all(ok_resp.to_string().as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                    }
                    Err(e) => {
                        let err_resp = json!({"status": "error", "msg": format!("Failed to send to channel: {}", e)});
                        writer.write_all(err_resp.to_string().as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                    }
                }
            }
            Some("subscribe") => {
                let channel = cmd["channel"].as_str().context("Missing channel")?.to_string();

                let mut rx = channels::subscribe_broadcast(project_uuid, &channel).await;

                // Stream messages until connection closes or error
                // Messages are already stored as TOON internally, just send them directly
                while let Ok(msg) = rx.recv().await {
                    writer.write_all(msg.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                }
            }
            _ => {
                let err_resp = json!({"status": "error", "msg": "Unknown command"});
                writer.write_all(err_resp.to_string().as_bytes()).await?;
                writer.write_all(b"\n").await?;
            }
        }
    }
    
    Ok(())
}
