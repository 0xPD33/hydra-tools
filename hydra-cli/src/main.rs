// ═══════════════════════════════════════════════════════════════════════════
// Hydra CLI - Thin wrapper for orchestrator
// ═══════════════════════════════════════════════════════════════════════════

use clap::{Parser, Subcommand};
use hydra_orchestrator::{find_project_root, Orchestrator, SessionConfig, SessionId};
use std::fs;
use ansi_term::Colour;

const HYDRA_BANNER: &str = r#"
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡀⠀⠀⠀⠀⢠⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠻⣦⡀⠀⢸⣆⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⣠⣦⣤⣀⣀⣤⣤⣀⡀⠀⣀⣠⡆⠀⠀⠀⠀⠀⠀⠤⠒⠛⣛⣛⣻⣿⣶⣾⣿⣦⣄⢿⣆⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠸⠿⢿⣿⣿⣿⣯⣭⣿⣿⣿⣿⣋⣀⠀⠀⠀⠀⠀⠀⣠⣶⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣷⣤⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠙⢿⣿⣿⡿⢿⣿⣿⣿⣿⣿⣓⠢⠄⢠⡾⢻⣿⣿⣿⣿⡟⠁⠀⠀⠈⠙⢿⣿⣿⣯⡻⣿⡄⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠉⠉⠀⠀⠀⠙⢿⣿⣿⣿⣷⣄⠁⠀⣿⣿⣿⣿⣿⡇⠀⠀⠀⠀⠀⢸⣿⣿⣿⣿⣿⣷⣄⡀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⣿⣿⣿⣷⣌⢧⠀⣿⣿⣿⣿⣿⣿⣄⠀⠀⠀⠀⢀⠉⠙⠛⠛⠿⣿⣿⣿⡆⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣿⣿⣿⣿⣿⡀⠠⢻⡟⢿⣿⣿⣿⣿⣧⣄⣀⠀⠘⢶⣄⣀⠀⠀⠈⢻⠿⠁⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣸⣿⣿⣿⣿⣾⠀⠀⠀⠻⣈⣙⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿⣷⣦⡀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠈⠲⣄⠀⠀⣀⡤⠤⠀⠀⠀⢠⣿⣿⣿⡿⣿⠇⠀⠀⠐⠺⢉⣡⣴⣿⣿⣿⣿⣿⣿⣿⡿⢿⣿⣿⣿⣶⣿⣿⣿⣶⣶⡀⠀⠀⠀
⠀⠀⠀⠀⢠⣿⣴⣿⣷⣶⣦⣤⡀⠀⢸⣿⣿⣿⠇⠏⠀⠀⠀⢀⣴⣿⣿⣿⣿⣿⠟⢿⣿⣿⣿⣷⠀⠹⣿⣿⠿⠿⠛⠻⠿⣿⠇⠀⠀⠀
⠀⠀⠀⣠⣿⣿⣿⣿⣿⣿⣿⣷⣯⡂⢸⣿⣿⣿⠀⠀⠀⠀⢀⠾⣻⣿⣿⣿⠟⠀⠀⠈⣿⣿⣿⣿⡇⠀⠀⣀⣀⡀⠀⢠⡞⠉⠀⠀⠀⠀
⠀⠀⢸⣟⣽⣿⣯⠀⠀⢹⣿⣿⣿⡟⠼⣿⣿⣿⣇⠀⠀⠀⠠⢰⣿⣿⣿⣿⡄⠀⠀⠀⣸⣿⣿⣿⡇⠀⢀⣤⣼⣿⣷⣾⣷⡀⠀⠀⠀⠀
⠀⢀⣾⣿⡿⠟⠋⠀⠀⢸⣿⣿⣿⣿⡀⢿⣿⣿⣿⣦⠀⠀⠀⢺⣿⣿⣿⣿⣿⣄⠀⠀⣿⣿⣿⣿⡇⠐⣿⣿⣿⣿⠿⣿⣿⡿⣦⠀⠀⠀
⠀⢻⣿⠏⠀⠀⠀⠀⢠⣿⣿⣿⡟⡿⠀⠀⢻⣿⣿⣿⣷⣤⡀⠘⣷⠻⣿⣿⣿⣿⣷⣼⣿⣿⣿⣿⣇⣾⣿⣿⣿⠁⠀⢼⣿⣿⣿⣆⠀⠀
⠀⠀⠈⠀⠀⠀⠀⠀⢸⣿⣿⣿⡗⠁⠀⠀⠀⠙⢿⣿⣿⣿⣿⣷⣾⣆⡙⣿⣿⣿⣿⣿⣿⣿⣿⣿⠌⣾⣿⣿⣿⣆⠀⠀⠀⠉⠻⣿⡷⠀
⠀⠀⠀⠀⠀⠀⠀⠀⢸⣿⣿⣿⣷⣄⠀⠀⠀⠀⠀⠈⠻⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡏⠀⠘⣟⣿⣿⣿⡆⠀⠀⠀⠀⠙⠁⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠻⣿⣿⣿⣿⣿⣶⣤⣤⣤⣀⣠⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿⠀⠀⠀⢈⣿⣿⣿⡇⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠙⠿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣟⣠⣤⣤⣶⣿⣿⣿⠟⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⢀⣠⣤⣄⠀⠠⢶⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣟⡁⠀⠀⠀⠀⠀⠀⠀⠀⠀
⢀⣀⠀⣠⣀⡠⠞⣿⣿⣿⣿⣶⣾⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣴⣿⣷⣦⣄⣀⢿⡽⢻⣦
⠻⠶⠾⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠿⠋
"#;

