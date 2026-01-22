use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use hydra_mail::{config::{Config, Limits}, channels, constants::*};
use serde_json::{json, Value};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use uuid::Uuid;
use base64::{Engine as _, engine::general_purpose};
use toon_format::{encode, EncodeOptions};
use toon_format::types::KeyFoldingMode;

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
        /// Message format (only 'toon' supported currently)
        #[arg(short = 'F', long, default_value = "toon")]
        format: String,
        /// Target agent ID (stored in metadata, agents can filter)
        #[arg(long)]
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
        /// Output format (only 'toon' supported currently)
        #[arg(short, long, default_value = "toon")]
        format: String,
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
    /// Handle Claude Code hook events
    Hook {
        #[command(subcommand)]
        event: HookEvent,
    },
}

#[derive(Subcommand)]
enum HookEvent {
    /// Handle SessionStart hook - check for messages from other agents
    SessionStart {
        /// Project path (default: .)
        #[arg(short, long, default_value = ".")]
        project: String,
    },
    /// Handle Stop hook - remind to emit summary
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

            // Generate config.sh for shell integration
            let sh_path = hydra_dir.join("config.sh");
            fs::write(&sh_path, config.generate_config_sh())
                .context("Failed to write config.sh")?;
            fs::set_permissions(&sh_path, fs::Permissions::from_mode(CONFIG_SH_PERMISSIONS))
                .context("Failed to set config.sh permissions")?;
            println!("✓ Generated .hydra/config.sh");

            println!("\nTo use with Claude Code, install the hydra-mail skill from:");
            println!("   https://github.com/0xPD33/hydra-tools/hydra-mail/skills/hydra-mail.yaml");

