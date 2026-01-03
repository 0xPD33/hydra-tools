# Hydra Observer

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](https://github.com/0xPD33/hydra-tools)

HydraMail integration for the [Mascots](https://github.com/0xPD33/mascots) desktop companion.

## What is Hydra Observer?

Hydra Observer connects the Mascots desktop companion to the HydraMail pub/sub system, enabling your mascot to react to agent communications and Hydra ecosystem events.

**Features:**
- Integrates Mascots with HydraMail channels
- Reacts to `repo:delta`, `team:alert`, `team:status` messages
- Shows agent activity through mascot animations
- Click-to-interact with Hydra ecosystem

## Quick Start

### Prerequisites

- [Mascots](https://github.com/0xPD33/mascots) - Core desktop companion
- [HydraMail](../hydra-mail/) - Agent pub/sub messaging

### Installation

```bash
# From the hydra-tools monorepo
nix build .#hydra-observer

# Or using Cargo
cargo build --release -p hydra-observer
```

### Usage

```bash
# Start the observer (requires HydraMail daemon running)
hydra-observer

# With verbose logging
hydra-observer --verbose

# Force specific platform
hydra-observer --platform wayland
```

## How It Works

Hydra Observer uses Mascots as its core rendering and interaction layer, then adds HydraMail integration:

1. **Mascots** handles:
   - GPU-accelerated rendering
   - Cursor tracking and animations
   - Window attachment
   - Terminal detection
   - Platform abstraction (Wayland/X11)

2. **Hydra Observer** adds:
   - HydraMail subscription to agent channels
   - State updates based on team messages
   - Hydra-specific reactions (new commits, alerts, etc.)
   - Integration with hydra-wt and other tools

## Configuration

Uses the standard Mascots config location:
- Linux: `~/.config/mascots/config.toml`

Additional Hydra-specific settings will be added in future versions.

## Related

- [Mascots](https://github.com/0xPD33/mascots) - Core desktop companion
- [hydra-mail](../hydra-mail/) - Pub/sub messaging for agent coordination
- [hydra-wt](../hydra-wt/) - Worktree management with port allocation

## License

MIT - See [LICENSE](../LICENSE) for details.
