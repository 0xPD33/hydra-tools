# Hydra CLI

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](https://github.com/0xPD33/hydra-tools)

Unified command-line interface for the Hydra orchestrator. Manages autonomous agent sessions with PRD-driven development, tmux integration, and optional worktree support.

## What is Hydra CLI?

Hydra CLI provides a user-friendly interface to `hydra-orchestrator`, enabling you to spawn and manage autonomous AI agent sessions that iterate on project requirements using the Ralph pattern (fresh agent per iteration for clean context).

**Key Features**
- Session management - Spawn, list, pause, resume, and kill agent sessions
- PRD-driven development - Agents work from product requirement documents with user stories
- Tmux integration - Each session runs in an isolated tmux window for easy monitoring
- Worktree support - Optional git worktree isolation per session
- Hydra Mail integration - Coordinates with other hydra-tools when available
- Agent injection - Send messages to running sessions for guidance
- Duration limits - Configurable time limits to prevent runaway sessions

## Quick Start

## Installation

### Via Nix (Recommended)

```bash
nix build .#hydra-cli
./result/bin/hydra --help
```

### Via Cargo

```bash
# From hydra-cli directory
cargo build --release
./target/release/hydra --help
```

### Usage

**1. Initialize your project:**
```bash
cd your-project
hydra init
# Creates .hydra/ralph/ with prd.json and progress tracking
```

**2. Edit your PRD:**
```bash
# Edit .hydra/ralph/prd.json with your user stories
cat .hydra/ralph/prd.json
{
  "title": "Project PRD",
  "userStories": [
    {
      "id": "story-1",
      "title": "First Story",
      "description": "Describe what needs to be done",
      "passes": false,
      "acceptance": ["Criteria 1", "Criteria 2"]
    }
  ]
}
```

**3. Spawn a session:**
```bash
hydra spawn
# Spawned session: abc123
# Attach: hydra attach abc123
# List:   hydra ls
```

**4. Monitor progress:**
```bash
hydra ls                    # List all sessions
hydra status abc123         # Get session details
hydra attach abc123         # Attach to tmux session
```

**5. Interact with session:**
```bash
hydra inject abc123 "Consider using async/await here"
hydra pause abc123          # Pause the session
hydra resume abc123         # Resume the session
hydra kill abc123           # Kill the session
```

## CLI Commands

```bash
hydra init                          # Initialize hydralph in current directory
hydra spawn [OPTIONS]               # Spawn a new hydralph session
hydra ls                            # List active sessions
hydra status <id>                   # Get session status
hydra attach <id>                   # Attach to session (opens tmux)
hydra pause <id>                    # Pause session
hydra resume <id>                   # Resume session
hydra inject <id> <message>         # Inject message for agent
hydra kill <id> [--reason <text>]   # Kill session
```

### Command Details

#### `hydra init`

Initializes hydralph in the current directory by creating `.hydra/ralph/` with:

- `prd.json` - Product requirements document (with example if not exists)
- `progress.txt` - Progress log for tracking completed work
- `hydralph.sh` - Agent script (copied from project root if available)
- `prompt.md` - Agent prompt template (copied from project root if available)

#### `hydra spawn [OPTIONS]`

Spawns a new autonomous agent session:

```bash
hydra spawn [OPTIONS]

Options:
  --prd <path>              PRD file path [default: .hydra/ralph/prd.json]
  --max-iterations <n>      Maximum iterations [default: 10]
  --max-duration <time>     Maximum duration [default: 4h] (e.g., 4h, 30m, 1h30m)
  --agent <name>            Agent CLI to use [default: claude]
  --worktree                Use git worktree for isolation
  --branch <name>           Branch name for worktree
```

**Duration Format**: Supports `h` (hours), `m` (minutes), `s` (seconds). Examples: `4h`, `30m`, `1h30m`, `90m`.

**Session Output**:
- Returns session ID (e.g., `abc123`)
- Displays attach command
- Session runs in isolated tmux window

#### `hydra ls`

Lists all active sessions:

```bash
hydra ls

# Output:
# ID           STATE                 DURATION   TMUX
# ------------------------------------------------------------
# abc123       Running { story-2 }   15m 32s    hydra-abc123
# def456       Paused                2h 15m     hydra-def456
```

#### `hydra status <id>`

Shows detailed session status with color-coded states:

```bash
hydra status abc123

# Output:
# Session:  abc123
# State:    Running { story-2, iteration 3 }
# Duration: 15m 32s
# TMUX:     hydra-abc123
# Port:     8080
# Iteration: 3/10
# Stories:   story-2
```

**Session States**:
- `Running` - Green - Agent actively working
- `Completed` - Green - All stories passed
- `Paused` - Yellow - Manually paused
- `Blocked` - Red - Blocked on issue
- `Failed` - Red - Error occurred
- `Stuck` - Yellow - No progress for 10+ minutes
- `MaxIterations` - Yellow - Hit iteration limit

#### `hydra attach <id>`

Attaches to the session's tmux window:

```bash
hydra attach abc123
# Opens tmux session (replaces current shell)
```

Use `Ctrl+B D` to detach from tmux without killing the session.

#### `hydra pause <id>`

Pauses a running session:

```bash
hydra pause abc123
# Paused
```

The agent stops at the next safe checkpoint.

#### `hydra resume <id>`

Resumes a paused session:

```bash
hydra resume abc123
# Resumed
```

#### `hydra inject <id> <message>`

Injects a message to the agent for the next iteration:

```bash
hydra inject abc123 "Consider using async/await for the database calls"
# Injected message for next iteration
```

Useful for providing guidance without pausing the session.

#### `hydra kill <id> [--reason <text>]`

Terminates a session:

```bash
hydra kill abc123
# Killed

hydra kill abc123 --reason "requirements changed"
# Killed
```

Session state is preserved in `.hydra/ralph/` for review.

## Configuration

### `.hydra/ralph/prd.json`

Product requirements document that drives agent behavior:

```json
{
  "title": "Project PRD",
  "userStories": [
    {
      "id": "story-1",
      "title": "First Story",
      "description": "Describe what needs to be done",
      "passes": false,
      "acceptance": ["Criteria 1", "Criteria 2"]
    }
  ]
}
```

### `.hydra/ralph/progress.txt`

Progress log maintained by the agent:

```markdown
# Hydralph Progress Log

## 2025-01-23 10:30:00
Started session abc123
Working on story-1: First Story

## 2025-01-23 11:15:00
Completed story-1
All acceptance criteria passed
```

### Session Config

Sessions use these defaults (overridable via flags):

| Setting | Default | Description |
|---------|---------|-------------|
| `--prd` | `.hydra/ralph/prd.json` | PRD file path |
| `--max-iterations` | `10` | Maximum agent iterations |
| `--max-duration` | `4h` | Maximum session duration |
| `--agent` | `claude` | Agent CLI to invoke |
| `--worktree` | `false` | Use git worktree isolation |
| `--branch` | (auto) | Branch name for worktree |

## Worktree Integration

When using `--worktree`, sessions create isolated git worktrees:

```bash
hydra spawn --worktree --branch feature-auth
# Creates worktree at ../feature-auth/
# All changes made in worktree
# Main repo stays clean
```

Worktrees are managed by `hydra-wt` and include:
- Unique port allocation (if configured)
- Isolated `.env.local` (if template exists)
- Clean separation from main branch

## Hydra Mail Integration

Hydra CLI automatically integrates with `hydra-mail` when available:

- **Session events** - Emits to `hydra:session` channel
- **State changes** - Running, paused, completed, failed
- **Agent messages** - Injection messages and responses
- **Coordination** - Other tools can react to session state

If `hydra-mail` is not running, CLI operates in standalone mode with warnings.

## Usage Examples

### Basic Workflow

```bash
# 1. Initialize project
cd my-project
hydra init

# 2. Define requirements
vim .hydra/ralph/prd.json
# Add your user stories

# 3. Spawn session
hydra spawn --max-iterations 5

# 4. Monitor progress
watch -n 5 'hydra status abc123'

# 5. Attach when needed
hydra attach abc123
# Press Ctrl+B D to detach

# 6. Inject guidance
hydra inject abc123 "Use the existing auth library"

# 7. Cleanup when done
hydra kill abc123
```

### Parallel Development with Worktrees

```bash
# Main feature
hydra spawn --worktree --branch feature-auth --max-duration 2h

# Bug fix in parallel
hydra spawn --worktree --branch fix-login --max-duration 1h

# List sessions
hydra ls

# Attach to specific session
hydra attach def456
```

### Time-Bounded Sessions

```bash
# Quick 30-minute session
hydra spawn --max-duration 30m

# Longer session with more iterations
hydra spawn --max-duration 8h --max-iterations 20
```

### Interactive Development

```bash
# Spawn session
hydra spawn

# Monitor from another terminal
watch -n 2 'hydra status $(hydra ls | head -n 2 | tail -n 1 | cut -d" " -f1)'

# Inject guidance as needed
hydra inject abc123 "Focus on the API endpoints first"

# Pause to review
hydra pause abc123
git diff ../feature-auth
hydra resume abc123
```

## Project Structure

```
hydra-cli/
├── Cargo.toml
├── README.md
├── CLAUDE.md
└── src/
    └── main.rs          # CLI entry (clap)
```

### Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| hydra-orchestrator | path | Session management library |
| clap | 4.5 | CLI argument parsing |
| anyhow | 1.0 | Error handling |
| ansi_term | 0.12 | Terminal colors |

## Architecture

```
User runs: hydra spawn
                │
                ▼
┌─────────────────────────────────────────────────┐
│  main.rs: CLI parsing (clap)                    │
│  - Parse args                                   │
│  - Find project root                            │
│  - Connect to hydra-mail (optional)             │
└────────────────────┬────────────────────────────┘
                     │
                     ▼
          ┌──────────────────┐
          │ hydra-orchestrator│
          │ - Spawn session   │
          │ - Manage state    │
          │ - tmux integration│
          └──────────────────┘
```

### Session Lifecycle

1. **Spawn** - Create session with config
2. **Initialize** - Setup tmux, load PRD, start agent
3. **Running** - Agent iterates on stories
4. **Pause/Resume** - Manual control
5. **Complete/Fail** - Terminal state
6. **Kill** - Manual termination

### Integration Points

- **hydra-orchestrator** - Core session management
- **hydra-mail** - Event coordination (optional)
- **hydra-wt** - Worktree management (via orchestrator)
- **tmux** - Terminal multiplexing for sessions
- **hydralph** - Agent scripts and templates

## Development

### Build & Test

```bash
# Enter Nix dev environment
nix develop

# Build
cargo build --release

# Test manually
cd /tmp && mkdir test && cd test
git init
hydra init
hydra spawn

# Format & lint
cargo fmt
cargo clippy
```

### Adding Commands

1. Add variant to `Commands` enum in main.rs
2. Add match arm in `main()` function
3. Call orchestrator methods
4. Update documentation

## Platform Support

- **Linux** - Full support
- **macOS** - Full support
- **Windows** - Not supported (tmux, Unix sockets)

## License

MIT - See [LICENSE](../LICENSE) for details.

## Related

- [hydra-orchestrator](../hydra-orchestrator/) - Session management library
- [hydra-mail](../hydra-mail/) - Pub/sub messaging for coordination
- [hydra-wt](../hydra-wt/) - Worktree management
- [hydralph](../hydralph/) - Agent scripts and templates
