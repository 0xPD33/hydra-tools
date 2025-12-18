# CLAUDE.md - Hydra WT Developer Guide

> Guidance for Claude Code when working with Hydra WT

## Quick Reference

### Essential Commands

```bash
# Build
nix build .#hydra-wt              # Nix build
cargo build --release              # Cargo build (from hydra-wt/)

# Test manually
cd /tmp && mkdir test && cd test
git init
mkdir .hydra && echo 'project_uuid = "test"' > .hydra/config.toml
hydra-wt init
hydra-wt create test-branch
hydra-wt list
hydra-wt remove -f test-branch

# Development
cargo fmt                          # Format code
cargo clippy                       # Lint
```

### Key Files

| File | LOC | Purpose |
|------|-----|---------|
| src/main.rs | ~230 | CLI entry, command handlers |
| src/config.rs | ~100 | .hydra/wt.toml load/save |
| src/ports.rs | ~70 | Port allocation registry |
| src/worktree.rs | ~115 | Git worktree operations |
| src/template.rs | ~35 | .env.template rendering |
| src/hydra.rs | ~60 | Hydra Mail event emission |

## Architecture

```
User runs: hydra-wt create feature-x
                    │
                    ▼
┌─────────────────────────────────────────────────┐
│  main.rs: CLI parsing (clap)                    │
│  - Parse args                                   │
│  - Route to cmd_create()                        │
└────────────────────┬────────────────────────────┘
                     │
        ┌────────────┼────────────┬───────────────┐
        ▼            ▼            ▼               ▼
┌──────────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐
│ config.rs    │ │ ports.rs │ │worktree.rs│ │template.rs│
│ Load wt.toml │ │ Allocate │ │ git wt   │ │ Render   │
│              │ │ port     │ │ add      │ │ .env     │
└──────────────┘ └──────────┘ └──────────┘ └──────────┘
                     │
                     ▼
          ┌──────────────────┐
          │ hydra.rs         │
          │ Emit to sys:reg  │
          │ (shell out)      │
          └──────────────────┘
```

## Code Navigation

### CLI Commands (main.rs)

| Command | Function | Line |
|---------|----------|------|
| init | `cmd_init()` | ~64 |
| create | `cmd_create()` | ~70 |
| list | `cmd_list()` | ~125 |
| remove | `cmd_remove()` | ~160 |
| status | `cmd_status()` | ~190 |

### Key Functions

| Function | Location | Purpose |
|----------|----------|---------|
| `WtConfig::load()` | config.rs | Load .hydra/wt.toml |
| `WtConfig::init()` | config.rs | Create default config |
| `PortRegistry::allocate()` | ports.rs | Get next free port |
| `PortRegistry::free()` | ports.rs | Release port |
| `worktree::add()` | worktree.rs | Git worktree add |
| `worktree::remove()` | worktree.rs | Git worktree remove |
| `template::render()` | template.rs | Tera template rendering |
| `hydra::emit_worktree_created()` | hydra.rs | Emit to Hydra Mail |

## Configuration Files

### `.hydra/wt.toml` (managed by hydra-wt)

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

### `.hydra/wt-ports.json` (managed by hydra-wt)

```json
{"feature-auth": 3001, "feature-billing": 3002}
```

### `.env.template` (user-created)

```
PORT={{port}}
WORKTREE={{worktree}}
PROJECT_UUID={{project_uuid}}
REPO_ROOT={{repo_root}}
```

## Common Tasks

### Adding a New CLI Command

1. Add variant to `Commands` enum in main.rs
2. Add match arm in `main()`
3. Implement `cmd_newcommand()` function

### Adding a New Template Variable

1. Add field to `TemplateContext` struct in template.rs
2. Insert into Tera context in `render()`
3. Provide value in `cmd_create()` when building context

### Modifying Port Allocation Logic

1. Edit `PortRegistry::allocate()` in ports.rs
2. Current: Sequential scan from range_start to range_end
3. Alternative: Random, or based on hash of branch name

### Adding New Hydra Events

1. Create event struct in hydra.rs (with `Serialize`)
2. Create emit function similar to `emit_worktree_created()`
3. Call from appropriate command handler

## Error Handling

- All functions return `anyhow::Result`
- User-facing errors printed without backtrace (main.rs handles this)
- Warnings for non-fatal issues (missing template, missing hydra-mail)

## Testing

Manual testing workflow:

```bash
# Setup test environment
cd /tmp
rm -rf test-wt && mkdir test-wt && cd test-wt
git init
mkdir .hydra
echo 'project_uuid = "test-uuid"' > .hydra/config.toml

# Test init
hydra-wt init
cat .hydra/wt.toml

# Test create (without template)
hydra-wt create test-feature
hydra-wt list

# Test create (with template)
echo 'PORT={{port}}' > .env.template
hydra-wt create test-feature-2
cat ../test-feature-2/.env.local

# Test remove
hydra-wt remove -f test-feature
hydra-wt remove -f test-feature-2
hydra-wt list

# Cleanup
cd /tmp && rm -rf test-wt
```

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| clap | 4 | CLI with derive macros |
| serde | 1 | Serialization |
| serde_json | 1 | JSON for port registry |
| toml | 0.8 | Config file parsing |
| tera | 1 | Template rendering |
| anyhow | 1 | Error handling |
| uuid | 1 | UUID type |

## Design Decisions

### Why shell out to hydra-mail?

- Avoids version coupling between crates
- Works regardless of how hydra-mail is installed
- Graceful degradation if not available

### Why Tera for templating?

- Mustache-style syntax familiar to most developers
- Lightweight, no runtime dependencies
- Good error messages for template syntax errors

### Why sequential port allocation?

- Simple and predictable
- Easy to debug (ports assigned in order)
- No collisions without external state

## Troubleshooting

### "Config not found" Error

```bash
# Ensure hydra-wt init was run
hydra-wt init

# Or check if .hydra/wt.toml exists
cat .hydra/wt.toml
```

### ".hydra/ directory not found" Error

```bash
# Need to run hydra-mail init first
hydra-mail init --daemon
hydra-wt init
```

### "git worktree remove failed" Error

```bash
# Worktree has modified/untracked files
# Use --force flag
hydra-wt remove --force branch-name
```

### Port Conflicts

```bash
# Check what's allocated
cat .hydra/wt-ports.json

# Manually edit if needed (not recommended)
# Better: remove stale worktrees
hydra-wt list
hydra-wt remove -f stale-branch
```

---

**Document Version**: v0.1.0
**Last Updated**: 2025-12-17
