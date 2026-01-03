use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// A single log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub project_uuid: Uuid,
    pub channel: String,
    pub message: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Append-only message log for crash recovery
pub struct MessageLog {
    path: PathBuf,
    file: File,
}

impl MessageLog {
    /// Open or create message log
    pub fn open(log_path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .context("Failed to open message log")?;

        Ok(Self {
            path: log_path.to_path_buf(),
            file,
        })
    }

    /// Append a message to the log
    pub fn append(&mut self, project_uuid: Uuid, channel: &str, message: &str) -> Result<()> {
        let entry = LogEntry {
            project_uuid,
            channel: channel.to_string(),
            message: message.to_string(),
            timestamp: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&entry).context("Failed to serialize log entry")?;
        writeln!(self.file, "{}", json).context("Failed to write to log")?;
        self.file.flush().context("Failed to flush log")?;

        Ok(())
    }

    /// Replay all log entries
    pub fn replay(&self) -> Result<Vec<LogEntry>> {
        let file = File::open(&self.path).context("Failed to open log for replay")?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for line in reader.lines() {
            let line = line.context("Failed to read log line")?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: LogEntry = serde_json::from_str(&line)
                .context("Failed to parse log entry")?;
            entries.push(entry);
        }

        Ok(entries)
    }

    /// Compact log to keep only last N messages per channel
    pub fn compact(&self, keep_per_channel: usize) -> Result<()> {
        use std::collections::HashMap;

        let entries = self.replay()?;

        // Group by (project_uuid, channel)
        let mut by_channel: HashMap<(Uuid, String), Vec<LogEntry>> = HashMap::new();
        for entry in entries {
            let key = (entry.project_uuid, entry.channel.clone());
            by_channel.entry(key).or_default().push(entry);
        }

        // Keep only last N per channel
        let mut kept_entries = Vec::new();
        for (_key, mut entries) in by_channel {
            entries.sort_by_key(|e| e.timestamp);
            let start = entries.len().saturating_sub(keep_per_channel);
            kept_entries.extend(entries.into_iter().skip(start));
        }

        // Sort by timestamp for replay order
        kept_entries.sort_by_key(|e| e.timestamp);

        // Write compacted log
        let temp_path = self.path.with_extension("tmp");
        let mut temp_file = File::create(&temp_path)
            .context("Failed to create temp log file")?;

        for entry in kept_entries {
            let json = serde_json::to_string(&entry)?;
            writeln!(temp_file, "{}", json)?;
        }
        temp_file.flush()?;
        drop(temp_file);

        // Atomic rename
        std::fs::rename(&temp_path, &self.path)
            .context("Failed to replace log with compacted version")?;

        Ok(())
    }
}
