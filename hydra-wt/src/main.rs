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

    /// Merge a worktree branch into another
    Merge {
        /// Source branch to merge from
        source: String,

        /// Target branch to merge into
        target: String,

        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,

        /// Create merge commit even for fast-forward
        #[arg(long)]
        no_ff: bool,

        /// Preview without merging
        #[arg(long)]
        dry_run: bool,

        /// Remove source worktree after successful merge
        #[arg(long)]
        cleanup: bool,
    },

    /// Abort an in-progress merge
    MergeAbort {
        /// Branch with in-progress merge
        branch: String,
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
        Commands::Merge {
            source,
            target,
            force,
            no_ff,
            dry_run,
            cleanup,
        } => cmd_merge(&source, &target, force, no_ff, dry_run, cleanup),
        Commands::MergeAbort { branch } => cmd_merge_abort(&branch),
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

    // Detect main branch
    let main_branch = detect_main_branch();

    println!(
        "{:<20} {:<6} {:<25} {:<10} {:<20}",
        "BRANCH", "PORT", "PATH", "STATUS", "COMMITS AHEAD"
    );
    println!("{}", "-".repeat(85));

    for (branch, port) in registry.list() {
        let wt_path = cfg.worktree_path(branch);
        let status = if worktree::exists(&wt_path) {
            "exists"
        } else {
            "missing"
        };

        // Get commits ahead of main (if not the main branch itself)
        let commits_info = if *branch != main_branch {
            match worktree::commits_ahead(branch, &main_branch) {
                Ok(commits) if commits.is_empty() => "up to date".to_string(),
                Ok(commits) => {
                    // Check if can merge without conflicts
                    let can_merge_status = if worktree::exists(&wt_path) {
                        match worktree::can_merge(&wt_path, &main_branch) {
                            Ok(true) => "",
                            Ok(false) => " (conflicts)",
                            Err(_) => "",
                        }
                    } else {
                        ""
                    };
                    format!("{}{}", commits.len(), can_merge_status)
                }
                Err(_) => "-".to_string(),
            }
        } else {
            "-".to_string()
        };

        println!(
            "{:<20} {:<6} {:<25} {:<10} {:<20}",
            branch,
            port,
            wt_path.display(),
            status,
            commits_info
        );
    }

    Ok(())
}

