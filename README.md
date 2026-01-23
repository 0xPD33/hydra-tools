# Hydra Tools

A collection of Rust tools for multi-agent coordination and collaboration.

## Quick Start

### For Claude Code Users (Recommended)

```bash
# Add the marketplace and install the plugin
claude plugin marketplace add 0xPD33/hydra-tools
claude plugin install hydra-mail@hydra-tools

# In any project, initialize hydra-mail
cargo install --git https://github.com/0xPD33/hydra-tools hydra-mail
hydra-mail init --daemon
```

The plugin includes a skill with **automatic hooks**:
- **SessionStart**: Checks for messages from other agents
- **Stop**: Reminds to emit a summary of your work

### Manual Installation

```bash
# With Nix
nix build .#hydra-mail
./result/bin/hydra-mail init --daemon

# With Cargo
cargo install --git https://github.com/0xPD33/hydra-tools hydra-mail
hydra-mail init --daemon
```

## Multi-Agent Coordination

Once initialized, agents communicate via channels:

```bash
# Emit a change
echo '{"action":"fixed","target":"auth.py","impact":"login works"}' | \
  hydra-mail emit --channel repo:delta --type delta --data @-

# Listen for changes
hydra-mail subscribe --channel repo:delta --once
```

**Channels:**
- `repo:delta` - Code changes, refactoring, fixes
- `team:status` - Task completion, build results
- `team:alert` - Errors, blockers, warnings
- `team:question` - Questions needing input

## Projects

### [hydra-mail](hydra-mail/) (Core)

Lightweight in-memory pub/sub messaging system with TOON encoding for token-efficient agent communication.

- 🚀 <5ms latency - In-memory Tokio broadcast channels
- 💾 30-60% token savings - TOON encoding
- 🔒 Project isolation - UUID-scoped channels
- 📼 Replay buffer - Last 100 messages per channel
- 🎯 Zero dependencies - Pure Rust, no external brokers

**Status**: v0.1.0 | **Required by**: hydra-wt

### [hydra-wt](hydra-wt/) (Worktree Manager)

Worktree management for parallel development with automatic port allocation and environment templating.

- 🌳 Git worktree management - Create/remove with one command
- 🔌 Automatic port allocation - Each worktree gets a unique port
- 📝 Environment templating - Generate `.env.local` per worktree
- 📡 Hydra Mail integration - Emit events to `sys:registry` channel

**Status**: v0.1.0 | **Requires**: hydra-mail

### [hydra-orchestrator](hydra-orchestrator/) (Session Management)

Multi-session agent orchestration library with tmux integration.

- 🎛️ Session lifecycle management - Spawn, monitor, and teardown agent sessions
- 🔀 Worktree isolation - Each session can run in its own git worktree
- 💾 Persistent state - Sessions survive restarts via filesystem store
- 📡 Hydra Mail integration - Subscribe to channels, react to events

**Status**: v0.1.0 | **Requires**: hydra-mail, hydra-wt (optional)

### [hydra-cli](hydra-cli/) (Unified CLI)

Command-line interface for hydra-orchestrator.

- `hydra init` - Initialize hydralph in current directory
- `hydra spawn` - Spawn a new agent session with PRD
- `hydra ls` - List active sessions
- `hydra status <id>` - Get session status
- `hydra attach <id>` - Attach to session tmux

**Status**: v0.1.0 | **Requires**: hydra-orchestrator

### [hydralph](hydralph/) (Agent Loop)

Shell script implementing the "Ralph loop" for autonomous agent iteration.

- 🔄 Iterative agent execution - Run agent in loop until task complete
- 📋 PRD-driven - Uses JSON PRD with user stories
- 📝 Progress tracking - Maintains progress.txt for context
- 🏷️ Promise tags - Detects `<promise>COMPLETE</promise>` or `<promise>BLOCKED</promise>`
- 📡 Hydra Mail integration - Emits status events

**Status**: v0.1.0 | **Requires**: claude CLI (or compatible agent)

## Dependency Graph

```
┌─────────────────────────────────────────────────┐
│                 hydra-mail                       │
│            (pub/sub backbone)                    │
└─────────────────┬───────────────────────────────┘
                  │
    ┌─────────────┼─────────────┬─────────────┐
    │             │             │             │
    ▼             ▼             ▼             ▼
┌─────────┐ ┌───────────┐ ┌───────────┐ ┌──────────┐
│hydra-wt │ │  hydra-   │ │           │ │ hydralph │
│(worktree│ │orchestrator│ │           │ │  (shell) │
└────┬────┘ └─────┬─────┘ └───────────┘ └──────────┘
     │            │
     │      ┌─────┴─────┐
     │      ▼
     │ ┌─────────┐
     └─│hydra-cli│
       └─────────┘
```

## Building

### With Nix (Recommended)

```bash
# Build specific package
nix build .#hydra-mail
nix build .#hydra-wt
nix build .#hydra-cli

# Enter development shell
nix develop
```

### With Cargo

```bash
# From workspace root
cargo build --release -p hydra-mail
cargo build --release -p hydra-wt
cargo build --release -p hydra-cli
```

## Repository Structure

```
hydra-tools/
├── hydra-mail/           # Core pub/sub messaging
│   ├── src/
│   ├── docs/
│   └── .claude-plugin/
├── hydra-wt/             # Worktree manager with merge support
│   └── src/
├── hydra-orchestrator/   # Multi-session orchestration library
│   └── src/
├── hydra-cli/            # Unified CLI for orchestrator
│   └── src/
├── hydralph/             # Ralph loop shell script
│   ├── hydralph.sh
│   └── prompt.md
├── flake.nix             # Nix build definitions
└── README.md             # This file
```

## Documentation

| Project | README | Developer Guide |
|---------|--------|-----------------|
| hydra-mail | [README](hydra-mail/README.md) | [CLAUDE.md](hydra-mail/CLAUDE.md) |
| hydra-wt | [README](hydra-wt/README.md) | [CLAUDE.md](hydra-wt/CLAUDE.md) |
| hydra-orchestrator | [README](hydra-orchestrator/README.md) | [CLAUDE.md](hydra-orchestrator/CLAUDE.md) |
| hydra-cli | [README](hydra-cli/README.md) | [CLAUDE.md](hydra-cli/CLAUDE.md) |
| hydralph | [README](hydralph/README.md) | [prompt.md](hydralph/prompt.md) |

## License

MIT - See individual projects for details.

## Contributing

Issues and PRs welcome at [0xPD33/hydra-tools](https://github.com/0xPD33/hydra-tools)
