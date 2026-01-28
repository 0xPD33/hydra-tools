// ═══════════════════════════════════════════════════════════════════════════
// Session Types
// ═══════════════════════════════════════════════════════════════════════════

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

/// Unique session identifier (short UUID)
#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionId(pub String);

impl SessionId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string()[..8].to_string())
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Configuration for a new session
#[derive(Clone, Debug)]
pub struct SessionConfig {
    pub max_iterations: u32,
    pub max_duration: Duration,
    pub agent_cli: String,
    pub agent_flags: String,
    pub prd_path: PathBuf,
    pub working_dir: PathBuf,
    pub use_worktree: bool,
    pub branch_name: Option<String>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            max_duration: Duration::from_secs(4 * 3600), // 4 hours
            agent_cli: "claude".into(),
            agent_flags: "--dangerously-skip-permissions".into(),
            prd_path: PathBuf::from(".hydra/ralph/prd.json"),
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            use_worktree: false,
            branch_name: None,
        }
    }
}

/// Current state of a session
#[derive(Clone, Debug)]
pub enum SessionState {
    Starting,
    Running { iteration: u32, stories: String },
    Paused,
    Completed { iterations: u32 },
    Blocked { iteration: u32, reason: String },
    MaxIterations { iterations: u32 },
    Failed { reason: String },
    Stuck { since: Instant, last_iteration: u32 },
}

/// A running hydralph session
#[derive(Debug)]
pub struct Session {
    pub id: SessionId,
    pub config: SessionConfig,
    pub state: SessionState,
    pub tmux_session: String,
    pub worktree_path: Option<PathBuf>,
    pub allocated_port: Option<u16>,  // Allocated by hydra-wt
    pub started_at: Instant,
    pub last_activity: Instant,
}

/// Persistent session metadata stored on disk
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub tmux_session: String,
    pub prd_path: PathBuf,
    pub max_iterations: u32,
    pub max_duration_secs: u64,
    pub agent_cli: String,
    pub agent_flags: String,
    pub working_dir: PathBuf,
    pub use_worktree: bool,
    pub branch_name: Option<String>,
    pub worktree_path: Option<PathBuf>,
    pub allocated_port: Option<u16>,
    pub created_at: u64,
}