fn detect_main_branch() -> String {
    // Try common main branch names
    for branch in &["main", "master"] {
        if worktree::branch_exists(branch).unwrap_or(false) {
            return branch.to_string();
        }
    }
    // Fallback
    "main".to_string()
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

fn cmd_merge(
    source: &str,
    target: &str,
    force: bool,
    no_ff: bool,
    dry_run: bool,
    cleanup: bool,
) -> Result<()> {
    let cfg = config::WtConfig::load()?;
    let registry = ports::PortRegistry::load()?;

    // Validate: cannot merge branch into itself
    if source == target {
        anyhow::bail!("Cannot merge branch '{}' into itself", source);
    }

    // Validate source branch exists
    if !worktree::branch_exists(source)? {
        anyhow::bail!("Source branch '{}' does not exist", source);
    }

    // Validate target branch exists
    if !worktree::branch_exists(target)? {
        anyhow::bail!("Target branch '{}' does not exist", target);
    }

    // Get target worktree path (could be main repo or a worktree)
    let target_path = match worktree::get_worktree_path(target)? {
        Some(path) => path,
        None => {
            // Check if target is the current branch in the main repo
            let repo_root = config::get_repo_root()?;
            let current = worktree::get_current_branch(&repo_root)?;
            if current == target {
                repo_root
            } else {
                anyhow::bail!(
                    "Target branch '{}' is not checked out in any worktree. \
                    Create a worktree first with: hydra-wt create {}",
                    target,
                    target
                );
            }
        }
    };

    // Check for uncommitted changes in target
    if worktree::has_uncommitted_changes(&target_path)? {
        anyhow::bail!(
            "Target worktree has uncommitted changes. \
            Commit or stash changes first:\n  cd {} && git status",
            target_path.display()
        );
    }

    // Check for merge in progress
    if worktree::is_merge_in_progress(&target_path) {
        anyhow::bail!(
            "A merge is already in progress in {}.\n\
            Complete it with: cd {} && git commit\n\
            Or abort with: hydra-wt merge-abort {}",
            target_path.display(),
            target_path.display(),
            target
        );
    }

    // Get commits ahead
    let commits = worktree::commits_ahead(source, target)?;

    if commits.is_empty() {
        println!("Already up to date. Nothing to merge.");
        return Ok(());
    }

    // Show preview
    println!("Merge preview: {} → {}", source, target);
    println!("{} commit(s) to merge:\n", commits.len());
    for commit in &commits {
        println!(
            "  {} {}",
            &commit.hash[..7.min(commit.hash.len())],
            commit.message
        );
    }
    println!();

    if dry_run {
        // Check if merge would have conflicts
        let can_merge = worktree::can_merge(&target_path, source)?;
        if can_merge {
            println!("✓ Merge can proceed without conflicts");
        } else {
            println!("⚠️  Merge would have conflicts");
        }
        return Ok(());
    }

    // Confirm unless --force
    if !force {
        print!("Proceed with merge? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Merge cancelled.");
            return Ok(());
        }
    }

    // Emit merge started event
    hydra::emit_merge_started(source, target, commits.len())?;

    // Perform the merge
    println!("Merging {} into {}...", source, target);
    let result = worktree::merge(&target_path, source, no_ff)?;

    match result {
        worktree::MergeResult::Success { merge_commit } => {
            println!(
                "✓ Merge successful (commit: {})",
                &merge_commit[..7.min(merge_commit.len())]
            );
            hydra::emit_merge_completed(source, target, &merge_commit)?;
        }
        worktree::MergeResult::FastForward { new_head } => {
            println!(
                "✓ Fast-forward merge (head: {})",
                &new_head[..7.min(new_head.len())]
            );
            hydra::emit_merge_completed(source, target, &new_head)?;
        }
        worktree::MergeResult::Conflict { files } => {
            println!("\n⚠️  Merge conflict in {} file(s):", files.len());
            for file in &files {
                println!("  - {}", file);
            }
            println!("\nResolve conflicts in: {}", target_path.display());
            println!("Then run: cd {} && git add . && git commit", target_path.display());
            println!("Or abort: hydra-wt merge-abort {}", target);

            hydra::emit_merge_conflict(source, target, &target_path.to_string_lossy(), &files)?;
            return Ok(());
        }
        worktree::MergeResult::NothingToMerge => {
            println!("Already up to date. Nothing to merge.");
            return Ok(());
        }
    }

    // Cleanup if requested
    if cleanup {
        println!("\nCleaning up source worktree...");
        let source_wt_path = cfg.worktree_path(source);

        if worktree::exists(&source_wt_path) {
            worktree::remove(&source_wt_path, true)?;

            // Free port if allocated
            let mut registry = registry;
            if let Ok(port) = registry.free(source) {
                registry.save()?;
                println!("Removed worktree '{}' and freed port {}", source, port);
            } else {
                println!("Removed worktree '{}'", source);
            }

            hydra::emit_worktree_removed(source)?;
        } else {
            println!("Source worktree '{}' not found (may not be managed by hydra-wt)", source);
        }
    }

    Ok(())
}

fn cmd_merge_abort(branch: &str) -> Result<()> {
    let _cfg = config::WtConfig::load()?;

    // Find the worktree for this branch
    let target_path = match worktree::get_worktree_path(branch)? {
        Some(path) => path,
        None => {
            // Check if it's the current branch in main repo
            let repo_root = config::get_repo_root()?;
            let current = worktree::get_current_branch(&repo_root)?;
            if current == branch {
                repo_root
            } else {
                anyhow::bail!(
                    "Branch '{}' is not checked out in any worktree",
                    branch
                );
            }
        }
    };

    // Check if merge is in progress
    if !worktree::is_merge_in_progress(&target_path) {
        anyhow::bail!("No merge in progress in '{}'", branch);
    }

    // Abort the merge
    worktree::merge_abort(&target_path)?;
    println!("Merge aborted in '{}'", branch);

    Ok(())
}