            if daemon {
                eprintln!("Spawning daemon process...");

                // Copy current binary to .hydra/hydra-daemon for reliable spawn
                let exe = std::env::current_exe()
                    .context("Failed to get current executable path. Is the binary installed correctly?")?;
                let daemon_binary = hydra_dir.join("hydra-daemon");
                fs::copy(&exe, &daemon_binary)
                    .context("Failed to copy binary for daemon")?;
                fs::set_permissions(&daemon_binary, fs::Permissions::from_mode(DAEMON_BINARY_PERMISSIONS))
                    .context("Failed to set daemon binary permissions")?;

                // Spawn daemon using the copied binary with proper detachment
                // Note: Don't pass --daemon flag since we're already detaching via stdio
                // Log stderr to daemon.err for debugging
                let err_log = hydra_dir.join("daemon.err");
                let err_file = fs::File::create(&err_log)
                    .context("Failed to create daemon.err")?;

                let child = Command::new(&daemon_binary)
                    .arg("start")
                    .arg("--project")
                    .arg(".")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(err_file)
                    .spawn()
                    .context("Failed to spawn daemon process")?;
                let pid = child.id();
                let pid_path = hydra_dir.join("daemon.pid");
                fs::write(&pid_path, pid.to_string().as_bytes())
                    .context("Failed to write daemon.pid")?;
                println!("Daemon spawned with PID: {}", pid);

                // Wait for socket to be created (up to 2 seconds)
                let socket_path = &config.socket_path;
                let mut attempts = 0;
                while !socket_path.exists() && attempts < 20 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    attempts += 1;
                }
                if !socket_path.exists() {
                    eprintln!("Warning: Daemon socket not created after 2s. Check {:?}", hydra_dir.join("daemon.err"));
                } else {
                    println!("Daemon ready at {:?}", socket_path);
                }
            } else {
                println!("To start the daemon, run: hydra-mail start");
            }
        }
        Commands::Start { project } => {
            let project_path = Path::new(&project);
            let config = Config::load(project_path)?;

            // Clean up stale files from previous daemon (if any)
            let project_path_abs = std::env::current_dir()?.join(&project);
            let hydra_dir = project_path_abs.join(".hydra");
            let pid_file = hydra_dir.join("daemon.pid");

            // Check if there's a stale PID file
            if pid_file.exists() {
                if let Ok(pid_str) = fs::read_to_string(&pid_file) {
                    if let Ok(pid) = pid_str.trim().parse::<u32>() {
                        let my_pid = std::process::id();

                        // If the PID matches our own, we were spawned by `init --daemon`
                        // and the parent already wrote our PID - just proceed
                        if pid == my_pid {
                            // This is us, proceed normally
                        } else {
                            // Check if the OTHER process is still alive
                            let check = Command::new("ps")
                                .arg("-p")
                                .arg(pid.to_string())
                                .output();
                            if let Ok(output) = check {
                                if !output.status.success() {
                                    // Process not running, clean up stale files
                                    let _ = fs::remove_file(&pid_file);
                                    let _ = fs::remove_file(&config.socket_path);
                                    eprintln!("Cleaned up stale daemon files (PID {} not running)", pid);
                                } else {
                                    anyhow::bail!("Daemon already running with PID {}. Use 'hydra-mail stop' first.", pid);
                                }
                            }
                        }
                    }
                }
            }

            // Remove existing socket if present
            let _ = fs::remove_file(&config.socket_path);

            let listener = UnixListener::bind(&config.socket_path)
                .context("Failed to bind Unix socket")?;

            // Set socket permissions to 0600
            fs::set_permissions(&config.socket_path, fs::Permissions::from_mode(SOCKET_PERMISSIONS))
                .context("Failed to set socket permissions")?;

            // Write PID file
            fs::write(&pid_file, std::process::id().to_string())
                .context("Failed to write daemon.pid")?;

            // Enable message logging for crash recovery
            let log_path = hydra_dir.join("messages.log");
            channels::set_message_log_path(Some(log_path.clone()));

            // Replay message log to restore state after crash
            if log_path.exists() {
                match channels::replay_message_log(&log_path).await {
                    Ok(count) => {
                        if count > 0 {
                            eprintln!("Restored {} messages from log", count);
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to replay message log: {}", e);
                    }
                }
            }

            // Set up signal handling for graceful shutdown
            let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .context("Failed to install SIGTERM handler")?;
            let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                .context("Failed to install SIGINT handler")?;

            eprintln!("Daemon started (PID: {}). Press Ctrl+C or send SIGTERM to stop.", std::process::id());

            // Spawn log compaction task (runs every 10 minutes)
            let log_path_compact = log_path.clone();
            let compaction_task = tokio::spawn(async move {
                use hydra_mail::message_log::MessageLog;
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(600)); // 10 min
                loop {
                    interval.tick().await;
                    if let Ok(log) = MessageLog::open(&log_path_compact) {
                        let _ = log.compact(REPLAY_BUFFER_CAPACITY);
                    }
                }
            });

            // Run the accepting loop with graceful shutdown
            loop {
                tokio::select! {
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok((stream, _)) => {
                                let project_uuid = config.project_uuid;
                                let limits = config.limits.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = handle_conn(stream, project_uuid, limits).await {
                                        eprintln!("Connection handler error: {:#}", e);
                                    }
                                });
                            }
                            Err(e) => {
                                eprintln!("Accept error: {}", e);
                                break;
                            }
                        }
                    }
                    _ = sigterm.recv() => {
                        eprintln!("Received SIGTERM, shutting down gracefully...");
                        break;
                    }
                    _ = sigint.recv() => {
                        eprintln!("Received SIGINT (Ctrl+C), shutting down gracefully...");
                        break;
                    }
                }
            }

            // Cleanup on shutdown
            compaction_task.abort();
            let _ = fs::remove_file(&pid_file);
            let _ = fs::remove_file(&config.socket_path);
            eprintln!("Daemon stopped cleanly.");
        }
        Commands::Emit { project, r#type, data, channel, format, target } => {
            // Validate format parameter
            if format != "toon" {
                anyhow::bail!("Only 'toon' format is supported (got: {})", format);
            }

            // Validate channel name
            if channel.trim().is_empty() {
                anyhow::bail!("Channel name cannot be empty");
            }

            let project_path = Path::new(&project);
            let config = Config::load(project_path)?;

            let mut stream = UnixStream::connect(&config.socket_path)
                .await
                .context(format!(
                    "Failed to connect to daemon socket at {:?}. \
                    Is the daemon running? Try:\n  \
                    1. Check status: hydra-mail status\n  \
                    2. Start daemon: hydra-mail start --daemon",
                    config.socket_path
                ))?;

            // Read data from stdin if --data not provided or if --data @-
            let data_json: Value = if let Some(data_str) = data {
                if data_str == "@-" {
                    // Read from stdin with size limit
                    use tokio::io::AsyncReadExt;
                    let stdin = tokio::io::stdin();
                    let mut buffer = Vec::with_capacity(MAX_STDIN_SIZE);
                    let bytes_read = stdin.take(MAX_STDIN_SIZE as u64).read_to_end(&mut buffer).await
                        .context("Failed to read stdin")?;
                    if bytes_read == MAX_STDIN_SIZE {
                        anyhow::bail!("Stdin data too large (max {} bytes)", MAX_STDIN_SIZE);
                    }
                    let full_data = String::from_utf8(buffer)
                        .context("Invalid UTF-8 in stdin")?;
                    serde_json::from_str(&full_data).context("Failed to parse stdin JSON")?
                } else {
                    serde_json::from_str(&data_str).context("Failed to parse --data JSON")?
                }
            } else {
                // Read stdin with size limit
                use tokio::io::AsyncReadExt;
                let stdin = tokio::io::stdin();
                let mut buffer = Vec::with_capacity(MAX_STDIN_SIZE);
                let bytes_read = stdin.take(MAX_STDIN_SIZE as u64).read_to_end(&mut buffer).await
                    .context("Failed to read stdin")?;
                if bytes_read == MAX_STDIN_SIZE {
                    anyhow::bail!("Stdin data too large (max {} bytes)", MAX_STDIN_SIZE);
                }
                let full_data = String::from_utf8(buffer)
                    .context("Invalid UTF-8 in stdin")?;
                serde_json::from_str(&full_data).context("Failed to parse stdin JSON")?
            };

            // Build Pulse JSON directly and encode to TOON (skip Pulse struct)
            let pulse_json = if let Some(target_id) = target {
                json!({
                    "id": Uuid::new_v4(),
                    "timestamp": chrono::Utc::now(),
                    "type": r#type,
                    "channel": channel.clone(),
                    "data": data_json,
                    "metadata": json!({"target": target_id})
                })
            } else {
                json!({
                    "id": Uuid::new_v4(),
                    "timestamp": chrono::Utc::now(),
                    "type": r#type,
                    "channel": channel.clone(),
                    "data": data_json,
                    "metadata": null
                })
            };

            // Encode directly to TOON with key folding
            let encode_opts = EncodeOptions::new()
                .with_key_folding(KeyFoldingMode::Safe);
            let toon_str = encode(&pulse_json, &encode_opts)
                .context("Failed to encode to TOON")?;

            // Message size validation
            if toon_str.len() > MAX_MESSAGE_SIZE {
                anyhow::bail!("Message too large: {} bytes (max {} bytes)", toon_str.len(), MAX_MESSAGE_SIZE);
            }

            let encoded_data = toon_str.into_bytes();

            let cmd_json = json!({
                "cmd": "emit",
                "channel": channel,
                "format": "toon",
                "data": general_purpose::STANDARD.encode(&encoded_data)
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
                if resp.get("status").and_then(|s| s.as_str()) == Some("error") {
                    let error_msg = resp.get("msg")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown error (missing or invalid 'msg' field)");
                    eprintln!("Emit failed: {}", error_msg);
                    std::process::exit(1);
                } else {
                    println!("Emit successful");
                }
            }
        }
        Commands::Subscribe { project, channel, format, once } => {
            // Validate format parameter
            if format != "toon" {
                anyhow::bail!("Only 'toon' format is supported (got: {})", format);
            }

            // Validate channel name
            if channel.trim().is_empty() {
                anyhow::bail!("Channel name cannot be empty");
            }

            let project_path = Path::new(&project);
            let config = Config::load(project_path)?;
            
            let mut stream = UnixStream::connect(&config.socket_path)
                .await
                .context(format!(
                    "Failed to connect to daemon socket at {:?}. \
                    Is the daemon running? Try:\n  \
                    1. Check status: hydra-mail status\n  \
                    2. Start daemon: hydra-mail start --daemon",
                    config.socket_path
                ))?;
            
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
            
            // Try to get stats from daemon if running
            if config.socket_path.exists() {
                match UnixStream::connect(&config.socket_path).await {
                    Ok(mut stream) => {
                        use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};

                        // Send stats command
                        let cmd = json!({"cmd": "stats"});
                        stream.write_all(cmd.to_string().as_bytes()).await?;
                        stream.write_all(b"\n").await?;
                        stream.flush().await?;

                        // Read response
                        let mut reader = BufReader::new(stream).lines();
                        if let Some(line) = reader.next_line().await? {
                            if let Ok(resp) = serde_json::from_str::<Value>(&line) {
                                if let Some(channels) = resp["channels"].as_array() {
                                    println!("\nActive Channels:");
                                    if channels.is_empty() {
                                        println!("  (none)");
                                    } else {
                                        for ch in channels {
                                            let name = ch["channel"].as_str().unwrap_or("?");
                                            let buffer_size = ch["replay_buffer_size"].as_u64().unwrap_or(0);
                                            let subs = ch["subscriber_count"].as_u64().unwrap_or(0);
                                            println!("  {} - {} msgs buffered, {} subscribers", name, buffer_size, subs);
                                        }
                                    }
                                }
                            }
                        }

                        // Show log file stats
                        let log_path = hydra_dir.join("messages.log");
                        if log_path.exists() {
                            if let Ok(metadata) = fs::metadata(&log_path) {
                                let size_kb = metadata.len() / 1024;
                                println!("\nMessage Log: {} KB", size_kb);
                            }
                        }
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

            // Load config to get socket path
            let config = Config::load(project_path)?;

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

            // Clean up files using config socket path
            let _ = fs::remove_file(&pid_path);
            let _ = fs::remove_file(&config.socket_path);
            let _ = fs::remove_file(hydra_dir.join("daemon.err"));
            println!("Cleaned up daemon files in {:?}", project_path);
        }
        Commands::Hook { event } => {
            match event {
                HookEvent::SessionStart { project } => {
                    // Check if hydra is initialized
                    let project_path = Path::new(&project);
                    let hydra_dir = project_path.join(".hydra");

                    if !hydra_dir.exists() {
                        // Not initialized - return continue with no context
                        println!("{}", json!({
                            "result": "continue"
                        }));
                        return Ok(());
                    }

                    // Try to get recent messages
                    let config = match Config::load(project_path) {
                        Ok(c) => c,
                        Err(_) => {
                            println!("{}", json!({
                                "result": "continue",
                                "message": "Hydra Mail: Config invalid"
                            }));
                            return Ok(());
                        }
                    };

                    // Check if daemon is running
                    if !config.socket_path.exists() {
                        println!("{}", json!({
                            "result": "continue",
                            "message": "Hydra Mail: Daemon not running. Run: hydra-mail init --daemon"
                        }));
                        return Ok(());
                    }

                    // Connect and get recent messages
                    match UnixStream::connect(&config.socket_path).await {
                        Ok(stream) => {
                            let (reader_side, mut writer) = stream.into_split();
                            let mut reader = BufReader::new(reader_side).lines();

                            // Send subscribe command (same format as CLI)
                            let subscribe_cmd = json!({
                                "cmd": "subscribe",
                                "channel": "repo:delta"
                            });
                            let cmd_str = subscribe_cmd.to_string() + "\n";
                            if writer.write_all(cmd_str.as_bytes()).await.is_err() {
                                println!("{}", json!({"result": "continue"}));
                                return Ok(());
                            }
                            let _ = writer.flush().await;

                            // Read messages with timeout (TOON format is multi-line YAML-like)
                            let mut content = String::new();
                            let mut line_count = 0;
                            loop {
                                match tokio::time::timeout(
                                    std::time::Duration::from_millis(100),
                                    reader.next_line()
                                ).await {
                                    Ok(Ok(Some(line))) => {
                                        content.push_str(&line);
                                        content.push('\n');
                                        line_count += 1;
                                        // Stop after ~50 lines to avoid too much context
                                        if line_count >= 50 {
                                            break;
                                        }
                                    }
                                    _ => break,
                                }
                            }

                            if content.trim().is_empty() {
                                println!("{}", json!({
                                    "result": "continue",
                                    "message": "Hydra Mail: No recent messages from other agents."
                                }));
                            } else {
                                // Format messages as context (TOON is human-readable YAML-like)
                                let context = format!(
                                    "## Recent Hydra Mail messages from other agents:\n\n```yaml\n{}```\n\nReview these before starting work.",
                                    content.trim()
                                );

                                println!("{}", json!({
                                    "result": "continue",
                                    "additionalContext": context
                                }));
                            }
                        }
                        Err(_) => {
                            println!("{}", json!({
                                "result": "continue",
                                "message": "Hydra Mail: Could not connect to daemon"
                            }));
                        }
                    }
                }
                HookEvent::Stop { project } => {
                    let project_path = Path::new(&project);
                    let hydra_dir = project_path.join(".hydra");

                    if !hydra_dir.exists() {
                        println!("{}", json!({"result": "continue"}));
                        return Ok(());
                    }

                    // Return reminder to emit summary
                    println!("{}", json!({
                        "result": "continue",
                        "additionalContext": "## Hydra Mail Reminder\n\nBefore finishing, emit a summary of your work:\n\n```bash\nhydra-mail emit --channel repo:delta --type status --data '{\"action\":\"completed\",\"summary\":\"<what you did>\"}'\n```"
                    }));
                }
            }
        }
    }

    Ok(())
}

async fn handle_conn(mut stream: UnixStream, project_uuid: Uuid, limits: Limits) -> Result<()> {
    use std::collections::VecDeque;
    use std::time::Instant;

    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader).lines();

    // Rate limiting: sliding window of emit timestamps
    let mut emit_times: VecDeque<Instant> = VecDeque::new();
    let rate_limit = limits.rate_limit_per_second;

    while let Some(line) = reader.next_line().await? {
        let cmd: Value = serde_json::from_str(&line).context("Failed to parse JSON command")?;

        match cmd["cmd"].as_str() {
            Some("emit") => {
                // Check rate limit (if enabled)
                if rate_limit > 0 {
                    let now = Instant::now();
                    // Remove timestamps older than 1 second
                    while let Some(&oldest) = emit_times.front() {
                        if now.duration_since(oldest).as_secs_f64() > 1.0 {
                            emit_times.pop_front();
                        } else {
                            break;
                        }
                    }
                    // Check if we're over the limit
                    if emit_times.len() >= rate_limit {
                        let err_resp = json!({
                            "status": "error",
                            "msg": format!("Rate limit exceeded: {} msgs/sec", rate_limit)
                        });
                        writer.write_all(err_resp.to_string().as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                        writer.flush().await?;
                        continue;
                    }
                    emit_times.push_back(now);
                }

                let channel = cmd["channel"].as_str().context("Missing channel")?.to_string();

                // Get the base64 encoded TOON data and store as-is (no decode needed!)
                let encoded_data = cmd["data"].as_str().context("Missing data")?;
                let decoded_bytes = general_purpose::STANDARD.decode(encoded_data)
                    .context("Failed to decode base64 data")?;

                // Check message size limit
                if decoded_bytes.len() > limits.max_message_size {
                    let err_resp = json!({
                        "status": "error",
                        "msg": format!("Message too large: {} bytes (max {})", decoded_bytes.len(), limits.max_message_size)
                    });
                    writer.write_all(err_resp.to_string().as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                    writer.flush().await?;
                    continue;
                }

                // Just validate UTF-8, but don't decode TOON
                let toon_str = String::from_utf8(decoded_bytes)
                    .context("Invalid UTF-8 in TOON data")?;

                // Emit and store in replay buffer atomically (daemon just passes through TOON)
                let toon_size = toon_str.len();
                let receiver_count = channels::emit_and_store(project_uuid, &channel, toon_str).await;
                let ok_resp = json!({"status": "ok", "format": "toon", "size": toon_size, "receivers": receiver_count});
                writer.write_all(ok_resp.to_string().as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
            Some("subscribe") => {
                let channel = cmd["channel"].as_str().context("Missing channel")?.to_string();

                let (mut rx, history) = channels::subscribe_broadcast(project_uuid, &channel).await;

                // Send history first (messages already in TOON format)
                for msg in history {
                    writer.write_all(msg.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                }
                // Flush history so clients receive it immediately
                writer.flush().await?;

                // Then stream live messages until connection closes or error
                while let Ok(msg) = rx.recv().await {
                    writer.write_all(msg.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                    writer.flush().await?;
                }
            }
            Some("stats") => {
                let stats = channels::get_channel_stats(project_uuid).await;
                let resp = json!({
                    "status": "ok",
                    "channels": stats
                });
                writer.write_all(resp.to_string().as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
            _ => {
                let err_resp = json!({"status": "error", "msg": "Unknown command"});
                writer.write_all(err_resp.to_string().as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
        }
    }
    
    Ok(())
}
