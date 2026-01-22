// ═══════════════════════════════════════════════════════════════════════════
// TMUX Wrapper Functions
// ═══════════════════════════════════════════════════════════════════════════

use std::process::Command;
use anyhow::{Result, Context};

/// Create a new TMUX session
pub fn new_session(name: &str, working_dir: &std::path::Path) -> Result<()> {
    let output = Command::new("tmux")
        .args(["new-session", "-d", "-s", name, "-c", &working_dir.display().to_string()])
        .output()
        .context("Failed to create tmux session. Is tmux installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("tmux new-session failed: {}", stderr);
    }

    Ok(())
}

/// Kill a TMUX session
pub fn kill_session(name: &str) -> Result<()> {
    let output = Command::new("tmux")
        .args(["kill-session", "-t", name])
        .output()?;

    // Don't error if session doesn't exist
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.contains("no such session") {
            anyhow::bail!("tmux kill-session failed: {}", stderr);
        }
    }

    Ok(())
}

/// Send keys to a TMUX session (appends to current input)
pub fn send_keys(name: &str, keys: &str) -> Result<()> {
    let output = Command::new("tmux")
        .args(["send-keys", "-t", name, keys, "C-m"])
        .output()
        .context("Failed to send keys to tmux session")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("tmux send-keys failed: {}", stderr);
    }

    Ok(())
}

/// Check if a TMUX session exists
pub fn session_exists(name: &str) -> Result<bool> {
    let output = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output()?;

    if !output.status.success() {
        // No server running = no sessions
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().any(|line| line == name))
}

/// List all active TMUX sessions
pub fn list_sessions() -> Result<Vec<String>> {
    let output = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            Ok(stdout.lines().map(|s| s.to_string()).collect())
        }
        _ => Ok(vec![]), // No server or error = no sessions
    }
}
