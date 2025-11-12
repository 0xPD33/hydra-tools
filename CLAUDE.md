# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Hydra Mail is a lightweight, in-memory pub/sub communication system designed for local agent collaboration. It provides a publish-subscribe messaging system that allows multiple AI agents (like Claude Code, Codex CLI wrappers) to coordinate through channels with minimal latency.

### Key Features
- **Project-aware**: Initializes per-project with `.hydra` directory and configuration
- **Daemon Mode**: Optional persistent daemon for shared channel state across processes
- **Unix Domain Sockets**: Efficient inter-process communication using local sockets
- **Tokio Channels**: High-performance in-memory pub/sub using Rust's async runtime
- **TOON Protocol**: Token-Oriented Object Notation for efficient AI message passing (30-60% token savings vs JSON)
- **Zero External Dependencies**: Core functionality uses only Rust standard library and minimal dependencies

## Build System

### Nix (Recommended)
This project uses Nix for deterministic builds and development environment setup:

```bash
# Development shell with all dependencies
nix develop

# Build the project
nix build

# Run the binary
nix run

# Run tests
nix run .#checks.${system}.default

# Just build without flake
nix build .#default
```

### Cargo
Traditional Rust Cargo builds also work:

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with verbose output
cargo test -- --nocapture

# Check with clippy
cargo clippy
```

### Build Tools
- **Rust Toolchain**: Nightly with clippy and rustfmt
- **Nixpkgs**: Uses rust-overlay for latest Rust versions
- **Crane**: For Nix-based caching and reproducible builds
- **mold**: Fast linker on Linux (enabled in dev shell)

## Architecture

### Core Components

#### 1. CLI Binary (`hydra-mail`)
Main entry point with these subcommands:
- `init`: Initialize a project with Hydra integration
- `start`: Start the daemon in the project directory
- `stop`: Stop the running daemon
- `emit`: Publish a message to a channel (supports `--format toon` for token-efficient encoding)
- `subscribe`: Listen to messages on a channel (automatic TOON decoding)
- `status`: Show daemon status and project information

#### 2. Daemon Process
Optional persistent process that:
- Listens on Unix Domain Socket for commands
- Manages channel lifecycle (broadcast and mpsc channels)
- Enables sharing state between multiple agent processes
- Maintains project-scoped communication

#### 3. Channel System
- **Broadcast Channels**: One-to-many pub/sub (`tokio::sync::broadcast`)
- **MPSC Channels**: Point-to-point communication (`tokio::sync::mpsc`)
- Project-scoped using UUID to prevent cross-project interference
- Static channel storage with lazy initialization

#### 4. Configuration System
Project-specific config stored in `.hydra/config.toml` containing:
- `project_uuid`: Unique identifier for the project
- `socket_path`: Unix socket location for daemon communication
- `default_topics`: Default channels to initialize

### Data Flow

1. **Initialization**: `hydra init` creates `.hydra` directory and config
2. **Daemon Start**: Optional daemon spawns with channel managers
3. **Agent Integration**: Agents detect `.hydra` and auto-configure commands
4. **Message Emission**: `hydra emit` sends JSON via Unix socket to daemon
5. **Subscription**: `hydra subscribe` creates receiver streams
6. **Processing**: Daemon handles message routing to appropriate channels

## Testing

### Test Suite
The project includes comprehensive tests:

```bash
# Run all tests
cargo test

# Run integration tests
cargo test integration_test

# Run with output capture for debugging
cargo test -- --nocapture

# Run specific test
cargo test test_init_creates_hydra

