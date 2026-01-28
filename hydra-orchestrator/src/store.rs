// ═══════════════════════════════════════════════════════════════════════════
// Session Store - Persist orchestrator sessions to disk
// ═══════════════════════════════════════════════════════════════════════════

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::session::{SessionId, SessionRecord};

pub struct SessionStore {
    root: PathBuf,
}

impl SessionStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn dir(&self) -> PathBuf {
        self.root.join(".hydra/orchestrator/sessions")
    }

    pub fn ensure_dir(&self) -> Result<()> {
        let dir = self.dir();
        fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create {}", dir.display()))?;
        Ok(())
    }

    pub fn record_path(&self, id: &SessionId) -> PathBuf {
        self.dir().join(format!("{}.json", id.0))
    }

    pub fn save(&self, record: &SessionRecord) -> Result<()> {
        self.ensure_dir()?;
        let path = self.record_path(&SessionId(record.id.clone()));
        let content = serde_json::to_string_pretty(record)
            .context("Failed to serialize session record")?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write {}", path.display()))?;
        Ok(())
    }

    pub fn load(&self, id: &SessionId) -> Result<Option<SessionRecord>> {
        let path = self.record_path(id);
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let record = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        Ok(Some(record))
    }

    pub fn list(&self) -> Result<Vec<SessionRecord>> {
        let dir = self.dir();
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut records = Vec::new();
        for entry in fs::read_dir(&dir)
            .with_context(|| format!("Failed to read {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let content = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            let record: SessionRecord = serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse {}", path.display()))?;
            records.push(record);
        }
        Ok(records)
    }

    pub fn remove(&self, id: &SessionId) -> Result<()> {
        let path = self.record_path(id);
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to remove {}", path.display()))?;
        }
        Ok(())
    }
}

pub fn find_project_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Prefer git root over arbitrary .hydra directories
    if let Some(root) = git_root(&cwd) {
        // Within git repo, look for .hydra directory
        let mut dir = root.clone();
        loop {
            if dir.join(".hydra").exists() {
                return dir;
            }
            if !dir.pop() || dir.starts_with(root.parent().unwrap_or(root.as_path())) {
                break;
            }
        }
        return root;
    }

    // Fallback: look for .hydra directory without git
    let mut dir = cwd.clone();
    loop {
        if dir.join(".hydra").exists() {
            return dir;
        }
        if !dir.pop() {
            break;
        }
    }

    cwd
}

fn git_root(cwd: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["-C", &cwd.to_string_lossy(), "rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if root.is_empty() {
        None
    } else {
        Some(PathBuf::from(root))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_store_paths() {
        let store = SessionStore::new(PathBuf::from("/tmp/test"));
        assert_eq!(store.root(), Path::new("/tmp/test"));
        assert_eq!(store.dir(), PathBuf::from("/tmp/test/.hydra/orchestrator/sessions"));
    }

    #[test]
    fn test_record_path() {
        let store = SessionStore::new(PathBuf::from("/tmp/test"));
        let id = SessionId("abc12345".to_string());
        let path = store.record_path(&id);
        assert_eq!(path, PathBuf::from("/tmp/test/.hydra/orchestrator/sessions/abc12345.json"));
    }

    #[test]
    fn test_save_and_load_record() {
        use crate::session::SessionRecord;

        let temp_dir = std::env::temp_dir().join(format!("hydra-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let store = SessionStore::new(temp_dir.clone());
        let record = SessionRecord {
            id: "test123".to_string(),
            tmux_session: "hydra-test123".to_string(),
            prd_path: PathBuf::from("prd.json"),
            max_iterations: 5,
            max_duration_secs: 3600,
            agent_cli: "claude".to_string(),
            agent_flags: "--test".to_string(),
            working_dir: PathBuf::from("/work"),
            use_worktree: false,
            branch_name: None,
            worktree_path: None,
            allocated_port: None,
            created_at: 1234567890,
        };

        store.save(&record).unwrap();

        let loaded = store.load(&SessionId("test123".to_string())).unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, "test123");
        assert_eq!(loaded.max_iterations, 5);

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_load_nonexistent_record() {
        let temp_dir = std::env::temp_dir().join(format!("hydra-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let store = SessionStore::new(temp_dir.clone());
        let result = store.load(&SessionId("nonexistent".to_string())).unwrap();
        assert!(result.is_none());

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_list_records() {
        use crate::session::SessionRecord;

        let temp_dir = std::env::temp_dir().join(format!("hydra-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let store = SessionStore::new(temp_dir.clone());

        // Save multiple records
        for i in 1..=3 {
            let record = SessionRecord {
                id: format!("test{}", i),
                tmux_session: format!("hydra-test{}", i),
                prd_path: PathBuf::from("prd.json"),
                max_iterations: 5,
                max_duration_secs: 3600,
                agent_cli: "claude".to_string(),
                agent_flags: "--test".to_string(),
                working_dir: PathBuf::from("/work"),
                use_worktree: false,
                branch_name: None,
                worktree_path: None,
                allocated_port: None,
                created_at: 1234567890,
            };
            store.save(&record).unwrap();
        }

        let records = store.list().unwrap();
        assert_eq!(records.len(), 3);

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_remove_record() {
        use crate::session::SessionRecord;

        let temp_dir = std::env::temp_dir().join(format!("hydra-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let store = SessionStore::new(temp_dir.clone());
        let record = SessionRecord {
            id: "test123".to_string(),
            tmux_session: "hydra-test123".to_string(),
            prd_path: PathBuf::from("prd.json"),
            max_iterations: 5,
            max_duration_secs: 3600,
            agent_cli: "claude".to_string(),
            agent_flags: "--test".to_string(),
            working_dir: PathBuf::from("/work"),
            use_worktree: false,
            branch_name: None,
            worktree_path: None,
            allocated_port: None,
            created_at: 1234567890,
        };

        store.save(&record).unwrap();
        assert!(store.load(&SessionId("test123".to_string())).unwrap().is_some());

        store.remove(&SessionId("test123".to_string())).unwrap();
        assert!(store.load(&SessionId("test123".to_string())).unwrap().is_none());

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_remove_nonexistent_record() {
        let temp_dir = std::env::temp_dir().join(format!("hydra-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let store = SessionStore::new(temp_dir.clone());
        // Should not error when removing nonexistent record
        let result = store.remove(&SessionId("nonexistent".to_string()));
        assert!(result.is_ok());

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_list_empty_directory() {
        let temp_dir = std::env::temp_dir().join(format!("hydra-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let store = SessionStore::new(temp_dir.clone());
        let records = store.list().unwrap();
        assert_eq!(records.len(), 0);

        std::fs::remove_dir_all(&temp_dir).ok();
    }
}
