//! Artifact handling for worktrees
//!
//! Provides symlink and copy operations to bring artifacts from the repo
//! root into new worktrees.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Symlink an artifact from repo root to worktree
///
/// Creates a symlink at `wt_path/artifact` pointing to `repo_root/artifact`.
/// Warns and skips if source doesn't exist or target already exists.
pub fn symlink_artifact(repo_root: &Path, wt_path: &Path, artifact: &str) -> Result<()> {
    let source = repo_root.join(artifact);
    let target = wt_path.join(artifact);

    // Check source exists
    if !source.exists() {
        eprintln!(
            "Warning: artifact source '{}' not found, skipping symlink",
            source.display()
        );
        return Ok(());
    }

    // Check target doesn't already exist
    if target.exists() || target.is_symlink() {
        eprintln!(
            "Warning: artifact target '{}' already exists, skipping symlink",
            target.display()
        );
        return Ok(());
    }

    // Create parent directories if needed
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create parent dirs for {}", target.display()))?;
    }

    // Create symlink
    #[cfg(unix)]
    std::os::unix::fs::symlink(&source, &target)
        .with_context(|| format!("Failed to symlink {} -> {}", target.display(), source.display()))?;

    #[cfg(windows)]
    {
        if source.is_dir() {
            std::os::windows::fs::symlink_dir(&source, &target)
        } else {
            std::os::windows::fs::symlink_file(&source, &target)
        }
        .with_context(|| format!("Failed to symlink {} -> {}", target.display(), source.display()))?;
    }

    println!("  Symlinked: {} -> {}", artifact, source.display());
    Ok(())
}

/// Copy an artifact from repo root to worktree
///
/// Copies `repo_root/artifact` to `wt_path/artifact`.
/// Uses `cp -a --reflink=auto` for copy-on-write optimization.
/// Warns and skips if source doesn't exist or target already exists.
pub fn copy_artifact(repo_root: &Path, wt_path: &Path, artifact: &str) -> Result<()> {
    let source = repo_root.join(artifact);
    let target = wt_path.join(artifact);

    // Check source exists
    if !source.exists() {
        eprintln!(
            "Warning: artifact source '{}' not found, skipping copy",
            source.display()
        );
        return Ok(());
    }

    // Check target doesn't already exist
    if target.exists() {
        eprintln!(
            "Warning: artifact target '{}' already exists, skipping copy",
            target.display()
        );
        return Ok(());
    }

    // Create parent directories if needed
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create parent dirs for {}", target.display()))?;
    }

    // Use cp for robust copying with COW support
    let output = Command::new("cp")
        .args(["-a", "--reflink=auto"])
        .arg(&source)
        .arg(&target)
        .output()
        .with_context(|| format!("Failed to run cp for {}", artifact))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("cp failed for {}: {}", artifact, stderr.trim());
    }

    println!("  Copied: {}", artifact);
    Ok(())
}
