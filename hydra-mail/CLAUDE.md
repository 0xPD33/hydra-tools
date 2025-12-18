# CLAUDE.md - Hydra Mail Developer Guide

> Comprehensive guidance for Claude Code when working with Hydra Mail

## Table of Contents

1. [Quick Reference](#quick-reference)
2. [Project Overview](#project-overview)
3. [Build System](#build-system)
4. [Architecture](#architecture)
5. [Development Workflow](#development-workflow)
6. [Testing](#testing)
7. [Code Navigation](#code-navigation)
8. [Common Tasks](#common-tasks)
9. [Troubleshooting](#troubleshooting)
10. [Contributing](#contributing)

## Quick Reference

### Essential Commands

```bash
# Build and Test
nix develop                # Enter dev environment (recommended)
cargo build --release      # Build release binary
cargo test                 # Run all tests
cargo clippy              # Lint code

# Project Usage
./target/release/hydra-mail init --daemon   # Initialize project
./target/release/hydra-mail status          # Check daemon status
./target/release/hydra-mail emit --channel repo:delta --type delta --data '{...}'
./target/release/hydra-mail subscribe --channel repo:delta

# Development
cargo test -- --nocapture  # Run tests with output
cargo test test_name       # Run specific test
cargo doc --open           # Generate and view documentation
cargo fmt                  # Format code
```

### Key Files at a Glance

| File | LOC | Purpose | Key Functions |
|------|-----|---------|--------------|
| src/main.rs | 592 | CLI + daemon | `main()`, `handle_conn()`, command handlers |
| src/channels.rs | 257 | Pub/sub system | `emit_and_store()`, `subscribe_broadcast()` |
| src/config.rs | 206 | Configuration | `init()`, `load()`, `generate_skill_yaml()` |
| src/schema.rs | 115 | Message schema | `Pulse::new()`, `validate_size()` |
| src/lib.rs | 84 | Module exports | Integration tests |
| src/toon.rs | 56 | TOON format | `MessageFormat` enum |

### Critical Locations

- **CLI Command Definitions**: src/main.rs:25-90
- **Daemon Connection Handler**: src/main.rs:537-592
- **Channel Map**: channels.rs:19-27 (global static)
- **Config Struct**: config.rs:12-17
- **Pulse Schema**: schema.rs:10-55
- **Skill YAML Template**: config.rs:53-157

## Project Overview

### What is Hydra Mail?

Hydra Mail is a **lightweight (1,310 LOC), in-memory pub/sub communication system** designed for local AI agent collaboration. It enables multiple agents (like Claude Code, Codex CLI wrappers) to coordinate through channels with minimal latency.

### Core Features

1. **Project-Aware**: Initializes per-project with `.hydra/` directory and unique UUID
2. **Daemon Mode**: Optional persistent daemon for shared channel state across processes
3. **Unix Domain Sockets**: Efficient inter-process communication (Linux/macOS only)
4. **Tokio Channels**: High-performance async in-memory pub/sub
5. **TOON Protocol**: Token-Oriented Object Notation (30-60% token savings vs JSON)
6. **Replay Buffer**: 100-message history per channel for late-joining subscribers
7. **Zero External Dependencies**: Core uses only std + minimal dependencies

### Design Philosophy

- **Local-Only**: Single-host pub/sub, no distributed coordination
- **Ephemeral by Default**: In-memory storage (sled durability is opt-in)
- **TOON-First**: No JSON fallback in v0.1.0
- **Project-Scoped**: UUID isolation prevents cross-project interference
- **Fast**: <5ms latency target, 1M+ events/sec (claimed, not verified)

### Current Status

- **Version**: 0.1.0
- **Maturity**: MVP complete, Phase 1 of 3-phase roadmap
- **Platforms**: Linux, macOS (Unix-only, no Windows support)
- **Use Cases**: Local agent coordination, development automation, AI agent messaging

## Build System

### Nix (Recommended)

Hydra Mail uses Nix for deterministic builds and reproducible development environments.

#### Quick Start

```bash
# Enter development shell with all dependencies
nix develop

# Build the project
nix build

# Run the binary
nix run

# Run tests via Nix check
nix run .#checks.${system}.default
```

#### Nix Configuration

**Inputs** (flake.nix):
- `nixpkgs` (unstable channel)
- `flake-utils` (multi-system builds)
- `rust-overlay` (oxalica overlay for latest Rust)
- `crane` (ipetkov crane for Rust builds)

**Outputs**:
- `packages.default`: Compiled `hydra-mail` binary
- `apps.default`: Runnable app via `nix run`
- `checks.default`: `cargo clippy` linting
- `devShells.default`: Development environment

**Development Shell Includes**:
- Rust toolchain (nightly with rustfmt, clippy, rust-src)
- Build tools: pkg-config, clang, cmake, mold (fast linker)
- Utilities: tree-sitter, cargo-dist, jq, fd, ripgrep, bat
- Environment: `RUST_SRC_PATH`, `RUST_BACKTRACE=1`

#### Why Nix?

- **Reproducible**: Same build on any machine
- **Isolated**: No system-wide dependency conflicts
- **Fast**: Cached builds via Nix store
- **Cross-Platform**: Works on Linux, macOS, NixOS

### Cargo (Traditional)

Standard Rust Cargo builds also work without Nix.

#### Quick Start

```bash
# Development build
cargo build

# Release build (recommended for testing)
cargo build --release

# Run binary
cargo run -- init --daemon

# Run tests
cargo test

# Run tests with verbose output
cargo test -- --nocapture

# Check with clippy
cargo clippy

# Format code
cargo fmt
```

#### Dependencies

**Core Dependencies** (Cargo.toml):

| Crate | Version | Purpose |
|-------|---------|---------|
| clap | 4.5 | CLI argument parsing with derive macros |
| tokio | 1.40 | Async runtime (multi-thread, sync, net, io-util) |
| serde | 1.0 | Serialization framework |
| serde_json | 1.0 | JSON serialization |
| uuid | 1.10 | UUID generation (v4) |
| toml | 0.8 | TOML config parsing |
| anyhow | 1.0 | Error handling with context |
| once_cell | 1.19 | Lazy static initialization |
| chrono | 0.4 | Timestamps (UTC) |
| toon-format | 0.3 | TOON encoding/decoding |
| base64 | 0.22 | Base64 encoding for transport |
| sled | 0.34 | Optional durability (feature-gated) |

**Dev Dependencies**:
- `tempfile = "3.10"` - Temporary directories for integration tests

**Features**:
- `default = []` (no features enabled by default)
- `durable = ["sled"]` (optional persistent storage)

#### Toolchain

**Rust Version**: Nightly (pinned in rust-toolchain.toml)

**Components**:
- `rustfmt` - Code formatting
- `clippy` - Linting
- `rust-src` - Source code for IDE integration

**Lints** (Cargo.toml):
```toml
[lints.rust]
unsafe_code = "deny"  # Forbid unsafe blocks
```

## Architecture

### High-Level Overview

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
│  │ - Parse JSON commands (emit, subscribe)              │  │
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

### Component Breakdown

See `docs/ARCHITECTURE.md` for complete details. Key components:

1. **CLI Binary** (`src/main.rs`, 592 LOC): Entry point with 6 commands (init, start, emit, subscribe, status, stop)
2. **Daemon Process** (`handle_conn()` in main.rs): Per-connection async handlers over Unix socket
3. **Channel System** (`src/channels.rs`, 257 LOC): Broadcast channels + replay buffer with project/topic isolation
4. **Configuration** (`src/config.rs`, 206 LOC): Project UUID, socket path, skill YAML generation
5. **Message Schema** (`src/schema.rs`, 115 LOC): Pulse struct with validation
6. **TOON Format** (`src/toon.rs`, 56 LOC): Token-efficient encoding

## Development Workflow

### Getting Started

1. **Clone repository**
   ```bash
   git clone https://github.com/yourusername/hydra-tools.git
   cd hydra-tools/hydra-mail
   ```

2. **Enter development environment**
   ```bash
   nix develop  # Recommended
   # OR
   # Ensure Rust nightly is installed
   ```

3. **Build the project**
   ```bash
   cargo build --release
   ```

4. **Test the binary**
   ```bash
   cd /tmp
   mkdir test-project && cd test-project
   /path/to/hydra-tools/hydra-mail/target/release/hydra-mail init --daemon
   /path/to/hydra-tools/hydra-mail/target/release/hydra-mail status
   ```

### Development Commands

```bash
# Code Quality
cargo fmt                  # Format code
cargo clippy              # Lint code
cargo clippy --fix        # Auto-fix lint warnings

# Documentation
cargo doc                  # Generate documentation
cargo doc --open          # Generate and open in browser

# Building
cargo build               # Debug build (fast, unoptimized)
cargo build --release     # Release build (slow, optimized)
cargo build --features durable  # Enable sled persistence

# Testing
cargo test                # Run all tests
cargo test -- --nocapture # Run with stdout/stderr
cargo test test_name      # Run specific test
cargo test --release      # Run in release mode (faster)

# Cleaning
cargo clean               # Remove build artifacts
```

### Code Style

**Rust Edition**: 2021

**Style Guidelines**:
- `#[deny(unsafe_code)]` - No unsafe blocks
- `anyhow::Result` for error handling
- `#[tokio::test]` for async tests
- Doc comments (`///`) on all public items
- Descriptive variable names (no single-letter except iterators)
- Prefer explicit over implicit (no `use *`)

**Error Handling**:
- Use `anyhow::Result` for functions that can fail
- Use `.context()` to add error context
- Use `?` for error propagation
- Log errors before returning (in daemon)

## Testing

### Test Categories

| Category | Location | Coverage |
|----------|----------|----------|
| Unit Tests | src/*.rs | Config, schema, TOON, channels |
| Integration Tests | tests/integration_test.rs | End-to-end workflows |
| Channel Tests | src/channels.rs | Pub/sub behavior, isolation |

### Running Tests

```bash
# Run all tests
cargo test

# Run with output (for debugging)
cargo test -- --nocapture

# Run specific test
cargo test test_init_creates_hydra

# Run integration tests only
cargo test integration_test

# Run in release mode (faster)
cargo test --release
```

### Key Tests

**Channel Tests** (channels.rs):
- `test_replay_buffer_capacity_limit`: Verifies FIFO eviction at 100 messages
- `test_multiple_channels_isolated`: Verifies topic isolation
- `test_different_projects_isolated`: Verifies UUID isolation

**Integration Tests** (integration_test.rs):
- `test_init_creates_hydra`: Verifies `.hydra/` creation
- `test_emit_subscribe_end_to_end`: Full workflow with daemon

## Code Navigation

### Critical Functions

| Function | Location | Purpose |
|----------|----------|---------|
| `main()` | main.rs:15-24 | Entry point, async runtime |
| `handle_conn()` | main.rs:537-592 | Daemon connection handler |
| `emit_and_store()` | channels.rs:44-62 | Atomic broadcast + buffer |
| `subscribe_broadcast()` | channels.rs:64-78 | Get receiver + history |
| `Config::init()` | config.rs:19-51 | Create .hydra/ directory |
| `Config::generate_skill_yaml()` | config.rs:53-157 | Generate Claude skill |

### Reading the Codebase

**Suggested Order**:

1. **schema.rs** (115 LOC): Understand `Pulse` message structure
2. **config.rs** (206 LOC): Understand `Config` and `.hydra/` initialization
3. **channels.rs** (257 LOC): Understand channel map and replay buffer
4. **toon.rs** (56 LOC): Understand TOON format
5. **main.rs** (592 LOC): Study CLI commands and daemon
6. **Tests**: See usage patterns

## Common Tasks

### Adding a New CLI Command

1. Add to `Commands` enum in main.rs:25-90
2. Add match arm in `main()` function
3. Add integration test in tests/integration_test.rs

### Modifying Skill YAML Template

1. Edit `Config::generate_skill_yaml()` in config.rs:53-157
2. Test with: `rm -rf .hydra && hydra-mail init`
3. Verify: `cat .hydra/skills/hydra-mail.yaml`

### Adding a New Channel Function

1. Add function to channels.rs
2. Export in lib.rs
3. Add unit test in channels.rs

### Modifying Message Schema

1. Edit `Pulse` struct in schema.rs:10-55
2. Update constructors
3. Update tests
4. Update CLI emit command in main.rs:254-373

## Troubleshooting

### Common Issues

#### 1. "Daemon not running" Error

**Symptoms**: `Error connecting to socket: No such file or directory`

**Solutions**:
```bash
# Check status
hydra-mail status

# Restart daemon
hydra-mail stop
hydra-mail start
```

#### 2. "Socket exists but cannot connect" Error

**Symptoms**: `Error connecting to socket: Connection refused`

**Solutions**:
```bash
# Check if process is alive
cat .hydra/daemon.pid
ps -p <pid>

# Cleanup manually
rm .hydra/hydra.sock .hydra/daemon.pid
hydra-mail start
```

#### 3. "Permission denied" Error

**Symptoms**: `Permission denied (os error 13)`

**Solutions**:
```bash
# Check permissions
ls -la .hydra/

# Fix permissions
chmod 700 .hydra
chmod 600 .hydra/hydra.sock
```

#### 4. "Message too large" Error

**Symptoms**: `Error: Message too large: 12345 bytes`

**Solution**: Reduce data payload size or modify `validate_size()` in schema.rs

### Debug Commands

```bash
# Check daemon status
hydra-mail status

# Manually test emit/subscribe
hydra-mail emit --channel test --type test --data '{"msg":"hello"}'
hydra-mail subscribe --channel test --once

# Check daemon logs
cat .hydra/daemon.err

# Verify config
cat .hydra/config.toml

# Clean up manually
rm -rf .hydra
```

## Contributing

### Pull Request Process

1. Fork repository
2. Create feature branch: `git checkout -b feature/my-feature`
3. Make changes (follow style guidelines)
4. Run checks: `cargo fmt --check && cargo clippy && cargo test`
5. Commit: `git commit -m "feat: add my feature"`
6. Push and create PR

### Code Review Checklist

- [ ] Code follows Rust best practices
- [ ] All tests pass (`cargo test`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] New functions have doc comments
- [ ] Documentation updated (CLAUDE.md, ARCHITECTURE.md)

### Development Best Practices

1. Write tests first (TDD approach)
2. Keep functions small (single responsibility)
3. Use descriptive names (no abbreviations)
4. Add error context (use `.context()`)
5. Document public APIs (use `///`)
6. Test on both Linux and macOS (if possible)

## Integration Guide

### For AI Agents (Claude Skills)

Skills should check for `.hydra` directory and auto-configure:

```bash
# In Skill commands, check for Hydra initialization
if [ -d ".hydra" ]; then
  hydra-mail emit --project .hydra --channel repo:delta --type delta --data @- --format toon
fi
```

### For Custom Agents

Create a wrapper script:

```bash
#!/bin/bash
set -euo pipefail
if [ -d ".hydra" ]; then
  hydra-mail emit --project .hydra --channel "${1:?channel}" --type "${2:?type}" --data "${3:?json}" --format toon
else
  echo "Hydra not initialized. Run: hydra init --daemon" >&2
  exit 1
fi
```

### Configuration Auto-Discovery

- Agents should check current directory (and parent directories) for `.hydra`
- Read `.hydra/config.toml` for project UUID and socket path
- Pass `--project .hydra` to all hydra-mail commands when found

## Performance Characteristics

- **Latency**: <5ms for local message delivery (claimed)
- **Throughput**: 1M+ events/sec (claimed)
- **Token Efficiency**: 30-60% reduction using TOON vs JSON (claimed)
- **Memory**: ~1MB binary, minimal overhead per channel
- **Message Size Limit**: 10KB per message (enforced)
- **Replay Buffer Size**: 100 messages per channel
- **Broadcast Capacity**: 1024 messages

## Roadmap

- **v0.1** (current): Basic pub/sub with TOON, replay buffer, daemon mode
- **v1.1**: Sled integration for durable queues
- **v1.2**: Full Unix socket proxy for daemon subcommands
- **v2**: MPSC channels, mode system (inject/loop/hybrid), SDK integration
- **Future**: Windows support (named pipes), metrics collection

---

**Document Version**: v0.1.0
**Last Updated**: 2025-11-26
**Maintainer**: Hydra Tools Contributors
