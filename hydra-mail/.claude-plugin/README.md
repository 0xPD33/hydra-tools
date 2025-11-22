# Hydra Mail - Claude Code Plugin

Multi-agent pub/sub messaging system with TOON encoding for token-efficient coordination between AI agents.

## Features

- **Lightweight pub/sub** - In-memory broadcast channels with <5ms latency
- **Token efficiency** - TOON encoding provides 30-60% token savings vs JSON
- **Project isolation** - UUID-scoped channels prevent cross-project interference
- **Replay buffer** - Late subscribers automatically receive message history
- **Skills integration** - Auto-generated Claude Code skills for seamless usage

## Installation

### Option 1: Install from Local Repository

If you've cloned this repository:

```bash
# From the hydra-tools directory
claude plugins install --local .

# Or specify the full path
claude plugins install --local /path/to/hydra-tools
```

### Option 2: Install from GitHub

```bash
claude plugins install --git https://github.com/0xPD33/hydra-tools.git
```

## Usage

### 1. Initialize in Your Project

```bash
cd your-project
hydra-mail init --daemon
```

This creates:
- `.hydra/config.toml` - Project configuration with UUID
- `.hydra/skills/hydra-mail.yaml` - Claude Code skill (auto-uploaded)
- `.hydra/config.sh` - Shell integration
- Daemon process for persistent messaging

### 2. Use in Claude Code

Once the skill is loaded, use these tools in your prompts:

**Emit a message after completing an action:**
```
After fixing the auth bug, I'll notify other agents:
hydra_emit channel='repo:delta' type='delta' data='{"action":"fixed","target":"auth.py","impact":"login validates tokens"}'
```

**Check for messages from other agents:**
```
Let me check if other agents have made changes:
hydra_subscribe channel='repo:delta' once=true
```

### 3. Channels

- `repo:delta` - Code changes, refactoring, architecture
- `team:alert` - Errors, warnings, critical issues
- `team:status` - Progress updates, test results
- `team:question` - Questions needing coordination

## CLI Commands

```bash
# Initialize project
hydra-mail init [--daemon]

# Start daemon
hydra-mail start

# Stop daemon
hydra-mail stop

# Check status
hydra-mail status

# Emit message
echo '{"action":"updated","target":"file.rs"}' | \
  hydra-mail emit --channel repo:delta --type delta

# Subscribe to channel
hydra-mail subscribe --channel repo:delta [--once]
```

## Skills

The plugin includes one skill:

- **hydra-mail** - Core pub/sub tools for multi-agent coordination

## Architecture

- **In-memory pub/sub** - Tokio broadcast channels
- **Unix Domain Sockets** - Efficient IPC (Linux/macOS only)
- **TOON encoding** - Token-Oriented Object Notation for efficiency
- **Project-scoped** - UUID-based isolation
- **Daemon mode** - Persistent process per project

## Building from Source

```bash
# Using Nix (recommended)
nix build

# Using Cargo
cargo build --release

# Binary location
./target/release/hydra-mail
```

## Documentation

- [Architecture](docs/ARCHITECTURE.md) - Detailed design document
- [CLAUDE.md](CLAUDE.md) - Project guidance for Claude Code
- [Skills](skills/hydra-mail/SKILL.md) - Skill reference

## License

MIT
