# Hydra WT

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](https://github.com/0xPD33/hydra-tools)

Worktree management for the Hydra ecosystem. Manages git worktrees with automatic port allocation, environment templating, and Hydra Mail integration.

## What is Hydra WT?

Hydra WT enables parallel development across multiple git worktrees with automatic resource management. Each worktree gets a unique port and customized environment file, making it easy to run multiple feature branches simultaneously.

**Key Features:**
- **Automatic port allocation** - Each worktree gets a unique port from a configurable range
- **Environment templating** - Generate `.env.local` files with worktree-specific values
- **Hydra Mail integration** - Emit events to `sys:registry` channel for coordination
- **Simple CLI** - Five commands: init, create, list, remove, status

## Quick Start

### Installation

```bash
# Using Nix (recommended)
nix build .#hydra-wt
./result/bin/hydra-wt --help

# Using Cargo (from hydra-wt directory)
cargo build --release
./target/release/hydra-wt --help
```

### Usage

**1. Initialize Hydra Mail first** (if not already done):
```bash
cd your-project
hydra-mail init --daemon
```

**2. Initialize Hydra WT:**
```bash
hydra-wt init
```

**3. Create a worktree:**
```bash
hydra-wt create feature-auth
# Output:
#   Allocated port 3001 for feature-auth
#   Creating worktree at ../feature-auth...
#   Created ../feature-auth/.env.local
#   Worktree 'feature-auth' created successfully
```

**4. List worktrees:**
```bash
hydra-wt list
# BRANCH               PORT   PATH                           STATUS
# ----------------------------------------------------------------------
# feature-auth         3001   ../feature-auth                exists
```

**5. Remove when done:**
```bash
hydra-wt remove -f feature-auth
```

## Configuration

### `.hydra/wt.toml`

Created by `hydra-wt init`:

```toml
[ports]
range_start = 3001
range_end = 3099

[env]
template = ".env.template"
output = ".env.local"

[worktrees]
directory = "../"
```

### `.env.template`

Create this file in your project root with variables to interpolate:

```bash
PORT={{port}}
WORKTREE_NAME={{worktree}}
DATABASE_URL=postgres://localhost:5432/myapp_{{worktree}}
PROJECT_UUID={{project_uuid}}
REPO_ROOT={{repo_root}}
```

### Template Variables

| Variable | Description |
|----------|-------------|
| `{{port}}` | Allocated port number (e.g., 3001) |
| `{{worktree}}` | Branch/worktree name (e.g., feature-auth) |
| `{{project_uuid}}` | Project UUID from `.hydra/config.toml` |
| `{{repo_root}}` | Absolute path to main repository |

## CLI Commands

```bash
hydra-wt init                    # Create .hydra/wt.toml and wt-ports.json
hydra-wt create <branch>         # Create worktree with port + env
hydra-wt list                    # Show managed worktrees
hydra-wt remove [-f] <branch>    # Remove worktree (-f for force)
hydra-wt status [branch]         # Show status summary or details
```

### Command Details

#### `hydra-wt create <branch>`

1. Allocates next free port from configured range
2. Creates git worktree (new branch if doesn't exist, checkout if exists)
3. Renders `.env.template` to worktree's `.env.local` (if template exists)
4. Emits `worktree_created` event to Hydra Mail (if available)

#### `hydra-wt remove [-f] <branch>`

1. Removes git worktree
2. Frees allocated port
3. Emits `worktree_removed` event to Hydra Mail (if available)

Use `-f/--force` to remove worktrees with untracked/modified files.

## Hydra Mail Integration

Hydra WT emits events to the `sys:registry` channel:

**On create:**
```json
{"type":"worktree_created","worktree":"feature-auth","port":3001,"path":"../feature-auth"}
```

**On remove:**
```json
{"type":"worktree_removed","worktree":"feature-auth"}
```

If `hydra-mail` is not installed or not running, warnings are printed but operations continue.

## Project Structure

```
hydra-wt/
├── Cargo.toml
├── README.md
├── CLAUDE.md
└── src/
    ├── main.rs          # CLI entry (clap)
    ├── config.rs        # .hydra/wt.toml management
    ├── ports.rs         # Port allocation registry
    ├── worktree.rs      # Git worktree operations
    ├── template.rs      # .env.template rendering (tera)
    └── hydra.rs         # Hydra Mail event emission
```

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
hydra-wt init  # Will fail: needs .hydra from hydra-mail
```

### Dependencies

| Crate | Purpose |
|-------|---------|
| clap | CLI argument parsing |
| serde + serde_json | Serialization |
| toml | Config file parsing |
| tera | Template rendering |
| anyhow | Error handling |
| uuid | UUID reading |

## Use Cases

### Parallel Feature Development

```bash
# Main repo at ~/project
cd ~/project
hydra-mail init --daemon
hydra-wt init

# Create worktrees for different features
hydra-wt create feature-auth      # Port 3001
hydra-wt create feature-billing   # Port 3002
hydra-wt create bugfix-login      # Port 3003

# Each worktree has its own .env.local
cat ../feature-auth/.env.local
# PORT=3001
# WORKTREE_NAME=feature-auth
# ...

# Run dev servers on different ports
cd ../feature-auth && npm run dev  # localhost:3001
cd ../feature-billing && npm run dev  # localhost:3002
```

### Multi-Agent Coordination

When combined with Hydra Mail, agents can:
- See which worktrees exist via `sys:registry` channel
- Know which ports are in use
- Coordinate work across branches

## Platform Support

- **Linux** - Full support
- **macOS** - Full support
- **Windows** - Not supported (Unix sockets, git worktrees)

## License

MIT - See [LICENSE](../LICENSE) for details.

## Related

- [hydra-mail](../hydra-mail/) - Pub/sub messaging for agent coordination
- [hydra-observer](../hydra-observer/) - Animated desktop mascot with coordination awareness
