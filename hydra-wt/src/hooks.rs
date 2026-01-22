//! Hook execution for worktrees
//!
//! Provides post-create hook execution for running setup commands
//! in newly created worktrees.

use anyhow::Result;
use std::path::Path;
use std::process::Command;

/// Run post-create hooks in the worktree directory
///
/// Executes each command via `sh -c` in the worktree directory.
/// Hook failures warn but don't fail - the worktree is already created.
pub fn run_post_create(wt_path: &Path, commands: &[String]) -> Result<()> {
    if commands.is_empty() {
        return Ok(());
    }

    println!("Running post-create hooks...");

    for cmd in commands {
        println!("  Running: {}", cmd);

        let output = Command::new("sh")
            .args(["-c", cmd])
            .current_dir(wt_path)
            .output();

        match output {
            Ok(result) => {
                if !result.status.success() {
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    eprintln!(
                        "Warning: hook '{}' failed: {}",
                        cmd,
                        stderr.trim()
                    );
                }
            }
            Err(e) => {
                eprintln!("Warning: failed to run hook '{}': {}", cmd, e);
            }
        }
    }

    Ok(())
}
