# Hydra Observer

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](https://github.com/0xPD33/hydra-tools)

Animated Claude asterisk overlay that follows your cursor and reacts to your work environment.

## What is Hydra Observer?

Hydra Observer renders an animated Claude icon as a transparent desktop overlay. The mascot follows your cursor with smooth animations, blinks naturally, and reacts when you hover over terminal windows. You can drag it around, attach it to windows, and it becomes a friendly companion while you work.

**Key Features:**
- **GPU-accelerated rendering** - Custom WGSL shaders via wgpu (Vulkan/GL)
- **Smooth cursor tracking** - Exponential smoothing with velocity-based effects
- **Drag animations** - Squash/stretch on pickup, bounce on placement, tilt while moving
- **Eye tracking** - Eyes follow the cursor, natural blinking
- **Terminal reactions** - Detects terminals and reacts with excitement
- **Window attachment** - Shift+click to attach mascot to a specific window
- **Wayland native** - Layer-shell support for proper overlay behavior
- **X11 fallback** - Works on X11 too

## Quick Start

### Installation

```bash
# Using Nix (recommended)
nix build .#hydra-observer
./result/bin/hydra-observer

# Using Cargo (from hydra-observer directory)
cargo build --release
./target/release/hydra-observer
```

### Usage

Just run it:

```bash
hydra-observer
```

The mascot appears and starts following your cursor.

**Interactions:**
- **Click on mascot** - Pick it up (starts following cursor)
- **Click again** - Put it down at current position
- **Shift+click while dragging** - Attach to window under cursor (KDE/KWin)
- **Drag quickly** - Mascot tilts in direction of movement

**CLI Options:**

```bash
hydra-observer --help
hydra-observer --verbose          # Enable debug logging
hydra-observer --platform wayland # Force Wayland backend
hydra-observer --platform x11     # Force X11 backend
hydra-observer --config path.toml # Use custom config file
```

## Configuration

Config file location:
- Linux: `~/.config/hydra-observer/config.toml`

### Example Configuration

```toml
[appearance]
scale = 0.5              # Size multiplier (default: 0.5)
glow_intensity = 0.5     # Glow effect intensity 0.0-1.0

[behavior]
smoothing = 10.0                  # Cursor following smoothness (higher = snappier)
hover_transition_speed = 5.0      # How fast excitement ramps up/down

[terminal_detection]
enabled = true
additional_patterns = ["my-custom-terminal"]  # Add your terminal
excluded_patterns = []                         # Exclude false positives

[advanced]
frame_rate = 0           # 0 = vsync, otherwise target FPS
multi_monitor = "all"    # "all", "primary", or "cursor"
```

### Built-in Terminal Detection

Automatically detects these terminals:
- alacritty, kitty, wezterm, foot
- gnome-terminal, konsole, xterm, urxvt
- st, terminator, tilix, hyper, tabby
- iterm2, terminal (macOS), warp

## Platform Support

| Platform | Backend | Status |
|----------|---------|--------|
| Linux (Wayland) | layer-shell via winit | Full support |
| Linux (X11) | Override-redirect window | Basic support |
| macOS | - | Not yet implemented |
| Windows | - | Not planned |

### Wayland Compositors

Tested on:
- KDE Plasma (KWin) - Full support including window picker
- Hyprland - Should work (layer-shell)
- Sway - Should work (layer-shell)

**Note:** Window attachment (Shift+click) currently requires KWin's D-Bus interface.

## Architecture

```
hydra-observer/
├── src/
│   ├── main.rs           # Entry point, CLI, platform detection
│   ├── app.rs            # Application runner
│   ├── config.rs         # Configuration loading
│   ├── core/
│   │   ├── state.rs      # Central ClaudeState (position, animations, drag)
│   │   ├── animation.rs  # Easing functions, blink controller
│   │   └── geometry.rs   # Vec2, bounds calculations
│   ├── input/
│   │   ├── commands.rs   # Input command handling
│   │   ├── global_shortcut.rs    # xdg-portal shortcuts
│   │   ├── kwin_window_picker.rs # KWin window selection
│   │   └── tmux_monitor.rs       # Tmux session awareness
│   ├── renderer/
│   │   ├── context.rs    # wgpu device/queue setup
│   │   ├── pipeline.rs   # Render pipeline
│   │   ├── uniforms.rs   # Shader uniform struct
│   │   └── shader.wgsl   # Claude asterisk shader
│   └── platform/
│       ├── wayland/      # Wayland backend (layer-shell)
│       └── x11/          # X11 backend
```

## Animation System

The mascot has several animation states:

| State | Effect |
|-------|--------|
| **Idle** | Subtle breathing, occasional blinks |
| **Following** | Smooth cursor tracking with velocity |
| **Pickup** | Squash then stretch (0.15s) |
| **Holding** | Eyes widen, tilt based on movement speed |
| **Settling** | Bounce curve, blink, return to upright |
| **Hovering terminal** | Excitement transition |

## Building

### Dependencies

**Core:**
- Rust nightly (for some wgpu features)
- wgpu 24 (Vulkan/GL)
- winit with layer-shell support (SergioRibera's fork)

**Wayland:**
- smithay-client-toolkit
- wayland-client libraries

**X11 (optional):**
- x11rb
- libxcb

### With Nix

```bash
# Enter dev shell
nix develop

# Build
nix build .#hydra-observer

# Run
./result/bin/hydra-observer
```

### With Cargo

```bash
cd hydra-observer

# Build with all features
cargo build --release

# Build Wayland-only
cargo build --release --no-default-features --features wayland

# Build X11-only
cargo build --release --no-default-features --features x11
```

## Troubleshooting

### "No display server detected"

Set the appropriate environment variable:
```bash
export WAYLAND_DISPLAY=wayland-0  # For Wayland
export DISPLAY=:0                  # For X11
```

### Mascot not visible

Check if your compositor supports layer-shell (wlr-layer-shell-unstable-v1).

### Window picker not working

Window picker requires KWin's D-Bus interface. On other compositors, manual positioning is available.

### Poor performance

Try forcing a different GPU backend:
```bash
WGPU_BACKEND=vulkan hydra-observer
WGPU_BACKEND=gl hydra-observer
```

## Related

- [hydra-mail](../hydra-mail/) - Pub/sub messaging for agent coordination
- [hydra-wt](../hydra-wt/) - Worktree management with port allocation

## License

MIT - See [LICENSE](../LICENSE) for details.
