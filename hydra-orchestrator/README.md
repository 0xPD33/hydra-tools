# hydra-orchestrator

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](https://github.com/0xPD33/hydra-tools)

Multi-session agent orchestration library with TMUX integration, health monitoring, and optional worktree isolation.

## What is hydra-orchestrator?

`hydra-orchestrator` is a Rust library that manages multiple concurrent AI agent sessions (hydralph) with robust lifecycle control. It provides TMUX integration for easy inspection, health monitoring with automatic stuck detection, pause/resume capabilities, and optional git worktree isolation via `hydra-wt`.

### Key Features

- **TMUX Integration** - Each agent runs in its own TMUX session for easy inspection and debugging
- **Health Monitoring** - Automatic stuck detection and duration limit enforcement
- **Pause/Resume** - Control agent execution without termination
- **Worktree Support** - Optional per-session git worktree isolation (via `hydra-wt`)
- **State Persistence** - Sessions survive orchestrator restarts via filesystem storage
- **Hydra-Mail Integration** - Event-based coordination via pub/sub messaging

## Installation

### Via Nix (Recommended)

```bash
# Add to your flake.nix inputs
inputs.hydra-tools.url = "github:your-org/hydra-tools";

# Use in your shell or package
{
  inputs.hydra-tools.url = "github:your-org/hydra-tools";

  outputs = { self, nixpkgs, hydra-tools, ... }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in {
      devShells.${system}.default = pkgs.mkShell {
        buildInputs = [
          hydra-tools.packages.${system}.hydra-orchestrator
        ];
      };
    };
}
```

### Cargo

```toml
[dependencies]
hydra-orchestrator = { git = "https://github.com/your-org/hydra-tools", package = "hydra-orchestrator" }

# Optional: enable worktree integration
hydra-orchestrator = { git = "...", features = ["worktree"] }
```

## Library API

### Creating an Orchestrator

```rust
use hydra_orchestrator::Orchestrator;
use std::path::Path;

// Standalone (no hydra-mail)
let orch = Orchestrator::new();

// With hydra-mail integration for event coordination
let orch = Orchestrator::with_mail(Path::new("/your/project"))?;
```

### Spawning Sessions

```rust
use hydra_orchestrator::{Orchestrator, SessionConfig};
use std::time::Duration;

let mut orch = Orchestrator::new();

let config = SessionConfig {
    max_iterations: 50,
    max_duration: Duration::from_secs(3600),
    agent_cli: "claude".into(),
    agent_flags: "-p fast".into(),
    prd_path: "/path/to/prd.json".into(),
    working_dir: std::env::current_dir()?,
    use_worktree: true,  // Requires "worktree" feature
    branch_name: Some("feature/agent-work".into()),
    ..Default::default()
};

let session_id = orch.spawn(config)?;
println!("Agent started: {}", session_id);
```

### Session Control

```rust
// List all sessions
let sessions = orch.list();
for status in sessions {
    println!("{}: {} ({:?})", status.id, status.state, status.duration);
}

// Pause an agent
orch.pause(&session_id)?;

// Resume a paused agent
orch.resume(&session_id)?;

// Inject a message into the agent's context
orch.inject(&session_id, "Please focus on X instead")?;

// Attach to the agent's TMUX session (interactive)
orch.attach(&session_id)?;

// Kill a session
orch.kill(&session_id, "Task completed")?;
```

### Health Monitoring

```rust
use std::time::Duration;

// Run periodic health checks
loop {
    tokio::time::sleep(Duration::from_secs(60)).await;
    let killed = orch.health_check()?;

    for id in killed {
        println!("Session {} killed by health check", id);
    }
}
```

## Session Lifecycle

```
┌─────────┐    spawn()    ┌──────────┐
│ None    │ ────────────> │ Starting │
└─────────┘               └─────┬────┘
                                │
                     ┌──────────┴──────────┐
                     ▼                     ▼
                ┌────────┐           ┌─────────┐
                │ Paused │           │ Running │
                └────────┘           └────┬────┘
                     ▲                    │
                     │            ┌───────┴────────┐
                     │            │                │
                pause()        ┌───▼────┐    ┌────▼────┐
                              │ Stuck  │    │ Blocked │
                              └────────┘    └─────────┘
                                   │              │
                                   ▼              ▼
                              ┌────────────────────────┐
                              │ Completed │ Failed     │
                              └────────────────────────┘
```

## State Management

### Filesystem Storage

Sessions are persisted to `.hydra/orchestrator/sessions/` as JSON files:

```json
{
  "id": "a1b2c3d4",
  "tmux_session": "hydralph-a1b2c3d4",
  "prd_path": "/project/.hydra/ralph/prd.json",
  "max_iterations": 50,
  "max_duration_secs": 3600,
  "agent_cli": "claude",
  "working_dir": "/project",
  "use_worktree": true,
  "branch_name": "hydralph/a1b2c3d4",
  "worktree_path": "/project/.hydra/worktrees/hydralph-a1b2c3d4",
  "allocated_port": 8080,
  "created_at": 1706112345
}
```

The orchestrator automatically loads persisted sessions on startup.

### Runtime State

Each session writes `status.json` to `.hydra/ralph/`:

```json
{
  "status": "running",
  "iteration": 12,
  "max": 50,
  "stories": "Implementing feature X"
}
```

### Hydra-Mail Events

When connected to hydra-mail, the orchestrator emits events:

- `session:spawned` - New session created
- `session:paused` - Session paused
- `session:resumed` - Session resumed
- `session:stuck` - Session detected as stuck (no activity for 15min)
- `session:killed` - Session terminated
- `session:injected` - Message injected into session

## Health Monitoring

### Stuck Detection

Sessions with no activity for 15 minutes are marked as `Stuck`. This is emitted via hydra-mail but does not auto-kill the session.

### Duration Limits

Sessions exceeding `max_duration` are automatically terminated by `health_check()`.

### TMUX Session Monitoring

If a TMUX session disappears unexpectedly, the session is marked as `Failed`.

## Integration Patterns

### CLI Wrapper (hydra-cli)

```rust
use hydra_orchestrator::Orchestrator;

fn main() -> anyhow::Result<()> {
    let mut orch = Orchestrator::with_mail(std::path::Path::new("."))?;

    match std::env::args().nth(1).as_deref() {
        Some("list") => {
            for s in orch.list() {
                println!("{}: {}", s.id, s.state);
            }
        }
        Some("attach") => {
            let id = std::env::args().nth(2).unwrap();
            orch.attach(&hydra_orchestrator::SessionId(id))?;
        }
        _ => {}
    }
    Ok(())
}
```

### Background Health Monitor

```rust
async fn monitor_task(mut orch: Orchestrator) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        let _ = orch.health_check();
    }
}
```

## Feature Flags

- `worktree` - Enable git worktree integration via `hydra-wt` (optional)

## Dependencies

- TMUX (runtime requirement)
- Tokio (async runtime)
- Serde (serialization)
- UUID (session IDs)
- `hydra-wt` (optional, with `worktree` feature)

## Platform Support

- **Linux** - Full support (tmux)
- **macOS** - Full support (tmux)
- **Windows** - Not supported (tmux not available)

## License

MIT - See [LICENSE](../LICENSE) for details.

## Contributing

Built by [0xPD33](https://github.com/0xPD33)

Issues and PRs welcome at [https://github.com/0xPD33/hydra-tools](https://github.com/0xPD33/hydra-tools)

## Related Projects

- **hydra-cli** - Unified CLI for Hydra orchestrator
- **hydra-mail** - Pub/sub messaging for coordination
- **hydra-wt** - Worktree management
- **hydralph** - Agent scripts and templates
