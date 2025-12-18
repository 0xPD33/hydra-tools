//! tmux session monitoring
//!
//! Detects and monitors tmux sessions in attached terminal windows.

use std::process::Command;

/// tmux session monitor
#[derive(Debug)]
pub struct TmuxMonitor {
    /// Currently monitored session name
    pub session: Option<String>,
    /// Cached pane content
    cached_content: Option<String>,
}

impl TmuxMonitor {
    pub fn new() -> Self {
        Self {
            session: None,
            cached_content: None,
        }
    }

    /// Try to detect an active tmux session
    pub fn detect_session(&mut self) -> bool {
        let output = Command::new("tmux")
            .args(["list-sessions", "-F", "#{session_name}"])
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                let sessions = String::from_utf8_lossy(&out.stdout);
                // Take the first session for now
                if let Some(first) = sessions.lines().next() {
                    self.session = Some(first.to_string());
                    tracing::info!("Detected tmux session: {}", first);
                    return true;
                }
            }
        }

        tracing::debug!("No tmux session detected");
        false
    }

    /// Capture current pane content
    pub fn capture_pane(&mut self) -> Option<&str> {
        let session = self.session.as_ref()?;

        let output = Command::new("tmux")
            .args(["capture-pane", "-t", session, "-p"])
            .output()
            .ok()?;

        if output.status.success() {
            self.cached_content = Some(String::from_utf8_lossy(&output.stdout).to_string());
            self.cached_content.as_deref()
        } else {
            None
        }
    }

    /// Get list of panes in the current session
    pub fn list_panes(&self) -> Vec<PaneInfo> {
        let Some(ref session) = self.session else {
            return vec![];
        };

        let output = Command::new("tmux")
            .args([
                "list-panes",
                "-t",
                session,
                "-F",
                "#{pane_id}:#{pane_width}x#{pane_height}:#{pane_active}",
            ])
            .output();

        let Ok(out) = output else {
            return vec![];
        };

        if !out.status.success() {
            return vec![];
        }

        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 3 {
                    let dims: Vec<&str> = parts[1].split('x').collect();
                    Some(PaneInfo {
                        id: parts[0].to_string(),
                        width: dims.get(0).and_then(|s| s.parse().ok()).unwrap_or(0),
                        height: dims.get(1).and_then(|s| s.parse().ok()).unwrap_or(0),
                        active: parts[2] == "1",
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Check if content contains an error pattern
    pub fn has_error(&self) -> bool {
        self.cached_content
            .as_ref()
            .map(|c| {
                let lower = c.to_lowercase();
                lower.contains("error:") || lower.contains("failed") || lower.contains("panic")
            })
            .unwrap_or(false)
    }

    /// Clear the session
    pub fn clear(&mut self) {
        self.session = None;
        self.cached_content = None;
    }
}

impl Default for TmuxMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about a tmux pane
#[derive(Debug, Clone)]
pub struct PaneInfo {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub active: bool,
}
