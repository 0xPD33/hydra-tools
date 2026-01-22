// ═══════════════════════════════════════════════════════════════════════════
// Hydra Orchestrator - Multi-session agent orchestration
// ═══════════════════════════════════════════════════════════════════════════

mod tmux;
mod session;
mod mail;
mod config;
mod store;

pub use session::{SessionId, SessionConfig, SessionState, Session, SessionStatus};
pub use mail::HydraMailClient;
pub use config::HydralphConfig;
pub use store::find_project_root;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Result, Context};
use store::SessionStore;
use serde::Deserialize;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

// ═══════════════════════════════════════════════════════════════════════════
// Orchestrator
// ═══════════════════════════════════════════════════════════════════════════

pub struct Orchestrator {
    sessions: HashMap<String, Session>,
    mail: Option<HydraMailClient>,
    store: SessionStore,
}

impl Orchestrator {
    pub fn new() -> Self {
        let root = find_project_root();
        let store = SessionStore::new(root);
        let mut orch = Self {
            sessions: HashMap::new(),
            mail: None,
            store,
        };
        if let Err(e) = orch.load_sessions() {
            eprintln!("Warning: Failed to load sessions: {}", e);
        }
        orch
    }

    /// Create orchestrator with hydra-mail integration
    pub fn with_mail(project_root: &Path) -> Result<Self> {
        let mail = HydraMailClient::connect(project_root)
            .ok();  // Graceful degradation if mail not available
        let store = SessionStore::new(project_root.to_path_buf());
        let mut orch = Self {
            sessions: HashMap::new(),
            mail,
            store,
        };
        if let Err(e) = orch.load_sessions() {
            eprintln!("Warning: Failed to load sessions: {}", e);
        }
        // Show mail status
        if orch.mail.is_some() {
            eprintln!("✓ Connected to hydra-mail");
        } else {
            eprintln!("Note: hydra-mail not available - running standalone");
        }
        Ok(orch)
    }