impl SessionRecord {
    pub fn from_session(session: &Session) -> Self {
        Self {
            id: session.id.0.clone(),
            tmux_session: session.tmux_session.clone(),
            prd_path: session.config.prd_path.clone(),
            max_iterations: session.config.max_iterations,
            max_duration_secs: session.config.max_duration.as_secs(),
            agent_cli: session.config.agent_cli.clone(),
            agent_flags: session.config.agent_flags.clone(),
            working_dir: session.config.working_dir.clone(),
            use_worktree: session.config.use_worktree,
            branch_name: session.config.branch_name.clone(),
            worktree_path: session.worktree_path.clone(),
            allocated_port: session.allocated_port,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    pub fn into_session(self) -> Session {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let elapsed = now.saturating_sub(self.created_at);
        let started_at = Instant::now() - Duration::from_secs(elapsed);

        let config = SessionConfig {
            max_iterations: self.max_iterations,
            max_duration: Duration::from_secs(self.max_duration_secs),
            agent_cli: self.agent_cli,
            agent_flags: self.agent_flags,
            prd_path: self.prd_path,
            working_dir: self.working_dir,
            use_worktree: self.use_worktree,
            branch_name: self.branch_name,
        };

        Session {
            id: SessionId(self.id),
            config,
            state: SessionState::Starting,
            tmux_session: self.tmux_session,
            worktree_path: self.worktree_path,
            allocated_port: self.allocated_port,
            started_at,
            last_activity: started_at,
        }
    }
}

/// Lightweight status for CLI display
#[derive(Clone, Debug)]
pub struct SessionStatus {
    pub id: String,
    pub state: String,
    pub duration: Duration,
    pub tmux: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_creation() {
        let id1 = SessionId::new();
        let id2 = SessionId::new();
        assert_eq!(id1.0.len(), 8);
        assert_eq!(id2.0.len(), 8);
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_session_id_display() {
        let id = SessionId("abc12345".to_string());
        assert_eq!(format!("{}", id), "abc12345");
    }

    #[test]
    fn test_session_config_default() {
        let config = SessionConfig::default();
        assert_eq!(config.max_iterations, 10);
        assert_eq!(config.max_duration, Duration::from_secs(4 * 3600));
        assert_eq!(config.agent_cli, "claude");
        assert_eq!(config.agent_flags, "--dangerously-skip-permissions");
        assert_eq!(config.prd_path, PathBuf::from(".hydra/ralph/prd.json"));
        assert!(!config.use_worktree);
        assert_eq!(config.branch_name, None);
    }

    #[test]
    fn test_session_record_serialization() {
        let record = SessionRecord {
            id: "test123".to_string(),
            tmux_session: "hydra-test123".to_string(),
            prd_path: PathBuf::from(".hydra/ralph/prd.json"),
            max_iterations: 5,
            max_duration_secs: 7200,
            agent_cli: "claude".to_string(),
            agent_flags: "--test".to_string(),
            working_dir: PathBuf::from("/tmp/test"),
            use_worktree: true,
            branch_name: Some("feature".to_string()),
            worktree_path: Some(PathBuf::from("/tmp/wt")),
            allocated_port: Some(3001),
            created_at: 1234567890,
        };

        let json = serde_json::to_string(&record).unwrap();
        let deserialized: SessionRecord = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, "test123");
        assert_eq!(deserialized.max_iterations, 5);
        assert_eq!(deserialized.allocated_port, Some(3001));
    }

    #[test]
    fn test_session_record_roundtrip() {
        let config = SessionConfig {
            max_iterations: 3,
            max_duration: Duration::from_secs(1800),
            agent_cli: "test-agent".to_string(),
            agent_flags: "--flag".to_string(),
            prd_path: PathBuf::from("prd.json"),
            working_dir: PathBuf::from("/work"),
            use_worktree: false,
            branch_name: None,
        };

        let session = Session {
            id: SessionId("abc12345".to_string()),
            config,
            state: SessionState::Starting,
            tmux_session: "hydra-abc12345".to_string(),
            worktree_path: None,
            allocated_port: None,
            started_at: Instant::now(),
            last_activity: Instant::now(),
        };

        let record = SessionRecord::from_session(&session);
        assert_eq!(record.id, "abc12345");
        assert_eq!(record.max_iterations, 3);

        let restored = record.into_session();
        assert_eq!(restored.id.0, "abc12345");
        assert_eq!(restored.config.max_iterations, 3);
    }

    #[test]
    fn test_session_record_with_worktree() {
        let record = SessionRecord {
            id: "test123".to_string(),
            tmux_session: "hydra-test123".to_string(),
            prd_path: PathBuf::from(".hydra/ralph/prd.json"),
            max_iterations: 5,
            max_duration_secs: 7200,
            agent_cli: "claude".to_string(),
            agent_flags: "--test".to_string(),
            working_dir: PathBuf::from("/tmp/test"),
            use_worktree: true,
            branch_name: Some("feature-branch".to_string()),
            worktree_path: Some(PathBuf::from("/tmp/test-wt")),
            allocated_port: Some(3005),
            created_at: 1234567890,
        };

        let session = record.clone().into_session();
        assert_eq!(session.worktree_path, Some(PathBuf::from("/tmp/test-wt")));
        assert_eq!(session.allocated_port, Some(3005));
        assert_eq!(session.config.branch_name, Some("feature-branch".to_string()));
        assert!(session.config.use_worktree);
    }

    #[test]
    fn test_session_record_invalid_json() {
        let invalid_json = r#"{"id": "test", "invalid_field": true}"#;
        let result: Result<SessionRecord, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_session_config_custom_values() {
        let config = SessionConfig {
            max_iterations: 100,
            max_duration: Duration::from_secs(86400), // 24 hours
            agent_cli: "custom-agent".to_string(),
            agent_flags: "--flag1 --flag2".to_string(),
            prd_path: PathBuf::from("custom/prd.json"),
            working_dir: PathBuf::from("/custom/dir"),
            use_worktree: true,
            branch_name: Some("custom-branch".to_string()),
        };

        assert_eq!(config.max_iterations, 100);
        assert_eq!(config.max_duration.as_secs(), 86400);
        assert!(config.use_worktree);
    }

    #[test]
    fn test_session_id_uniqueness() {
        let ids: Vec<SessionId> = (0..100).map(|_| SessionId::new()).collect();
        let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
        // All 100 IDs should be unique
        assert_eq!(unique_ids.len(), 100);
    }

    #[test]
    fn test_session_state_variants() {
        let states = vec![
            SessionState::Starting,
            SessionState::Running { iteration: 1, stories: "test".to_string() },
            SessionState::Paused,
            SessionState::Completed { iterations: 5 },
            SessionState::Blocked { iteration: 3, reason: "error".to_string() },
            SessionState::MaxIterations { iterations: 10 },
            SessionState::Failed { reason: "crash".to_string() },
            SessionState::Stuck { since: Instant::now(), last_iteration: 7 },
        ];

        // Just verify all variants can be constructed
        assert_eq!(states.len(), 8);
    }
}
