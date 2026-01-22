mod artifacts;
mod config;
mod hooks;
mod hydra;
mod ports;
mod template;
mod worktree;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "hydra-wt")]
#[command(about = "Worktree management for the Hydra ecosystem")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize hydra-wt configuration
    Init,

    /// Create a new worktree with port allocation
    Create {
        /// Branch name (creates new branch if doesn't exist)
        branch: String,
    },

    /// List all managed worktrees
    List,

    /// Remove a worktree and free its port
    Remove {
        /// Branch name to remove
        branch: String,
        /// Force removal even with untracked/modified files
        #[arg(short, long)]
        force: bool,
    },

    /// Show status of worktrees
    Status {
        /// Specific branch to show (optional)
        branch: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init => cmd_init(),
        Commands::Create { branch } => cmd_create(&branch),
        Commands::List => cmd_list(),
        Commands::Remove { branch, force } => cmd_remove(&branch, force),
        Commands::Status { branch } => cmd_status(branch.as_deref()),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn cmd_init() -> Result<()> {
    config::WtConfig::init()?;
    ports::PortRegistry::init()?;
    println!("hydra-wt initialized successfully");
    Ok(())
}

fn cmd_create(branch: &str) -> Result<()> {
    let cfg = config::WtConfig::load()?;
    let mut registry = ports::PortRegistry::load()?;

    // Check if worktree already exists
    let wt_path = cfg.worktree_path(branch);
    if worktree::exists(&wt_path) {
        anyhow::bail!("Worktree already exists at {}", wt_path.display());
    }

    // Allocate port
    let port = registry.allocate(branch, cfg.ports.range_start, cfg.ports.range_end)?;
    println!("Allocated port {} for {}", port, branch);

    // Create worktree
    println!("Creating worktree at {}...", wt_path.display());
    if let Err(e) = worktree::add(&wt_path, branch) {
        // Rollback port allocation on failure
        registry.allocations.remove(branch);
        registry.save()?;
        return Err(e);
    }

    // Save port allocation
    registry.save()?;

    // Handle artifacts
    let repo_root = config::get_repo_root()?;
    if !cfg.artifacts.symlink.is_empty() || !cfg.artifacts.copy.is_empty() {
        println!("Setting up artifacts...");
    }
    for artifact in &cfg.artifacts.symlink {
        artifacts::symlink_artifact(&repo_root, &wt_path, artifact)?;
    }
    for artifact in &cfg.artifacts.copy {
        artifacts::copy_artifact(&repo_root, &wt_path, artifact)?;
    }

    // Render template if exists
    let template_path = PathBuf::from(&cfg.env.template);
    let output_path = wt_path.join(&cfg.env.output);

    let project_uuid = config::get_project_uuid().unwrap_or_else(|_| "unknown".to_string());
    let repo_root = config::get_repo_root()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string());

    let ctx = template::TemplateContext {
        port,
        worktree: branch.to_string(),
        project_uuid,
        repo_root,
    };

    template::render(&template_path, &output_path, &ctx)?;

    if template_path.exists() {
        println!("Created {}", output_path.display());
    }

    // Run post-create hooks
    hooks::run_post_create(&wt_path, &cfg.hooks.post_create)?;

    // Emit to Hydra
    hydra::emit_worktree_created(branch, port, &wt_path.to_string_lossy())?;

    println!("\nWorktree '{}' created successfully", branch);
    println!("  Path: {}", wt_path.display());
    println!("  Port: {}", port);

    Ok(())
}

fn cmd_list() -> Result<()> {
    let cfg = config::WtConfig::load()?;
    let registry = ports::PortRegistry::load()?;

    if registry.allocations.is_empty() {
        println!("No worktrees managed by hydra-wt");
        return Ok(());
    }

    println!("{:<20} {:<6} {:<30} {:<10}", "BRANCH", "PORT", "PATH", "STATUS");
    println!("{}", "-".repeat(70));

    for (branch, port) in registry.list() {
        let wt_path = cfg.worktree_path(branch);
        let status = if worktree::exists(&wt_path) {
            "exists"
        } else {
            "missing"
        };
        println!(
            "{:<20} {:<6} {:<30} {:<10}",
            branch,
            port,
            wt_path.display(),
            status
        );
    }

    Ok(())
}

fn cmd_remove(branch: &str, force: bool) -> Result<()> {
    let cfg = config::WtConfig::load()?;
    let mut registry = ports::PortRegistry::load()?;

    let wt_path = cfg.worktree_path(branch);

    // Remove worktree
    if worktree::exists(&wt_path) {
        println!("Removing worktree at {}...", wt_path.display());
        worktree::remove(&wt_path, force)?;
    } else {
        println!("Worktree not found at {}, cleaning up registry...", wt_path.display());
    }

    // Free port
    match registry.free(branch) {
        Ok(port) => {
            println!("Freed port {}", port);
            registry.save()?;
        }
        Err(_) => {
            println!("No port allocation found for {}", branch);
        }
    }

    // Emit to Hydra
    hydra::emit_worktree_removed(branch)?;

    println!("Worktree '{}' removed", branch);

    Ok(())
}

fn cmd_status(branch: Option<&str>) -> Result<()> {
    let cfg = config::WtConfig::load()?;
    let registry = ports::PortRegistry::load()?;

    match branch {
        Some(b) => {
            // Show specific branch
            let wt_path = cfg.worktree_path(b);
            let port = registry.get(b);

            println!("Branch: {}", b);
            println!("  Path: {}", wt_path.display());
            println!("  Port: {}", port.map(|p| p.to_string()).unwrap_or_else(|| "not allocated".to_string()));
            println!("  Exists: {}", worktree::exists(&wt_path));

            // Show git info if exists
            if worktree::exists(&wt_path) {
                let worktrees = worktree::list()?;
                if let Some(wt) = worktrees.iter().find(|w| w.path == wt_path.to_string_lossy()) {
                    println!("  HEAD: {}", &wt.head[..8.min(wt.head.len())]);
                    if let Some(ref branch) = wt.branch {
                        println!("  Branch: {}", branch);
                    }
                }
            }
        }
        None => {
            // Summary
            let total = registry.allocations.len();
            let existing = registry
                .allocations
                .keys()
                .filter(|b| worktree::exists(&cfg.worktree_path(b)))
                .count();

            println!("hydra-wt status");
            println!("  Total managed: {}", total);
            println!("  Existing: {}", existing);
            println!("  Missing: {}", total - existing);
            println!("  Port range: {}-{}", cfg.ports.range_start, cfg.ports.range_end);
            println!("  Ports used: {}", total);
            println!("  Ports free: {}", (cfg.ports.range_end - cfg.ports.range_start + 1) as usize - total);
        }
    }

    Ok(())
}
