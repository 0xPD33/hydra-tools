# Hydra Tools

A collection of Rust tools for multi-agent coordination and collaboration.

## Projects

### [hydra-mail](hydra-mail/)

Lightweight in-memory pub/sub messaging system with TOON encoding for token-efficient agent communication.

- 🚀 <5ms latency - In-memory Tokio broadcast channels
- 💾 30-60% token savings - TOON encoding
- 🔒 Project isolation - UUID-scoped channels
- 📼 Replay buffer - Last 100 messages per channel
- 🎯 Zero dependencies - Pure Rust, no external brokers

**Status**: v0.1.0 (Phase 1 - Skills MVP)

**Links**:
- [hydra-mail README](hydra-mail/README.md) - Project overview
- [Installation Guide](hydra-mail/INSTALLATION.md) - Setup instructions
- [Architecture](hydra-mail/docs/ARCHITECTURE.md) - Design details
- [Specification](hydra-mail/docs/SPEC.md) - Full spec and roadmap

## Getting Started

Each project has its own README. Start with the project you're interested in:

```bash
cd hydra-mail
cat README.md
```

## Building

### All Projects
```bash
nix build .#all
# or
cargo build --release -p hydra-mail
```

### Specific Project
```bash
cd hydra-mail
nix build
# or
cargo build --release
```

## Repository Structure

```
hydra-tools/
├── hydra-mail/           # Main project (pub/sub messaging)
│   ├── src/
│   ├── tests/
│   ├── skills/
│   ├── docs/
│   ├── .claude-plugin/
│   ├── README.md         # Project-specific README
│   ├── Cargo.toml
│   └── ...
└── README.md             # This file (monorepo overview)
```

## License

MIT - See individual projects for details.

## Contributing

Issues and PRs welcome. See [0xPD33/hydra-tools](https://github.com/0xPD33/hydra-tools)
