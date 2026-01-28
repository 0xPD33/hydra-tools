# hydra-wt

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](https://github.com/0xPD33/hydra-tools)

Git worktree management with automatic port allocation and environment templating for the Hydra ecosystem.

## What is hydra-wt?

`hydra-wt` simplifies working with Git worktrees by automating resource management.

### Key Features

- **Automatic port allocation** - Assigns unique ports to each worktree from a configurable range
- **Environment templating** - Generates per-worktree environment files using Tera templates
- **Merge workflow** - Provides safe merge operations with conflict detection and cleanup
- **Hydra-mail integration** - Emits events to the Hydra message bus for coordination
- **Artifacts system** - Share dependencies and files via symlinks or copies
- **Hooks system** - Run commands after worktree creation

Perfect for running multiple instances of web services side-by-side during development.

## Installation

### Via Nix (Recommended)

```bash
nix build .#hydra-wt
./result/bin/hydra-wt --help
```

### From Source

```bash
cd hydra-wt
cargo build --release
./target/release/hydra-wt --help
```

## Quick Start

```bash
# Initialize (requires .hydra/ from hydra-mail)
hydra-wt init

# Create a worktree with automatic port allocation
hydra-wt create feature-auth

# List all worktrees with status
hydra-wt list

# Merge feature branch to main
hydra-wt merge feature-auth main --force

# Clean up
hydra-wt remove feature-auth
```

## CLI Commands

### `init`

Initialize `hydra-wt` configuration in your repository.

```bash
hydra-wt init
```

Creates `.hydra/wt.toml` with default settings and initializes the port registry.

**Prerequisites**: Requires `.hydra/` directory (created by `hydra-mail init`).

### `create`

Create a new worktree with automatic port allocation.

```bash
hydra-wt create <branch>
```

**What it does:**
1. Allocates a free port from the configured range
2. Creates a Git worktree at the configured directory
3. Renders `.env.template` to `.env.local` (or configured output) with worktree-specific variables
4. Sets up any configured artifacts (symlinks/copies)
5. Runs post-create hooks
6. Emits a `worktree_created` event to Hydra

**Example:**
```bash
hydra-wt create feature-user-profile
# Output:
# Allocated port 3001 for feature-user-profile
# Creating worktree at ../feature-user-profile/...
# Created ../feature-user-profile/.env.local
#
# Worktree 'feature-user-profile' created successfully
#   Path: ../feature-user-profile
#   Port: 3001
```

### `list`

List all managed worktrees with status and merge information.

```bash
hydra-wt list
```

**Output columns:**
- **BRANCH** - Branch/worktree name
- **PORT** - Allocated port number
- **PATH** - Filesystem path
- **STATUS** - `exists` or `missing`
- **COMMITS AHEAD** - Number of commits ahead of main, or `(conflicts)` if merge would conflict

**Example:**
```
BRANCH               PORT   PATH                      STATUS     COMMITS AHEAD
-------------------------------------------------------------------------------------
feature-auth         3001   ../feature-auth           exists     3 (conflicts)
feature-billing      3002   ../feature-billing        exists     up to date
main                 -      .                         exists     -
```

### `remove`

Remove a worktree and free its port.

```bash
hydra-wt remove <branch> [--force]
```

- Without `--force`: Fails if worktree has uncommitted/untracked files
- With `--force`: Removes regardless of working tree state

**Example:**
```bash
hydra-wt remove feature-auth
# Output:
# Removing worktree at ../feature-auth/...
# Freed port 3001
# Worktree 'feature-auth' removed
```

### `status`

Show status of worktrees.

```bash
hydra-wt status [branch]
```

- **Without argument**: Shows summary (total, existing, missing, port usage)
- **With branch name**: Shows detailed info for specific worktree

### `merge`

Merge a source branch into a target branch.

```bash
hydra-wt merge <source> <target> [options]
```

**Options:**
- `--force` - Skip confirmation prompt
- `--no-ff` - Create a merge commit even for fast-forward
- `--dry-run` - Preview merge without executing (checks for conflicts)
- `--cleanup` - Remove source worktree after successful merge

**What it does:**
1. Validates both branches exist
2. Checks for uncommitted changes in target
3. Shows commits that will be merged
4. Performs merge with conflict detection
5. Emits appropriate events to Hydra

**Examples:**

Preview merge (check for conflicts):
```bash
hydra-wt merge feature-auth main --dry-run
```

Merge with cleanup:
```bash
hydra-wt merge feature-auth main --force --cleanup
```

### `merge-abort`

Abort an in-progress merge.

```bash
hydra-wt merge-abort <branch>
```

Use this when a merge has conflicts and you want to return to the pre-merge state.

## Configuration

Configuration is stored in `.hydra/wt.toml`:

```toml
[ports]
range_start = 3001
range_end = 3099

[env]
template = ".env.template"
output = ".env.local"

[worktrees]
directory = "../"

[artifacts]
symlink = ["node_modules", ".cache"]
copy = ["config.local.json"]

[hooks]
post_create = ["npm install", "npm run build"]
```

### Sections

#### `[ports]`

- `range_start` - First port in allocation range (default: 3001)
- `range_end` - Last port in allocation range (default: 3099)

Ports are allocated sequentially from `range_start` to `range_end`.

#### `[env]`

- `template` - Path to Tera template file (relative to repo root)
- `output` - Output filename for rendered template (relative to worktree)

#### `[worktrees]`

- `directory` - Parent directory for worktrees (default: "../")

Worktrees are created as `directory/<branch-name>`.

#### `[artifacts]`

- `symlink` - List of paths to symlink from repo root to worktree
- `copy` - List of paths to copy from repo root to worktree

#### `[hooks]`

- `post_create` - List of shell commands to run after worktree creation

## Template System

`hydra-wt` uses Tera templating to generate per-worktree environment files.

### Template Variables

Available in templates:

| Variable | Type | Description |
|----------|------|-------------|
| `port` | `u16` | Allocated port for this worktree |
| `worktree` | `string` | Branch/worktree name |
| `project_uuid` | `string` | UUID from `.hydra/config.toml` |
| `repo_root` | `string` | Absolute path to repository root |

### Example Template

Create `.env.template` in your repo root:

```env
# Service configuration
PORT={{ port }}
NODE_ENV=development

# Worktree info
WORKTREE_NAME={{ worktree }}
PROJECT_ID={{ project_uuid }}

# Paths
REPO_ROOT={{ repo_root }}
```

After running `hydra-wt create feature-auth`, the worktree will contain `.env.local`:

```env
# Service configuration
PORT=3001
NODE_ENV=development

# Worktree info
WORKTREE_NAME=feature-auth
PROJECT_ID=abc-123-def

# Paths
REPO_ROOT=/home/user/dev/myproject
```

## Merge Workflow

The merge command provides a safe workflow for integrating feature branches.

### 1. Preview Merge

Check what would be merged and detect conflicts:

```bash
hydra-wt merge feature-auth main --dry-run
```

Output:
```
Merge preview: feature-auth → main
3 commit(s) to merge:

  a1b2c3d Add user authentication
  d4e5f6g Fix login bug
  h7i8j9k Update tests

✓ Merge can proceed without conflicts
```

### 2. Perform Merge

```bash
hydra-wt merge feature-auth main --force
```

Output:
```
Merging feature-auth into main...
✓ Merge successful (commit: f1e2d3c)
```

### 3. Handle Conflicts (if any)

If conflicts occur:

```
⚠️  Merge conflict in 2 file(s):
  - src/auth.rs
  - tests/auth_test.rs

Resolve conflicts in: ../main
Then run: cd ../main && git add . && git commit
Or abort: hydra-wt merge-abort main
```

Resolve conflicts manually, then:

```bash
cd ../main
# Edit conflicted files...
git add .
git commit
```

Or abort:

```bash
hydra-wt merge-abort main
```

### 4. Cleanup (optional)

Automatically remove the source worktree after successful merge:

```bash
hydra-wt merge feature-auth main --force --cleanup
```

## Hydra-Mail Integration

`hydra-wt` integrates with `hydra-mail` to emit events for cross-agent coordination.

### Events Emitted

| Event | Channel | When |
|-------|---------|------|
| `worktree_created` | `sys:registry` | After worktree creation |
| `worktree_removed` | `sys:registry` | After worktree removal |
| `merge_started` | `sys:registry` | Before merge operation |
| `merge_completed` | `sys:registry` | After successful merge |
| `merge_conflict` | `sys:registry` | When merge conflicts detected |

### Event Examples

**On create:**
```json
{"type":"worktree_created","worktree":"feature-auth","port":3001,"path":"../feature-auth"}
```

**On remove:**
```json
{"type":"worktree_removed","worktree":"feature-auth"}
```

**On merge start:**
```json
{"type":"merge_started","source":"feature-auth","target":"main","commits":3}
```

**On merge completion:**
```json
{"type":"merge_completed","source":"feature-auth","target":"main","merge_commit":"a1b2c3d"}
```

**On merge conflict:**
```json
{
  "type":"merge_conflict",
  "source":"feature-auth",
  "target":"main",
  "target_worktree":"/path/to/main",
  "conflicted_files":["src/auth.rs","tests/auth_test.rs"]
}
```

### Graceful Degradation

If `hydra-mail` is not installed, `hydra-wt` continues to work normally. Events are silently skipped with a warning.

## Port Registry

Port allocations are tracked in `.hydra/wt-ports.json`:

```json
{
  "feature-auth": 3001,
  "feature-billing": 3002,
  "main": 3003
}
```

Ports are freed when worktrees are removed. The registry prevents port conflicts.

## Artifacts and Hooks

### Artifacts

Automatically share files between repo root and worktrees:

```toml
[artifacts]
symlink = ["node_modules", ".cache"]  # Create symlinks
copy = ["config.local.json"]           # Copy files
```

Symlinks are useful for large directories (node_modules, build caches) to save disk space.

### Hooks

Run commands after worktree creation:

```toml
[hooks]
post_create = ["npm install", "npm run build"]
```

Hooks execute from the worktree directory.

## Library API

`hydra-wt` can be used as a Rust library:

```toml
[dependencies]
hydra-wt = { path = "./hydra-wt" }
```

```rust
use hydra_wt::{config, ports, worktree};

// Load configuration
let cfg = config::WtConfig::load()?;

// Allocate a port
let mut registry = ports::PortRegistry::load()?;
let port = registry.allocate("feature-x", 3001, 3099)?;

// Create worktree
let wt_path = cfg.worktree_path("feature-x");
worktree::add(&wt_path, "feature-x")?;

// Free port
registry.free("feature-x");
```

## Troubleshooting

### "Config not found" Error

```bash
# Initialize hydra-wt first
hydra-wt init
```

### ".hydra/ directory not found" Error

```bash
# Need to run hydra-mail init first
hydra-mail init --daemon
hydra-wt init
```

### Port Conflicts

```bash
# Check what's allocated
cat .hydra/wt-ports.json

# Remove stale worktrees
hydra-wt list
hydra-wt remove -f stale-branch
```

### Merge Conflicts

```bash
# Preview first
hydra-wt merge feature-x main --dry-run

# If conflicts, abort and resolve
hydra-wt merge-abort main

# Or resolve manually
cd ../main
# Edit conflicted files...
git add .
git commit
```

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
    ├── hydra.rs         # Hydra Mail event emission
    ├── artifacts.rs     # Symlink/copy artifacts
    └── hooks.rs         # Post-create hook execution
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| clap | CLI argument parsing |
| serde + serde_json | Serialization |
| toml | Config file parsing |
| tera | Template rendering |
| anyhow | Error handling |
| uuid | UUID reading |

## Platform Support

- **Linux** - Full support
- **macOS** - Full support
- **Windows** - Not supported (Unix sockets, git worktrees)

## License

MIT - See [LICENSE](../LICENSE) for details.

## Related

- [hydra-mail](../hydra-mail/) - Pub/sub messaging for agent coordination
- [hydra-cli](../hydra-cli/) - CLI orchestration tool
- [hydra-orchestrator](../hydra-orchestrator/) - Multi-agent coordination