fn print_version() {
    println!("{}", Colour::Cyan.paint(HYDRA_BANNER));
    println!("  {} {}",
        Colour::White.bold().paint("hydra"),
        Colour::Yellow.paint(env!("CARGO_PKG_VERSION"))
    );
    println!("  {}", Colour::White.dimmed().paint("Multi-agent development orchestration"));
    println!();
}

#[derive(Parser)]
#[command(name = "hydra")]
#[command(about = "Hydra Tools - Multi-agent development orchestration", long_about = None)]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(disable_version_flag = true)]
struct Cli {
    /// Print version
    #[arg(short = 'V', long = "version")]
    version: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize hydralph in current directory
    Init,

    /// Spawn a new hydralph session
    Spawn {
        #[arg(long, default_value = ".hydra/ralph/prd.json")]
        prd: String,

        #[arg(long, default_value_t = 10)]
        max_iterations: u32,

        #[arg(long, default_value = "4h")]
        max_duration: String,

        #[arg(long, default_value = "claude")]
        agent: String,

        #[arg(long)]
        worktree: bool,

        #[arg(long)]
        branch: Option<String>,
    },

    /// List active sessions
    Ls,

    /// Get session status
    Status {
        id: String
    },

    /// Attach to session (opens tmux)
    Attach {
        id: String
    },

    /// Pause session
    Pause {
        id: String
    },

    /// Resume session
    Resume {
        id: String
    },

    /// Inject message for agent
    Inject {
        id: String,
        message: String,
    },

    /// Kill session
    Kill {
        id: String,
        #[arg(long)]
        reason: Option<String>,
    },
}

fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Parse duration string like "4h", "30m", "1h30m" into seconds
fn parse_duration(s: &str) -> Result<u64, String> {
    let mut total = 0u64;
    let mut current = String::new();

    for ch in s.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else if ch == 'h' || ch == 'H' {
            let val: u64 = current.parse().map_err(|_| "Invalid number".to_string())?;
            total += val * 3600;
            current.clear();
        } else if ch == 'm' || ch == 'M' {
            let val: u64 = current.parse().map_err(|_| "Invalid number".to_string())?;
            total += val * 60;
            current.clear();
        } else if ch == 's' || ch == 'S' {
            let val: u64 = current.parse().map_err(|_| "Invalid number".to_string())?;
            total += val;
            current.clear();
        } else {
            return Err(format!("Invalid character in duration: {}", ch));
        }
    }

    if total == 0 {
        Err("Duration must be greater than 0".to_string())
    } else {
        Ok(total)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Handle version flag
    if cli.version {
        print_version();
        return Ok(());
    }

    // Require a command
    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            print_version();
            println!("Run 'hydra --help' for usage");
            return Ok(());
        }
    };

    // Try to connect to hydra-mail if available, otherwise run standalone
    let root = find_project_root();
    let mut orch = match Orchestrator::with_mail(&root) {
        Ok(o) => o,
        Err(_) => Orchestrator::new(),
    };

    match command {
        Commands::Init => {
            println!("{}", Colour::Cyan.paint(HYDRA_BANNER));

            let root = find_project_root();
            let ralph_dir = root.join(".hydra/ralph");
            fs::create_dir_all(&ralph_dir)?;
            println!("✅ Initialized {}", ralph_dir.display());

            // Create example prd.json if not exists
            let prd_path = ralph_dir.join("prd.json");
            if !prd_path.exists() {
                let example_prd = r#"{
  "title": "Project PRD",
  "userStories": [
    {
      "id": "story-1",
      "title": "First Story",
      "description": "Describe what needs to be done",
      "passes": false,
      "acceptance": ["Criteria 1", "Criteria 2"]
    }
  ]
}"#;
                fs::write(&prd_path, example_prd)?;
                println!("   Created prd.json (edit this with your stories)");
            }

            // Create progress.txt if not exists
            let progress_path = ralph_dir.join("progress.txt");
            if !progress_path.exists() {
                fs::write(&progress_path, "# Hydralph Progress Log\n")?;
                println!("   Created progress.txt");
            }

            // Copy hydralph templates from project root
            let project_hydralph = root.join("hydralph");

            // Copy hydralph.sh
            let script_src = project_hydralph.join("hydralph.sh");
            let script_dst = ralph_dir.join("hydralph.sh");
            if script_src.exists() && !script_dst.exists() {
                fs::copy(&script_src, &script_dst)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = fs::metadata(&script_dst)?.permissions();
                    perms.set_mode(0o755);
                    fs::set_permissions(&script_dst, perms)?;
                }
                println!("   Created hydralph.sh");
            }

            // Copy prompt.md
            let prompt_src = project_hydralph.join("prompt.md");
            let prompt_dst = ralph_dir.join("prompt.md");
            if prompt_src.exists() && !prompt_dst.exists() {
                fs::copy(&prompt_src, &prompt_dst)?;
                println!("   Created prompt.md");
            }

            println!();
            println!("Next steps:");
            println!("  1. Edit .hydra/ralph/prd.json with your stories");
            println!("  2. Run: hydra spawn");
        }

        Commands::Spawn { prd, max_iterations, max_duration, agent, worktree, branch } => {
            // Load config for defaults
            let ralph_config = match hydra_orchestrator::HydralphConfig::load() {
                Ok(cfg) => cfg,
                Err(e) => {
                    eprintln!("Warning: Failed to load config: {}", e);
                    hydra_orchestrator::HydralphConfig::default()
                }
            };

            // Parse duration (e.g., "4h", "30m", "1h30m")
            let max_duration_secs = if max_duration == "4h" {
                // Use default from config if not overridden
                ralph_config.max_duration().as_secs()
            } else {
                parse_duration(&max_duration).unwrap_or_else(|_| {
                    eprintln!("Invalid duration format: {}. Use e.g., 4h, 30m, 1h30m", max_duration);
                    std::process::exit(1);
                })
            };

            let config = SessionConfig {
                prd_path: prd.into(),
                max_iterations,
                max_duration: std::time::Duration::from_secs(max_duration_secs),
                agent_cli: agent,
                use_worktree: worktree,
                branch_name: branch,
                ..Default::default()
            };
            match orch.spawn(config) {
                Ok(id) => {
                    println!("🚀 Spawned session: {}", id.0);
                    println!("   Attach: hydra attach {}", id.0);
                    println!("   List:   hydra ls");
                }
                Err(e) => {
                    eprintln!("❌ Failed to spawn: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Ls => {
            let sessions = orch.list();
            if sessions.is_empty() {
                println!("No active sessions");
            } else {
                println!("{:<12} {:<20} {:<10} {}", "ID", "STATE", "DURATION", "TMUX");
                for s in sessions {
                    println!("{:<12} {:<20} {:<10} {}",
                        s.id,
                        s.state,
                        format_duration(s.duration),
                        s.tmux
                    );
                }
            }
        }

        Commands::Status { id } => {
            use hydra_orchestrator::SessionState;
            let session_id = hydra_orchestrator::SessionId(id.clone());
            if let Some(session) = orch.get_status(&session_id) {
                let state_color = match &session.state {
                    SessionState::Running { .. } => Colour::Green,
                    SessionState::Completed { .. } => Colour::Green,
                    SessionState::Paused => Colour::Yellow,
                    SessionState::Blocked { .. } | SessionState::Failed { .. } => Colour::Red,
                    SessionState::Stuck { .. } | SessionState::MaxIterations { .. } => Colour::Yellow,
                    _ => Colour::White,
                };

                println!("Session:  {}", Colour::Cyan.bold().paint(&session.id.0));
                println!("State:    {}", state_color.bold().paint(format!("{:?}", session.state)));
                println!("Duration: {}", format_duration(session.started_at.elapsed()));
                println!("TMUX:     {}", session.tmux_session);
                if let Some(port) = session.allocated_port {
                    println!("Port:     {}", port);
                }
                if let Some(wt) = &session.worktree_path {
                    println!("Worktree: {}", wt.display());
                }

                // Show additional details based on state
                match &session.state {
                    SessionState::Running { iteration, stories } => {
                        println!("Iteration: {}/{}", Colour::Yellow.paint(iteration.to_string()), session.config.max_iterations);
                        println!("Stories:   {}", stories);
                    }
                    SessionState::Completed { iterations } => {
                        println!("{}", Colour::Green.bold().paint(format!("✅ Completed in {} iterations", iterations)));
                    }
                    SessionState::Blocked { iteration, reason } => {
                        println!("{}", Colour::Red.bold().paint(format!("⛔ Blocked at iteration {}", iteration)));
                        println!("Reason: {}", reason);
                    }
                    SessionState::MaxIterations { iterations } => {
                        println!("{}", Colour::Yellow.bold().paint(format!("⚠️  Hit max iterations ({})", iterations)));
                    }
                    SessionState::Failed { reason } => {
                        println!("{}", Colour::Red.bold().paint(format!("❌ Failed: {}", reason)));
                    }
                    SessionState::Stuck { since, last_iteration } => {
                        let stuck_duration = since.elapsed();
                        println!("{}", Colour::Yellow.bold().paint(format!("⚠️  Stuck for {}", format_duration(stuck_duration))));
                        println!("Last iteration: {}", last_iteration);
                    }
                    SessionState::Paused => {
                        println!("{}", Colour::Yellow.bold().paint("⏸️  Paused"));
                    }
                    _ => {}
                }
            } else {
                eprintln!("Session not found: {}", id);
                std::process::exit(1);
            }
        }

        Commands::Attach { id } => {
            match orch.attach(&SessionId(id)) {
                Ok(_) => unreachable!(), // exec replaces process
                Err(e) => {
                    eprintln!("❌ Failed to attach: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Pause { id } => {
            match orch.pause(&SessionId(id)) {
                Ok(_) => println!("⏸️  Paused"),
                Err(e) => {
                    eprintln!("❌ Failed to pause: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Resume { id } => {
            match orch.resume(&SessionId(id)) {
                Ok(_) => println!("▶️  Resumed"),
                Err(e) => {
                    eprintln!("❌ Failed to resume: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Inject { id, message } => {
            match orch.inject(&SessionId(id), &message) {
                Ok(_) => println!("💉 Injected message for next iteration"),
                Err(e) => {
                    eprintln!("❌ Failed to inject: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Kill { id, reason } => {
            let reason = reason.as_deref().unwrap_or("user request");
            match orch.kill(&SessionId(id), reason) {
                Ok(_) => println!("💀 Killed"),
                Err(e) => {
                    eprintln!("❌ Failed to kill: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}
