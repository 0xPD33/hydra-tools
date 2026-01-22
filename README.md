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

**Status**: v0.1.0 | **Required by**: hydra-wt, hydra-observer

### [hydra-wt](hydra-wt/) (Worktree Manager)

Worktree management for parallel development with automatic port allocation and environment templating.

- 🌳 Git worktree management - Create/remove with one command
- 🔌 Automatic port allocation - Each worktree gets a unique port
- 📝 Environment templating - Generate `.env.local` per worktree
- 📡 Hydra Mail integration - Emit events to `sys:registry` channel

**Status**: v0.1.0 | **Requires**: hydra-mail

### [hydra-observer](hydra-observer/) (Mascots Integration)

HydraMail integration layer for the [Mascots](https://github.com/0xPD33/mascots) desktop companion.

- 🔗 Connects Mascots to HydraMail channels
- 📡 Reacts to `repo:delta`, `team:alert`, `team:status` messages
- 🎭 Shows agent activity through mascot animations
- 🖱️ Click-to-interact with Hydra ecosystem

**Status**: v0.1.0 | **Requires**: hydra-mail, [Mascots](https://github.com/0xPD33/mascots)

## Dependency Graph

```
┌─────────────────────────────────────────────────┐
│                 hydra-mail                       │
│            (pub/sub backbone)                    │
└─────────────────┬───────────────────────────────┘
                  │
        ┌─────────┴─────────┐
        │                   │
        ▼                   ▼
┌───────────────┐   ┌───────────────┐
│   hydra-wt    │   │hydra-observer │
│  (worktrees)  │   │(mascots glue) │
└───────────────┘   └───────┬───────┘
                            │
                            ▼
                    ┌───────────────┐
                    │    Mascots    │
                    │  (external)   │
                    └───────────────┘
```

## Building

### With Nix (Recommended)

```bash
# Build specific package
nix build .#hydra-mail
nix build .#hydra-wt
nix build .#hydra-observer

# Enter development shell
nix develop
```

### With Cargo

```bash
# From each project directory
cd hydra-mail && cargo build --release
cd hydra-wt && cargo build --release
cd hydra-observer && cargo build --release
```

## Repository Structure

```
hydra-tools/
├── hydra-mail/           # Core pub/sub messaging
│   ├── src/
│   ├── docs/
│   ├── .claude-plugin/
│   └── README.md
├── hydra-wt/             # Worktree manager
│   ├── src/
│   └── README.md
├── hydra-observer/       # Mascots integration
│   ├── src/
│   └── README.md
├── flake.nix             # Nix build definitions
└── README.md             # This file
```

## Documentation

| Project | README | Developer Guide |
|---------|--------|-----------------|
| hydra-mail | [README](hydra-mail/README.md) | [CLAUDE.md](hydra-mail/CLAUDE.md) |
| hydra-wt | [README](hydra-wt/README.md) | [CLAUDE.md](hydra-wt/CLAUDE.md) |
| hydra-observer | [README](hydra-observer/README.md) | - |

## License

MIT - See individual projects for details.

## Contributing

Issues and PRs welcome at [0xPD33/hydra-tools](https://github.com/0xPD33/hydra-tools)
