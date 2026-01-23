use anyhow::{Context, Result};
use serde::Serialize;
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Serialize)]
pub struct WorktreeCreatedEvent {
    #[serde(rename = "type")]
    pub event_type: &'static str,
    pub worktree: String,
    pub port: u16,
    pub path: String,
}

#[derive(Serialize)]
pub struct WorktreeRemovedEvent {
    #[serde(rename = "type")]
    pub event_type: &'static str,
    pub worktree: String,
}

#[derive(Serialize)]
pub struct MergeStartedEvent {
    #[serde(rename = "type")]
    pub event_type: &'static str,
    pub source: String,
    pub target: String,
    pub commits: usize,
}

#[derive(Serialize)]
pub struct MergeCompletedEvent {
    #[serde(rename = "type")]
    pub event_type: &'static str,
    pub source: String,
    pub target: String,
    pub merge_commit: String,
}

#[derive(Serialize)]
pub struct MergeConflictEvent {
    #[serde(rename = "type")]
    pub event_type: &'static str,
    pub source: String,
    pub target: String,
    pub target_worktree: String,
    pub conflicted_files: Vec<String>,
}

pub fn emit_merge_started(source: &str, target: &str, commits: usize) -> Result<()> {
    let event = MergeStartedEvent {
        event_type: "merge_started",
        source: source.to_string(),
        target: target.to_string(),
        commits,
    };
    emit("sys:registry", "status", &event)
}

pub fn emit_merge_completed(source: &str, target: &str, merge_commit: &str) -> Result<()> {
    let event = MergeCompletedEvent {
        event_type: "merge_completed",
        source: source.to_string(),
        target: target.to_string(),
        merge_commit: merge_commit.to_string(),
    };
    emit("sys:registry", "status", &event)
}

pub fn emit_merge_conflict(source: &str, target: &str, target_worktree: &str, files: &[String]) -> Result<()> {
    let event = MergeConflictEvent {
        event_type: "merge_conflict",
        source: source.to_string(),
        target: target.to_string(),
        target_worktree: target_worktree.to_string(),
        conflicted_files: files.to_vec(),
    };
    emit("sys:registry", "status", &event)
}

pub fn emit_worktree_created(worktree: &str, port: u16, path: &str) -> Result<()> {
    let event = WorktreeCreatedEvent {
        event_type: "worktree_created",
        worktree: worktree.to_string(),
        port,
        path: path.to_string(),
    };
    emit("sys:registry", "status", &event)
}

pub fn emit_worktree_removed(worktree: &str) -> Result<()> {
    let event = WorktreeRemovedEvent {
        event_type: "worktree_removed",
        worktree: worktree.to_string(),
    };
    emit("sys:registry", "status", &event)
}

fn emit<T: Serialize>(channel: &str, msg_type: &str, data: &T) -> Result<()> {
    let json = serde_json::to_string(data).context("Failed to serialize event")?;

    // Check if hydra-mail is available
    let which = Command::new("which")
        .arg("hydra-mail")
        .output();

    if which.is_err() || !which.unwrap().status.success() {
        eprintln!("Warning: hydra-mail not found, skipping event emission");
        return Ok(());
    }

    let mut child = Command::new("hydra-mail")
        .args(["emit", "--channel", channel, "--type", msg_type, "--data", "@-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn hydra-mail")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(json.as_bytes())
            .context("Failed to write to hydra-mail stdin")?;
    }

    let output = child.wait_with_output().context("Failed to wait for hydra-mail")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Warning: hydra-mail emit failed: {}", stderr.trim());
    }

    Ok(())
}