# Run tests in release mode for performance
cargo test --release
```

### Test Categories
- **Unit Tests**: Individual module testing (`src/lib.rs`)
- **Integration Tests**: End-to-end workflow testing (`tests/`)
- **Channel Tests**: Tokio channel behavior validation
- **Configuration Tests**: Config file parsing and validation

### Test Features
- Uses `tempfile` for isolated test environments
- Spawns actual binary processes for integration testing
- Simulates real agent workflows
- Cleanup of test directories after execution

## Dependencies

### Core Dependencies
- `clap = "4.5"`: Command line interface with derive macros
- `tokio = "1.40"`: Async runtime with sync features
- `serde = "1.0"`: Serialization framework
- `uuid = "1.10"`: UUID generation and handling
- `toml = "0.8"`: TOML configuration parsing
- `anyhow = "1.0"`: Error handling
- `once_cell = "1.19"`: Lazy initialization
- `toon = "0.1"`: Token-Oriented Object Notation for efficient AI message passing

### Optional Features
- `sled = "0.34"`: Optional durable storage (feature "durable")

### Development Dependencies
- `tempfile = "3.10"`: Temporary file/directory creation for tests

## Development Workflow

### Getting Started
1. Clone repository
2. `nix develop` for dev environment or use Cargo directly
3. `cargo build` to build the project
4. In your project: `./target/release/hydra-mail init --daemon`

### Development Commands
```bash
# Format code
cargo fmt

# Lint code
cargo clippy

# Run documentation
cargo doc --open

# Build examples
cargo build --examples

# Test specific features
cargo test --features durable
```

### Code Style
- Rust 2021 edition
- `#[deny(unsafe_code)]` enforced
- `anyhow::Result` for error handling
- `#[tokio::test]` for async tests
- Comprehensive doc comments on public interfaces

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

## Project Structure

```
.
├── Cargo.toml              # Rust project configuration
├── flake.nix               # Nix build system configuration
├── rust-toolchain.toml    # Pinned Rust toolchain
├── CLAUDE.md               # This documentation
├── src/
│   ├── main.rs             # CLI entry point and main logic
│   ├── lib.rs              # Library exports and tests
│   ├── config.rs           # Configuration management
│   └── channels.rs         # Channel management and pub/sub logic
├── tests/
│   └── integration_test.rs # End-to-end integration tests
├── docs/
│   └── ARCHITECTURE.md     # Detailed architecture design
└── .hydra/                 # Runtime directory (created by init)
    ├── config.toml         # Project configuration
    ├── hydra.sock          # Unix domain socket
    └── daemon.pid          # Daemon process ID
```

## Environment Variables

- `RUST_BACKTRACE=1`: Enable Rust backtraces for debugging
- `HYDRA_KEY` (optional): HMAC key for message authentication (future)

## Security Considerations

- Unix Domain Sockets with restricted permissions (0600)
- Project isolation via UUID scoping
- Directory permissions enforced (0700 for `.hydra`)
- No external network communication by design
- Message size limits prevent abuse (1KB max)

## Performance Characteristics

- **Latency**: <5ms for local message delivery
- **Throughput**: 1M+ events/sec in benchmarks
- **Memory**: ~1MB binary, minimal overhead per channel
- **Scalability**: Handles 10-20 subscribers per channel efficiently
- **Token Efficiency**: 30-60% reduction in message size using TOON vs JSON
- **Durability**: Ephemeral by default, optional sled persistence

## Roadmap

- v1.1: Sled integration for durable queues
- v1.2: Full Unix socket proxy for daemon subcommands
- v2: Cross-process Unix socket support
- Future: Stateful watch channels, metrics collection

## Troubleshooting

### Common Issues
- **"Daemon not running"**: Run `hydra start` or `hydra init --daemon`
- **"Socket exists but cannot connect"**: Kill daemon and restart
- **Permission denied**: Check `.hydra` directory permissions
- **Cross-platform issues**: Currently Linux/macOS only

### Debug Commands
```bash
# Check daemon status
hydra-mail status

# Test emit/subscribe manually
hydra-mail emit --project .hydra --channel test --data '{"msg":"test"}'
hydra-mail subscribe --project .hydra --channel test --once

# Clean up manually
rm -rf .hydra
```

## Contributing

- Follow Rust best practices and clippy rules
- Add tests for new functionality
- Update documentation for breaking changes
- Test on Linux and macOS
- Use Nix environment for consistent development