    /// Check if hydra-mail is connected
    pub fn has_mail(&self) -> bool {
        self.mail.is_some()
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Spawn
    // ─────────────────────────────────────────────────────────────────────────

    pub fn spawn(&mut self, config: SessionConfig) -> Result<SessionId> {
        let id = SessionId::new();
        let mut config = config;

        // Worktree integration (if feature enabled)
        let (working_dir, worktree_path, allocated_port, branch_name): (PathBuf, Option<PathBuf>, Option<u16>, Option<String>) = if config.use_worktree {
            #[cfg(feature = "worktree")]
            {
                // Try worktree creation, fallback to main dir if it fails
                match self.try_create_worktree(&id, &config) {
                    Ok(result) => result,
                    Err(e) => {
                        eprintln!("Warning: Worktree creation failed: {}. Using main directory.", e);
                        (self.store.root().to_path_buf(), None, None, None)
                    }
                }
            }
            #[cfg(not(feature = "worktree"))]
            {
                eprintln!("Warning: worktree feature not enabled, running in main directory");
                (self.store.root().to_path_buf(), None, None, None)
            }
        } else {
            (self.store.root().to_path_buf(), None, None, None)
        };

        // Update config to reflect actual working directory and branch
        config.working_dir = working_dir.clone();
        if branch_name.is_some() {
            config.branch_name = branch_name;
        }

        // Ensure .hydra/ralph directory exists
        let ralph_dir = working_dir.join(".hydra/ralph");
        fs::create_dir_all(&ralph_dir)
            .context("Failed to create .hydra/ralph directory")?;

        // Copy hydralph.sh and prompt.md if not present
        self.init_ralph_files(&ralph_dir, &config)?;

        // Create TMUX session
        let tmux_session = format!("hydralph-{}", id.0);
        tmux::new_session(&tmux_session, &working_dir)?;

        // Set environment and start hydralph
        let port_env = if let Some(port) = allocated_port {
            format!(" HYDRALPH_PORT='{}'", port)
        } else {
            String::new()
        };

        let env_cmd = format!(
            "export HYDRALPH_SESSION_ID='{}' \
             HYDRALPH_AGENT='{}' \
             HYDRALPH_FLAGS='{}' \
             HYDRALPH_MAX_ITERATIONS='{}' \
             HYDRALPH_PRD='{}'{}",
            id.0,
            config.agent_cli,
            config.agent_flags,
            config.max_iterations,
            ralph_dir.join("prd.json").display(),
            port_env
        );
        tmux::send_keys(&tmux_session, &env_cmd)?;

        // Start the loop
        let script_path = ralph_dir.join("hydralph.sh").display().to_string();
        tmux::send_keys(&tmux_session, &script_path)?;

        // Track session
        let session = Session {
            id: id.clone(),
            config,
            state: SessionState::Starting,
            tmux_session,
            worktree_path,
            allocated_port,
            started_at: std::time::Instant::now(),
            last_activity: std::time::Instant::now(),
        };
        self.sessions.insert(id.0.clone(), session);

        if let Some(session) = self.sessions.get(&id.0) {
            let record = session::SessionRecord::from_session(session);
            self.store.save(&record)?;
        }

        // Emit to hydra-mail
        if let Err(e) = self.emit("session:spawned", &id) {
            eprintln!("Warning: Failed to emit to hydra-mail: {}", e);
        }

        Ok(id)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Worktree helper
    // ─────────────────────────────────────────────────────────────────────────

    #[cfg(feature = "worktree")]
    fn try_create_worktree(&self, id: &SessionId, config: &SessionConfig) -> Result<(PathBuf, Option<PathBuf>, Option<u16>, Option<String>)> {
        let branch = config.branch_name.clone()
            .unwrap_or_else(|| format!("hydralph/{}", id.0));

        // Initialize port registry if needed
        if let Err(e) = hydra_wt::ports::PortRegistry::init() {
            eprintln!("Warning: Failed to init port registry: {}", e);
        }

        let wt_config = match hydra_wt::config::WtConfig::load() {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("Warning: Failed to load hydra-wt config: {}. Using defaults.", e);
                hydra_wt::config::WtConfig::default()
            }
        };

        let mut registry = hydra_wt::ports::PortRegistry::load()
            .unwrap_or_default();

        // Allocate port
        let port = registry.allocate(&branch, wt_config.ports.range_start, wt_config.ports.range_end)?;

        // Create worktree (base path is project root)
        let wt_base = PathBuf::from(&wt_config.worktrees.directory);
        let wt_path = if wt_base.is_absolute() {
            wt_base.join(&branch)
        } else {
            self.store.root().join(wt_base).join(&branch)
        };
        hydra_wt::worktree::add(&wt_path, &branch)?;

        // Save port allocation
        registry.save()?;

        Ok((wt_path.clone(), Some(wt_path), Some(port), Some(branch)))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Session Control
    // ─────────────────────────────────────────────────────────────────────────

    pub fn kill(&mut self, id: &SessionId, reason: &str) -> Result<()> {
        let session = self.sessions.get_mut(&id.0)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", id.0))?;

        tmux::kill_session(&session.tmux_session)?;
        session.state = SessionState::Failed { reason: reason.to_string() };

        // Cleanup worktree if we created one
        if let Some(wt_path) = session.worktree_path.take() {
            #[cfg(feature = "worktree")]
            {
                // Remove worktree
                if let Err(e) = hydra_wt::worktree::remove(&wt_path, true) {
                    eprintln!("Warning: Failed to remove worktree: {}", e);
                }

                // Free allocated port
                if let Some(_port) = session.allocated_port {
                    let branch_name = session.config.branch_name.clone()
                        .unwrap_or_else(|| format!("hydralph/{}", id.0));
                    let branch = branch_name.as_str();
                    if let Ok(mut registry) = hydra_wt::ports::PortRegistry::load() {
                        if let Ok(freed_port) = registry.free(branch) {
                            eprintln!("Freed port {} for branch '{}'", freed_port, branch);
                            let _ = registry.save();
                        }
                    }
                }

                let _ = wt_path; // Mark as intentionally used
            }
        }

        self.sessions.remove(&id.0);
        self.store.remove(id)?;

        // Emit to hydra-mail
        let _ = self.emit("session:killed", id);

        Ok(())
    }

    pub fn pause(&mut self, id: &SessionId) -> Result<()> {
        let session = self.sessions.get_mut(&id.0)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", id.0))?;
        // Write pause marker file that hydralph checks between iterations
        let pause_path = session.config.working_dir
            .join(".hydra/ralph/.pause");
        fs::write(&pause_path, b"1")
            .context("Failed to write pause marker")?;

        session.state = SessionState::Paused;
        let _ = self.emit("session:paused", id);
        Ok(())
    }

    pub fn resume(&mut self, id: &SessionId) -> Result<()> {
        let session = self.sessions.get_mut(&id.0)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", id.0))?;
        // Remove pause marker file
        let pause_path = session.config.working_dir
            .join(".hydra/ralph/.pause");
        if pause_path.exists() {
            fs::remove_file(&pause_path)
                .context("Failed to remove pause marker")?;
        }

        // Send a key to wake up the session if it's waiting
        tmux::send_keys(&session.tmux_session, "echo 'Resumed...'")?;

        session.state = SessionState::Running { iteration: 0, stories: "unknown".into() };
        let _ = self.emit("session:resumed", id);
        Ok(())
    }

    pub fn inject(&mut self, id: &SessionId, message: &str) -> Result<()> {
        let session = self.sessions.get(&id.0)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", id.0))?;
        let inject_path = session.config.working_dir
            .join(".hydra/ralph/inject.md");
        fs::write(&inject_path, message)
            .context("Failed to write inject.md")?;
        let _ = self.emit("session:injected", id);
        Ok(())
    }

    pub fn attach(&self, id: &SessionId) -> Result<()> {
        if let Some(session) = self.sessions.get(&id.0) {
            // This will replace current process with tmux attach
            let _ = std::process::Command::new("tmux")
                .args(["attach", "-t", &session.tmux_session])
                .exec();
        }
        Err(anyhow::anyhow!("Session not found: {}", id.0))
    }

    pub fn list(&mut self) -> Vec<SessionStatus> {
        if let Err(e) = self.refresh_all_states() {
            eprintln!("Warning: Failed to refresh sessions: {}", e);
        }
        self.sessions.values().map(|s| SessionStatus {
            id: s.id.0.clone(),
            state: format!("{:?}", s.state),
            duration: s.started_at.elapsed(),
            tmux: s.tmux_session.clone(),
        }).collect()
    }

    pub fn get_status(&mut self, id: &SessionId) -> Option<&Session> {
        match self.refresh_state(id) {
            Ok(true) => {}
            Ok(false) => {
                self.sessions.remove(&id.0);
                let _ = self.store.remove(id);
                return None;
            }
            Err(e) => {
                eprintln!("Warning: Failed to refresh session {}: {}", id.0, e);
            }
        }
        self.sessions.get(&id.0)
    }

    fn load_sessions(&mut self) -> Result<()> {
        let records = self.store.list()?;
        self.sessions.clear();
        for record in records {
            let session = record.into_session();
            self.sessions.insert(session.id.0.clone(), session);
        }
        Ok(())
    }

    fn refresh_all_states(&mut self) -> Result<()> {
        let ids: Vec<String> = self.sessions.keys().cloned().collect();
        let mut stale = Vec::new();
        for id in ids {
            if !self.refresh_state(&SessionId(id.clone()))? {
                stale.push(id);
            }
        }
        for id in stale {
            self.sessions.remove(&id);
            let _ = self.store.remove(&SessionId(id));
        }
        Ok(())
    }

    fn refresh_state(&mut self, id: &SessionId) -> Result<bool> {
        let session = match self.sessions.get_mut(&id.0) {
            Some(session) => session,
            None => return Ok(false),
        };

        let session_exists = tmux::session_exists(&session.tmux_session).unwrap_or(false);
        if !session_exists {
            return Ok(false);
        }

        let ralph_dir = session.config.working_dir.join(".hydra/ralph");
        let pause_path = ralph_dir.join(".pause");
        if pause_path.exists() {
            session.state = SessionState::Paused;
            return Ok(true);
        }

        if let Some(status) = read_status(&ralph_dir) {
            session.state = map_status(status);
        } else if matches!(session.state, SessionState::Starting) {
            session.state = SessionState::Running {
                iteration: 0,
                stories: "unknown".into(),
            };
        }

        Ok(true)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Initialization
    // ─────────────────────────────────────────────────────────────────────────

    fn init_ralph_files(&self, ralph_dir: &Path, config: &SessionConfig) -> Result<()> {
        // Ensure prd.json exists
        let prd_path = ralph_dir.join("prd.json");
        if !prd_path.exists() && config.prd_path.exists() {
            fs::copy(&config.prd_path, &prd_path)?;
        }

        // Copy hydralph.sh from hydralph/ directory if not present
        let script_path = ralph_dir.join("hydralph.sh");
        if !script_path.exists() {
            let source = self.store.root().join("hydralph/hydralph.sh");
            if source.exists() {
                fs::copy(&source, &script_path)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = fs::metadata(&script_path)?.permissions();
                    perms.set_mode(0o755);
                    fs::set_permissions(&script_path, perms)?;
                }
            }
        }

        // Copy prompt.md from hydralph/ directory if not present
        let prompt_path = ralph_dir.join("prompt.md");
        if !prompt_path.exists() {
            let source = self.store.root().join("hydralph/prompt.md");
            if source.exists() {
                fs::copy(&source, &prompt_path)?;
            }
        }

        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Hydra-Mail Integration
    // ─────────────────────────────────────────────────────────────────────────

    /// Emit a message to hydra-mail (no-op if mail not available)
    pub fn emit(&self, channel: &str, id: &SessionId) -> Result<()> {
        if let Some(mail) = &self.mail {
            let payload = serde_json::json!({ "session": id.0 }).to_string();
            mail.emit(channel, &payload)?;
        }
        Ok(())
    }

    /// Process incoming mail messages and update session states
    pub async fn process_mail(&mut self) -> Result<()> {
        // For now, this is a stub
        // In Phase 4, we'll have a background task that subscribes
        // to ralph:* channels and updates session states
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Health Monitoring
    // ─────────────────────────────────────────────────────────────────────────

    /// Check health of all sessions, enforcing duration limits and detecting stuck sessions
    pub fn health_check(&mut self) -> Result<Vec<SessionId>> {
        let now = std::time::Instant::now();
        let mut killed = vec![];
        let mut to_kill = vec![];
        let mut stuck_to_emit = vec![];

        for (id, session) in &mut self.sessions {
            // Check duration limit
            if now.duration_since(session.started_at) > session.config.max_duration {
                to_kill.push((SessionId(id.clone()), format!("Duration limit exceeded ({:?})", session.config.max_duration)));
                continue;
            }

            // Check if TMUX session still exists
            let session_exists = tmux::session_exists(&session.tmux_session).unwrap_or(false);
            if !session_exists {
                // Session ended - check if it was complete or failed
                match &session.state {
                    SessionState::Completed { .. } => {
                        // Clean removal
                    }
                    _ => {
                        session.state = SessionState::Failed {
                            reason: "TMUX session ended unexpectedly".into()
                        };
                    }
                }
                continue;
            }

            // Check for stuck (no activity in 15 minutes)
            let stuck_threshold = std::time::Duration::from_secs(15 * 60);
            if now.duration_since(session.last_activity) > stuck_threshold {
                let was_stuck = matches!(session.state, SessionState::Stuck { .. });
                if !was_stuck {
                    let last_iter = match &session.state {
                        SessionState::Running { iteration, .. } => *iteration,
                        _ => 0,
                    };
                    session.state = SessionState::Stuck {
                        since: now,
                        last_iteration: last_iter,
                    };
                    // Queue up emit call (do after borrow ends)
                    stuck_to_emit.push(SessionId(id.clone()));
                }
            }
        }

        // Emit stuck events after borrow ends
        for id in &stuck_to_emit {
            let _ = self.emit("session:stuck", id);
        }

        // Kill sessions that exceeded limits
        for (id, reason) in to_kill {
            if self.kill(&id, &reason).is_ok() {
                killed.push(id);
            }
        }

        Ok(killed)
    }
}

#[derive(Debug, Deserialize)]
struct RalphStatus {
    status: String,
    iteration: u32,
    max: u32,
    #[serde(default)]
    stories: Option<String>,
}

fn read_status(ralph_dir: &Path) -> Option<RalphStatus> {
    let status_path = ralph_dir.join("status.json");
    let content = fs::read_to_string(status_path).ok()?;
    serde_json::from_str(&content).ok()
}

fn map_status(status: RalphStatus) -> SessionState {
    match status.status.as_str() {
        "running" => SessionState::Running {
            iteration: status.iteration,
            stories: status.stories.unwrap_or_else(|| "unknown".into()),
        },
        "complete" => SessionState::Completed {
            iterations: status.iteration,
        },
        "blocked" => SessionState::Blocked {
            iteration: status.iteration,
            reason: "Agent signaled blocked".into(),
        },
        "max-iterations" => SessionState::MaxIterations {
            iterations: status.iteration,
        },
        "started" => SessionState::Starting,
        _ => SessionState::Starting,
    }
}

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new()
    }
}
