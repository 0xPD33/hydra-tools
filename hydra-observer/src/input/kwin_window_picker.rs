//! KWin window picker - finds window at cursor position
//!
//! Uses KWin scripting to find which window is under the cursor,
//! excluding the hydra-observer overlay.

use std::process::Command;
use std::time::Duration;

/// Window information from KWin
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub resource_name: String,
    pub caption: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl WindowInfo {
    /// Get a unique ID for this window (hash of name + caption)
    pub fn id(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.resource_name.hash(&mut hasher);
        self.caption.hash(&mut hasher);
        hasher.finish()
    }

    /// Check if this window is likely a terminal emulator
    pub fn is_terminal(&self) -> bool {
        let name = self.resource_name.to_lowercase();
        name.contains("konsole")
            || name.contains("alacritty")
            || name.contains("kitty")
            || name.contains("wezterm")
            || name.contains("foot")
            || name.contains("gnome-terminal")
            || name.contains("xterm")
            || name.contains("terminator")
            || name.contains("tilix")
            || name.contains("urxvt")
            || name == "st"
    }
}

const KWIN_SCRIPT: &str = r#"
var pos = workspace.cursorPos;
var found = null;
var ts = Date.now();

// Get windows in stacking order (top to bottom)
var clients = workspace.stackingOrder;

// Iterate in reverse to check topmost windows first
for (var i = clients.length - 1; i >= 0; i--) {
    var c = clients[i];

    // Skip our overlay
    if (c.resourceName === "hydra-observer") continue;

    // Skip minimized windows
    if (c.minimized) continue;

    // Skip desktop and panels (plasmashell)
    if (c.resourceName === "plasmashell") continue;

    var x = c.x;
    var y = c.y;
    var w = c.width;
    var h = c.height;

    if (pos.x >= x && pos.x < x + w && pos.y >= y && pos.y < y + h) {
        found = c;
        break;  // Found topmost window, stop searching
    }
}

if (found) {
    print("HYDRA_WIN_" + ts + ":" + found.resourceName + ":" + found.caption + ":" + Math.round(found.x) + ":" + Math.round(found.y) + ":" + found.width + ":" + found.height);
} else {
    print("HYDRA_WIN_" + ts + ":NONE");
}
"#;

/// Get the window at the current cursor position (excluding our overlay)
pub fn pick_window() -> Option<WindowInfo> {
    tracing::info!("Finding window at cursor position...");

    // Write script to temp file
    let script_path = "/tmp/hydra-window-picker.js";
    if std::fs::write(script_path, KWIN_SCRIPT).is_err() {
        tracing::error!("Failed to write KWin script");
        return None;
    }

    // Load script
    let load_output = Command::new("qdbus")
        .args([
            "org.kde.KWin",
            "/Scripting",
            "org.kde.kwin.Scripting.loadScript",
            script_path,
        ])
        .output()
        .ok()?;

    if !load_output.status.success() {
        tracing::warn!("Failed to load KWin script");
        return None;
    }

    let script_id = String::from_utf8_lossy(&load_output.stdout).trim().to_string();
    if script_id.is_empty() {
        tracing::warn!("Got empty script ID");
        return None;
    }

    // Run script
    let _ = Command::new("qdbus")
        .args([
            "org.kde.KWin",
            &format!("/Scripting/Script{}", script_id),
            "org.kde.kwin.Script.run",
        ])
        .output();

    // Wait for script to execute
    std::thread::sleep(Duration::from_millis(100));

    // Read output from journal
    let journal_output = Command::new("journalctl")
        .args([
            "--user",
            "-u", "plasma-kwin_wayland.service",
            "-n", "30",
            "--no-pager",
            "-o", "cat",
        ])
        .output()
        .ok()?;

    // Stop script
    let _ = Command::new("qdbus")
        .args([
            "org.kde.KWin",
            &format!("/Scripting/Script{}", script_id),
            "org.kde.kwin.Script.stop",
        ])
        .output();

    // Parse output - find the most recent HYDRA_WIN_ line
    let journal_str = String::from_utf8_lossy(&journal_output.stdout);
    let result_line = journal_str
        .lines()
        .rev()
        .find(|line| line.contains("HYDRA_WIN_"))?;

    tracing::debug!("KWin script output: {}", result_line);

    // Parse: HYDRA_WIN_<ts>:<resourceName>:<caption>:<x>:<y>:<width>:<height>
    // Or:    HYDRA_WIN_<ts>:NONE
    let parts: Vec<&str> = result_line.split(':').collect();

    if parts.len() < 2 {
        tracing::warn!("Invalid script output format");
        return None;
    }

    let resource_name = parts[1].to_string();

    if resource_name == "NONE" {
        tracing::info!("No window found at cursor position");
        return None;
    }

    if parts.len() < 7 {
        tracing::warn!("Invalid script output format: {} parts", parts.len());
        return None;
    }

    // parts[0] = HYDRA_WIN_<timestamp>
    // parts[1] = resourceName
    // parts[2..n-4] = caption (may contain colons)
    // parts[n-4] = x
    // parts[n-3] = y
    // parts[n-2] = width
    // parts[n-1] = height

    // Join middle parts as caption (in case caption contains colons)
    let caption = parts[2..parts.len()-4].join(":");
    let x: i32 = parts[parts.len()-4].parse().unwrap_or(0);
    let y: i32 = parts[parts.len()-3].parse().unwrap_or(0);
    let width: i32 = parts[parts.len()-2].parse().unwrap_or(0);
    let height: i32 = parts[parts.len()-1].parse().unwrap_or(0);

    let info = WindowInfo {
        resource_name,
        caption,
        x,
        y,
        width,
        height,
    };

    tracing::info!(
        "Window at cursor: {} ({}) at ({}, {}) size {}x{}",
        info.caption,
        info.resource_name,
        info.x,
        info.y,
        info.width,
        info.height
    );

    Some(info)
}

/// Check if KWin DBus is available
pub fn is_available() -> bool {
    Command::new("qdbus")
        .args(["org.kde.KWin", "/KWin"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
