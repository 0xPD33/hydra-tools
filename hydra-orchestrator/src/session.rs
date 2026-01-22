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
