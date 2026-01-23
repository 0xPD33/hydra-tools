use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;

/// Result of a merge operation
#[derive(Debug)]
pub enum MergeResult {
    /// Merge completed with a merge commit
    Success { merge_commit: String },
    /// Fast-forward merge (no merge commit needed)
    FastForward { new_head: String },
    /// Merge has conflicts that need resolution
    Conflict { files: Vec<String> },
    /// Nothing to merge (already up to date)
    NothingToMerge,
}

/// Information about a commit
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub date: String,
}

/// Get commits that source has but target doesn't
pub fn commits_ahead(source: &str, target: &str) -> Result<Vec<CommitInfo>> {
    let output = Command::new("git")
        .args(["log", &format!("{}..{}", target, source), "--format=%H|%s|%an|%ai"])
        .output()
        .context("Failed to run git log")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git log failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let commits: Vec<CommitInfo> = stdout
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(4, '|').collect();
            if parts.len() >= 4 {
                Some(CommitInfo {
                    hash: parts[0].to_string(),
                    message: parts[1].to_string(),
                    author: parts[2].to_string(),
                    date: parts[3].to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(commits)
}

/// Get the merge base (common ancestor) of two branches
pub fn merge_base(source: &str, target: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["merge-base", source, target])
        .output()
        .context("Failed to run git merge-base")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git merge-base failed: {}", stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Check if a merge would have conflicts (dry-run)
pub fn can_merge(target_path: &Path, source: &str) -> Result<bool> {
    // Try a merge with --no-commit to see if it would succeed
    let output = Command::new("git")
        .args(["-C", &target_path.to_string_lossy(), "merge", "--no-commit", "--no-ff", source])
        .output()
        .context("Failed to run git merge --no-commit")?;

    // Abort the merge attempt regardless of outcome
    let _ = Command::new("git")
        .args(["-C", &target_path.to_string_lossy(), "merge", "--abort"])
        .output();

    Ok(output.status.success())
}

/// Perform a merge
pub fn merge(target_path: &Path, source: &str, no_ff: bool) -> Result<MergeResult> {
    // Check if already up to date
    let commits = commits_ahead(source, &get_current_branch(target_path)?)?;
    if commits.is_empty() {
        return Ok(MergeResult::NothingToMerge);
    }

    let mut args = vec!["-C", &target_path.to_string_lossy().into_owned()];
    let path_str = target_path.to_string_lossy().into_owned();
    args = vec!["-C", &path_str, "merge"];
    if no_ff {
        args.push("--no-ff");
    }
    args.push(source);

    let output = Command::new("git")
        .args(&args)
        .output()
        .context("Failed to run git merge")?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Check if it was a fast-forward
        if stdout.contains("Fast-forward") {
            let head = get_head_commit(target_path)?;
            return Ok(MergeResult::FastForward { new_head: head });
        }

        let head = get_head_commit(target_path)?;
        return Ok(MergeResult::Success { merge_commit: head });
    }

    // Check for conflicts
    let conflict_files = get_conflict_files(target_path)?;
    if !conflict_files.is_empty() {
        return Ok(MergeResult::Conflict { files: conflict_files });
    }

    // Some other error
    let stderr = String::from_utf8_lossy(&output.stderr);
    bail!("git merge failed: {}", stderr.trim());
}

/// Abort an in-progress merge
pub fn merge_abort(target_path: &Path) -> Result<()> {
    let output = Command::new("git")
        .args(["-C", &target_path.to_string_lossy(), "merge", "--abort"])
        .output()
        .context("Failed to run git merge --abort")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git merge --abort failed: {}", stderr.trim());
    }

    Ok(())
}

/// Check if worktree has uncommitted changes
pub fn has_uncommitted_changes(path: &Path) -> Result<bool> {
    let output = Command::new("git")
        .args(["-C", &path.to_string_lossy(), "status", "--porcelain"])
        .output()
        .context("Failed to run git status")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git status failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(!stdout.trim().is_empty())
}

/// Get files with merge conflicts
pub fn get_conflict_files(path: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["-C", &path.to_string_lossy(), "status", "--porcelain"])
        .output()
        .context("Failed to run git status")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let conflicts: Vec<String> = stdout
        .lines()
        .filter(|line| line.starts_with("UU") || line.starts_with("AA") ||
                       line.starts_with("DD") || line.starts_with("UD") ||
                       line.starts_with("DU") || line.starts_with("AU") ||
                       line.starts_with("UA"))
        .map(|line| line[3..].to_string())
        .collect();

    Ok(conflicts)
}

