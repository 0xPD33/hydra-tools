# Hydra Mail

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](https://github.com/0xPD33/hydra-tools)

Lightweight in-memory pub/sub messaging system for multi-agent coordination with TOON encoding for token efficiency.

## What is Hydra Mail?

Hydra Mail enables multiple AI agents (like Claude Code, custom agents, or CLI tools) to coordinate through broadcast channels with minimal latency. It's designed for **local, same-machine** collaboration with zero external dependencies.

### Key Features

- **<5ms latency** - In-memory Tokio broadcast channels for ultra-fast message delivery
- **30-60% token savings** - TOON (Token-Oriented Object Notation) encoding vs JSON
- **Project isolation** - UUID-scoped channels prevent cross-project interference
- **Replay buffer** - Late subscribers automatically get message history (last 100 messages)
- **Unix domain socket** - Efficient inter-process communication between agents and daemon
- **Zero external dependencies** - No Redis, RabbitMQ, or other brokers required
- **Claude Code integration** - Hook-based integration for session context and status updates
- **Crash recovery** - Message logging with automatic replay on daemon restart
- **Configurable limits** - Message size, rate limiting, and buffer capacity tuning

### Use Cases

- **Multi-agent coordination** - Multiple Claude Code sessions working on the same project
- **Build automation** - Notify agents of file changes, test results, or deployments
- **Development workflows** - Status updates, progress tracking, and team coordination
- **Local event streaming** - Pub/sub messaging for scripts and tools

## Quick Start

### Installation

#### Via Nix (Recommended)

```bash
# From the hydra-tools repository
cd hydra-tools
nix build .#hydra-mail

# The binary will be at ./result/bin/hydra-mail
# Or run directly with:
nix run .#hydra-mail -- init --daemon
```

#### Via Cargo

```bash
cd hydra-mail
cargo build --release

# Binary location: ./target/release/hydra-mail
```

#### Add to PATH

```bash
# For Nix builds
export PATH="$PATH:$(pwd)/result/bin"

# For Cargo builds
export PATH="$PATH:$(pwd)/target/release"
```

### Basic Usage

**1. Initialize your project:**

```bash
cd your-project
hydra-mail init --daemon
```

This creates:
- `.hydra/config.toml` - Project configuration with unique UUID
- `.hydra/hydra.sock` - Unix domain socket for daemon IPC
- `.hydra/daemon.pid` - Daemon process ID
- `.hydra/config.sh` - Shell environment variables for integration

**2. Emit messages:**

```bash
# Emit a delta (code change notification)
echo '{"action":"fixed","target":"auth.py","impact":"login validates tokens"}' | \
  hydra-mail emit --channel repo:delta --type delta --data @-

# Emit a status update
hydra-mail emit --channel team:status --type progress \
  --data '{"task":"refactoring","percent":75}'
```

**3. Subscribe to channels:**

```bash
# Get one message and exit
hydra-mail subscribe --channel repo:delta --once

# Stream messages continuously
hydra-mail subscribe --channel repo:delta
```

**4. Check status:**

```bash
hydra-mail status
```

Output:
```
Hydra Status for "."
Project UUID: a1b2c3d4-e5f6-7890-abcd-ef1234567890
Socket path: "/path/to/project/.hydra/hydra.sock"
Socket: ✓ exists
Daemon: ✓ running (PID: 12345)

Active Channels:
  repo:delta - 5 msgs buffered, 2 subscribers
  team:status - 3 msgs buffered, 1 subscribers
```

## Channel Reference

Hydra Mail uses a `prefix:name` convention for channels. Standard channels:

| Channel | Purpose | Example Payload |
|---------|---------|-----------------|
| `repo:delta` | Code changes, refactoring, architecture updates | `{"action":"refactored","target":"src/auth.rs","impact":"removed duplicated validation"}` |
| `team:alert` | Errors, warnings, critical issues | `{"severity":"error","file":"tests/auth_test.rs","message":"2 tests failed"}` |
| `team:status` | Progress updates, task completion | `{"task":"pr-review","status":"completed","pr":"123"}` |
| `team:question` | Questions needing coordination or human input | `{"from":"agent-2","question":"How should we handle the edge case?"}` |
| `agent:presence` | Agent lifecycle events (connect/disconnect) | `{"agent":"claude-code","status":"started","session":"abc123"}` |

You can create custom channels using any `prefix:name` format:

```bash
hydra-mail emit --channel custom:builds --type deploy \
  --data '{"env":"staging","status":"success"}'
```

## Architecture

```
┌────────────────────────────────────────────────────────────┐
│                    Agents (CLI Clients)                    │
│  Claude Code │ Custom Scripts │ Other AI Agents            │
└────────────────┬───────────────────────────────────────────┘
                 │
                 │ Unix Domain Socket
                 │ (JSON commands)
                 ▼
┌────────────────────────────────────────────────────────────┐
│                   Daemon Process                           │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ Connection Handler (async per-connection)            │  │
│  │ - Parse JSON commands (emit, subscribe, stats)       │  │
│  │ - Decode base64 TOON messages                        │  │
│  │ - Route to channel manager                           │  │
│  └──────────────────────────────────────────────────────┘  │
└────────────────┬───────────────────────────────────────────┘
                 │
                 ▼
┌────────────────────────────────────────────────────────────┐
│              Channel Manager (channels.rs)                 │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ Global Channel Map (Mutex)                           │  │
│  │ Key: (project_uuid, topic_name)                      │  │
│  │ Value: (broadcast::Sender, ReplayBuffer)             │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                            │
│  ┌─────────────────┐         ┌─────────────────┐          │
│  │ Broadcast       │         │ Replay Buffer   │          │
│  │ Channel         │         │ (100 messages)  │          │
│  │ (Tokio)         │         │ (VecDeque)      │          │
│  │ Capacity: 1024  │         │ FIFO eviction   │          │
│  └─────────────────┘         └─────────────────┘          │
└────────────────────────────────────────────────────────────┘
```

### Core Components

- **CLI Binary** (`src/main.rs`) - Entry point with commands: init, start, stop, emit, subscribe, status, hook
- **Daemon Process** - Persistent server handling client connections via Unix socket
- **Channel System** (`src/channels.rs`) - Tokio broadcast channels with replay buffer
- **Configuration** (`src/config.rs`) - Project UUID, socket path, configurable limits
- **Message Log** (`src/message_log.rs`) - Crash recovery via append-only log
- **Constants** (`src/constants.rs`) - Default capacities, permissions, and limits

### Message Flow

1. **Emit**: Client connects to daemon → sends JSON command with TOON-encoded data
2. **Store**: Daemon decodes message, stores in replay buffer, broadcasts to subscribers
3. **Subscribe**: Client connects → receives history from replay buffer → streams live messages
4. **Crash Recovery**: On restart, daemon replays message log to restore channel state

## CLI Commands

### init

Initialize Hydra Mail in the current project.

```bash
hydra-mail init [--daemon]
```

- `--daemon` - Automatically start the daemon after initialization

Creates `.hydra/` directory with:
- `config.toml` - Project configuration
- `config.sh` - Shell environment variables
- `hydra.sock` - Unix domain socket (when daemon running)
- `daemon.pid` - Daemon process ID (when daemon running)
- `messages.log` - Message log for crash recovery

### start

Start the daemon process.

```bash
hydra-mail start [--project PATH]
```

- `--project` - Project path (default: current directory)

The daemon:
- Binds Unix socket at `.hydra/hydra.sock`
- Loads or replays message log for crash recovery
- Runs log compaction every 10 minutes
- Handles SIGTERM/SIGINT for graceful shutdown

### stop

Stop the daemon process.

```bash
hydra-mail stop [--project PATH]
```

Sends SIGTERM to daemon and cleans up socket/pid files.

### emit

Publish a message to a channel.

```bash
hydra-mail emit --channel CHANNEL --type TYPE [--data DATA|--data @-] \
  [--project PATH] [--format toon] [--target AGENT_ID]
```

- `--channel` - Channel name (e.g., `repo:delta`)
- `--type` - Message type (e.g., `delta`, `status`, `alert`)
- `--data` - JSON data (use `@-` to read from stdin)
- `--format` - Message format (only `toon` supported currently)
- `--target` - Optional target agent ID for filtering

**Examples:**

```bash
# Direct data
hydra-mail emit --channel repo:delta --type delta \
  --data '{"file":"src/main.rs","change":"added error handling"}'

# From stdin
echo '{"status":"passed","tests":42}' | \
  hydra-mail emit --channel team:status --type test --data @-

# With target agent
hydra-mail emit --channel team:question --type query \
  --data '{"question":"How to handle this?"}' --target agent-2
```

### subscribe

Listen to messages on a channel.

```bash
hydra-mail subscribe --channel CHANNEL [--project PATH] [--format toon] [--once]
```

- `--channel` - Channel name to subscribe to
- `--format` - Message format (only `toon` supported currently)
- `--once` - Get one message and exit (for polling)

**Examples:**

```bash
# Stream continuously
hydra-mail subscribe --channel repo:delta

# Get one message
hydra-mail subscribe --channel team:status --once
```

### status

Show daemon and channel status.

```bash
hydra-mail status [--project PATH]
```

Shows:
- Project UUID
- Socket path and status
- Daemon PID and running status
- Active channels with message counts
- Message log file size

### hook

Handle Claude Code hook events (for integration).

```bash
hydra-mail hook session-start [--project PATH]
hydra-mail hook stop [--project PATH]
```

## Integration Examples

### Claude Code Hook Integration

Add to `.claude/settings.json`:

```json
{
  "hooks": {
    "SessionStart": [{
      "hooks": [{
        "type": "command",
        "command": "hydra-mail hook session-start --project ."
      }]
    }],
    "Stop": [{
      "hooks": [{
        "type": "command",
        "command": "hydra-mail hook stop --project ."
      }]
    }]
  }
}
```

This provides:
- Session start: Shows recent messages from other agents as additional context
- Session stop: Reminds to emit a summary of work completed

### Shell Script Integration

Source `.hydra/config.sh` for environment variables:

```bash
source .hydra/config.sh
# Now available: $HYDRA_UUID, $HYDRA_SOCKET, $HYDRA_FORMAT

# Wrapper function
hydra_emit() {
  local channel=$1
  local type=$2
  local data=$3
  hydra-mail emit --channel "$channel" --type "$type" --data "$data"
}

# Usage
hydra_emit "repo:delta" "delta" '{"file":"README.md","change":"updated docs"}'
```

### Python Integration

```python
import socket
import json
import base64

def hydra_emit(channel, msg_type, data):
    """Emit a message to Hydra Mail."""
    payload = {
        "cmd": "emit",
        "channel": channel,
        "format": "toon",
        "data": base64.b64encode(json.dumps(data).encode()).decode()
    }

    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect(".hydra/hydra.sock")
    sock.sendall((json.dumps(payload) + "\n").encode())
    sock.close()

# Usage
hydra_emit("repo:delta", "delta", {
    "file": "src/main.rs",
    "change": "fixed memory leak"
})
```

## Configuration

### config.toml Structure

```toml
project_uuid = "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
socket_path = "/path/to/project/.hydra/hydra.sock"
default_topics = ["repo:delta", "agent:presence"]

[limits]
max_message_size = 10240           # 10KB default
replay_buffer_capacity = 100       # Messages per channel
broadcast_channel_capacity = 1024  # In-flight messages
rate_limit_per_second = 0          # 0 = unlimited
```

### Tuning Limits

Edit `.hydra/config.toml` and restart the daemon:

```toml
[limits]
# Increase for larger messages (e.g., file diffs)
max_message_size = 51200

# Increase for longer history
replay_buffer_capacity = 500

# Prevent flooding (100 msgs/sec per client)
rate_limit_per_second = 100
```

## Performance

**Benchmarked on AMD Ryzen 9 9950X3D @ 5.7GHz**

| Metric | Value |
|--------|-------|
| Emit latency | **100 ns** |
| Roundtrip latency | **2.7 µs** |
| Peak throughput | **10.6M msgs/sec** |
| Concurrent scaling | **Linear up to 16 tasks** |
| Replay buffer capacity | 100 messages/channel |
| Broadcast capacity | 1024 in-flight messages |
| Max message size | 10KB (configurable) |

**Memory & Disk:**
- Binary size: ~1MB
- Memory per channel: Minimal (replay buffer is primary consumer)
- Message log: Append-only, compacted every 10 minutes

## Platform Support

| Platform | Status |
|----------|--------|
| Linux | ✅ Full support (Unix domain sockets) |
| macOS | ✅ Full support (Unix domain sockets) |
| Windows | ❌ Planned for v1.0 (named pipes) |

## Development

### Build & Test

```bash
# Enter development environment
nix develop

# Build
cargo build --release

# Run tests
cargo test

# Run with output
cargo test -- --nocapture

# Run benchmarks
cargo bench

# Format & lint
cargo fmt
cargo clippy
```

### Project Structure

```
hydra-mail/
├── src/
│   ├── main.rs          # CLI and daemon entry point (906 lines)
│   ├── channels.rs      # Pub/sub system (535 lines)
│   ├── config.rs        # Configuration management (168 lines)
│   ├── constants.rs     # Default capacities and limits (26 lines)
│   ├── message_log.rs   # Crash recovery log
│   └── lib.rs           # Library exports
├── tests/
│   ├── integration_test.rs       # End-to-end tests
│   └── crash_recovery_test.rs    # Daemon restart tests
├── benches/
│   └── channels.rs      # Performance benchmarks
├── docs/
│   ├── ARCHITECTURE.md  # Detailed architecture
│   └── SPEC.md          # Full specification
├── CLAUDE.md            # Developer guide for Claude Code
└── README.md            # This file
```

### Code Statistics

- **Total LOC**: ~1,600 (Rust)
- **Unsafe code**: 0% (denied by lint)
- **Dependencies**: 9 core, 3 dev (minimal footprint)
- **Test coverage**: Unit + integration + stress tests

## Troubleshooting

### "Daemon not running" error

```bash
# Check status
hydra-mail status

# If daemon PID file exists but process is dead
hydra-mail stop
hydra-mail start
```

### "Socket exists but cannot connect"

```bash
# Manual cleanup
rm .hydra/hydra.sock .hydra/daemon.pid
hydra-mail start
```

### "Permission denied" error

```bash
# Check permissions
ls -la .hydra/

# Fix permissions
chmod 700 .hydra
chmod 600 .hydra/hydra.sock
```

### "Message too large" error

Edit `.hydra/config.toml`:
```toml
[limits]
max_message_size = 51200  # Increase from 10KB default
```

### View daemon logs

```bash
cat .hydra/daemon.err
```

## Roadmap

- **v0.1.0** (Current) - Basic pub/sub with TOON, replay buffer, daemon mode, crash recovery
- **v0.2.0** - Claude Code skill with rich context formatting
- **v0.3.0** - Message filtering by target agent, TTL support
- **v1.0.0** - Windows support (named pipes), metrics collection, durable sled integration

## License

MIT - See [LICENSE](LICENSE) for details

## Contributing

Built by [0xPD33](https://github.com/0xPD33)

Issues and PRs welcome at [https://github.com/0xPD33/hydra-tools](https://github.com/0xPD33/hydra-tools)

## Related Projects

- **hydra-wt** - Git worktree manager for parallel development
- **hydra-orchestrator** - Session management for multi-agent systems
- **hydra-cli** - Unified CLI for all Hydra tools
- **hydralph** - Agent iteration loop for PRD-driven development