/// Check if a merge is in progress
pub fn is_merge_in_progress(path: &Path) -> bool {
    path.join(".git").join("MERGE_HEAD").exists() ||
    path.join(".git").is_file() && {
        // For worktrees, .git is a file pointing to the actual git dir
        if let Ok(content) = std::fs::read_to_string(path.join(".git")) {
            if let Some(git_dir) = content.strip_prefix("gitdir: ") {
                let git_dir = git_dir.trim();
                Path::new(git_dir).join("MERGE_HEAD").exists()
            } else {
                false
            }
        } else {
            false
        }
    }
}

/// Get the current branch name for a worktree
pub fn get_current_branch(path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["-C", &path.to_string_lossy(), "rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .context("Failed to run git rev-parse")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git rev-parse failed: {}", stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Get the HEAD commit hash for a path
pub fn get_head_commit(path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["-C", &path.to_string_lossy(), "rev-parse", "HEAD"])
        .output()
        .context("Failed to run git rev-parse HEAD")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git rev-parse HEAD failed: {}", stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Check if a branch exists
pub fn branch_exists(branch: &str) -> Result<bool> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", branch])
        .output()
        .context("Failed to check if branch exists")?;

    Ok(output.status.success())
}

/// Get worktree path for a branch (if it exists as a worktree)
pub fn get_worktree_path(branch: &str) -> Result<Option<std::path::PathBuf>> {
    let worktrees = list()?;
    for wt in worktrees {
        if wt.branch.as_deref() == Some(branch) {
            return Ok(Some(std::path::PathBuf::from(wt.path)));
        }
    }
    Ok(None)
}

pub fn add(path: &Path, branch: &str) -> Result<()> {
    // Check if branch exists
    let branch_exists = Command::new("git")
        .args(["rev-parse", "--verify", branch])
        .output()
        .context("Failed to check if branch exists")?
        .status
        .success();

    let output = if branch_exists {
        // Check out existing branch
        Command::new("git")
            .args(["worktree", "add", &path.to_string_lossy(), branch])
            .output()
            .context("Failed to run git worktree add")?
    } else {
        // Create new branch
        Command::new("git")
            .args(["worktree", "add", "-b", branch, &path.to_string_lossy()])
            .output()
            .context("Failed to run git worktree add -b")?
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git worktree add failed: {}", stderr.trim());
    }

    Ok(())
}

pub fn remove(path: &Path, force: bool) -> Result<()> {
    let path_str = path.to_string_lossy();
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(&path_str);

    let output = Command::new("git")
        .args(&args)
        .output()
        .context("Failed to run git worktree remove")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git worktree remove failed: {}", stderr.trim());
    }

    Ok(())
}

pub fn exists(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }

    // Check if it's actually a git worktree
    let git_dir = path.join(".git");
    git_dir.exists()
}

#[derive(Debug)]
pub struct WorktreeInfo {
    pub path: String,
    pub branch: Option<String>,
    pub head: String,
}

pub fn list() -> Result<Vec<WorktreeInfo>> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .output()
        .context("Failed to run git worktree list")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git worktree list failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut worktrees = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_head: Option<String> = None;
    let mut current_branch: Option<String> = None;

    for line in stdout.lines() {
        if line.starts_with("worktree ") {
            // Save previous worktree if any
            if let (Some(path), Some(head)) = (current_path.take(), current_head.take()) {
                worktrees.push(WorktreeInfo {
                    path,
                    head,
                    branch: current_branch.take(),
                });
            }
            current_path = Some(line.strip_prefix("worktree ").unwrap().to_string());
        } else if line.starts_with("HEAD ") {
            current_head = Some(line.strip_prefix("HEAD ").unwrap().to_string());
        } else if line.starts_with("branch ") {
            let branch = line.strip_prefix("branch refs/heads/").unwrap_or(
                line.strip_prefix("branch ").unwrap()
            );
            current_branch = Some(branch.to_string());
        }
    }

    // Save last worktree
    if let (Some(path), Some(head)) = (current_path, current_head) {
        worktrees.push(WorktreeInfo {
            path,
            head,
            branch: current_branch,
        });
    }

    Ok(worktrees)
}
