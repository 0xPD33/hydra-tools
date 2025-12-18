# Claude Overlay — Design Document

## Overview

A cross-platform desktop application that renders an animated Claude asterisk icon as a transparent overlay. The icon follows the cursor, detects terminal windows beneath it, and reacts with visual feedback (excitement, blush, wiggle) when hovering over terminals.

**Primary Platform**: Linux (Wayland via layer-shell, X11 fallback)
**Secondary Platform**: macOS
**Tertiary Platform**: Windows (not prioritized)

---

## Goals

1. Minimal resource usage — idle when not moving
2. Sub-frame latency cursor tracking
3. Native Wayland support (no XWayland)
4. Single binary, no runtime dependencies beyond system libraries
5. Delightful, polished animation

---

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              Application                                 │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐                 │
│  │   Input     │    │    Core     │    │  Renderer   │                 │
│  │  Handler    │───▶│   State     │───▶│   (wgpu)    │                 │
│  └─────────────┘    └─────────────┘    └─────────────┘                 │
│         ▲                  │                  │                         │
│         │                  ▼                  ▼                         │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐                 │
│  │  Platform   │    │   Window    │    │   Frame     │                 │
│  │  Abstraction│◀───│   Tracker   │    │   Output    │                 │
│  └─────────────┘    └─────────────┘    └─────────────┘                 │
│         │                                     │                         │
├─────────┴─────────────────────────────────────┴─────────────────────────┤
│                        Platform Layer                                    │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐         │
│  │     Wayland     │  │       X11       │  │      macOS      │         │
│  │  (layer-shell)  │  │   (XComposite)  │  │  (NSWindow)     │         │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘         │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Module Structure

```
claude-overlay/
├── Cargo.toml
├── flake.nix
├── src/
│   ├── main.rs                 # Entry point, CLI parsing, platform detection
│   │
│   ├── app.rs                  # Application state machine
│   │
│   ├── config.rs               # Configuration loading/defaults
│   │
│   ├── core/
│   │   ├── mod.rs
│   │   ├── state.rs            # Central application state
│   │   ├── animation.rs        # Animation timing, easing, state transitions
│   │   └── geometry.rs         # Position, velocity, bounds calculations
│   │
│   ├── input/
│   │   ├── mod.rs
│   │   ├── pointer.rs          # Pointer position, velocity tracking
│   │   └── commands.rs         # Keyboard shortcuts, IPC commands
│   │
│   ├── renderer/
│   │   ├── mod.rs
│   │   ├── context.rs          # wgpu device, queue, surface management
│   │   ├── pipeline.rs         # Render pipeline setup
│   │   ├── uniforms.rs         # Shader uniform definitions
│   │   └── shader.wgsl         # Claude asterisk shader
│   │
│   ├── window_tracker/
│   │   ├── mod.rs              # Trait definition for window tracking
│   │   ├── wayland.rs          # foreign-toplevel-management
│   │   ├── x11.rs              # X11 window enumeration
│   │   └── macos.rs            # Accessibility API
│   │
│   └── platform/
│       ├── mod.rs              # Platform trait definition
│       ├── wayland/
│       │   ├── mod.rs
│       │   ├── layer_surface.rs    # Layer shell surface management
│       │   ├── pointer.rs          # Pointer handling
│       │   ├── output.rs           # Multi-monitor support
│       │   └── registry.rs         # Global registry handling
│       ├── x11/
│       │   ├── mod.rs
│       │   ├── overlay_window.rs   # Transparent override-redirect window
│       │   └── pointer.rs          # XInput2 pointer tracking
│       └── macos/
│           ├── mod.rs
│           └── overlay_window.rs   # NSWindow with appropriate flags
│
└── assets/
    └── (optional embedded assets)
```

---

## Core State

### `ClaudeState`

The central state structure, updated each frame:

```
ClaudeState
├── position: Vec2              # Current rendered position (smoothed)
├── target_position: Vec2       # Raw cursor position (target)
├── velocity: Vec2              # Movement velocity for squash/stretch
├── 
├── animation_time: f32         # Monotonic time for shader
├── hover_state: HoverState     # What we're hovering over
├── hover_transition: f32       # 0.0 = not hovering, 1.0 = fully hovering (animated)
├── 
├── blink_timer: f32            # Time until next blink
├── blink_state: BlinkState     # Open, Closing, Closed, Opening
├── 
├── poke_time: Option<f32>      # Time since last "poke" interaction
└── visibility: Visibility      # Visible, FadingOut, Hidden, FadingIn
```

### `HoverState`

```
HoverState
├── None                        # Not over any tracked window
├── Terminal {                  # Over a terminal
│       window_id: u64,
│       app_id: String,         # e.g., "Alacritty", "kitty"
│       title: String,
│   }
└── Other {                     # Over a non-terminal window (future use)
        window_id: u64,
        app_id: String,
    }
```

---

## Platform Abstraction

### `Platform` Trait

```
trait Platform {
    /// Initialize the platform, create overlay surface
    fn new(config: &Config) -> Result<Self>;
    
    /// Run the event loop (takes ownership)
    fn run(self, app: App) -> !;
    
    /// Get the raw handles for wgpu surface creation
    fn raw_display_handle(&self) -> RawDisplayHandle;
    fn raw_window_handle(&self) -> RawWindowHandle;
    
    /// Surface dimensions
    fn surface_size(&self) -> (u32, u32);
    
    /// Request a redraw
    fn request_redraw(&self);
    
    /// Set input region (for click-through)
    fn set_input_region(&self, region: Option<Rect>);
}
```

### `WindowTracker` Trait

```
trait WindowTracker {
    /// Start tracking windows
    fn new() -> Result<Self>;
    
    /// Get list of currently known terminal windows
    fn terminals(&self) -> &[TrackedWindow];
    
    /// Check if a screen position is over a terminal
    fn window_at_position(&self, pos: Vec2) -> Option<&TrackedWindow>;
    
    /// Update internal state (call from event loop)
    fn update(&mut self);
}

struct TrackedWindow {
    id: u64,
    app_id: String,
    title: String,
    geometry: Rect,         # Screen coordinates
    is_terminal: bool,
}
```

---

## Wayland Platform Detail

### Protocols Used

| Protocol | Interface | Purpose |
|----------|-----------|---------|
| `wayland` | `wl_compositor` | Create surfaces |
| `wayland` | `wl_surface` | The overlay surface |
| `wayland` | `wl_pointer` | Pointer events |
| `wayland` | `wl_output` | Monitor info |
| `wayland` | `wl_seat` | Input devices |
| `wlr-layer-shell-v1` | `zwlr_layer_shell_v1` | Create layer surface |
| `wlr-layer-shell-v1` | `zwlr_layer_surface_v1` | Configure layer |
| `wlr-foreign-toplevel-v1` | `zwlr_foreign_toplevel_manager_v1` | Window enumeration |

### Layer Surface Configuration

```
Layer:                  Overlay
Anchor:                 Top | Bottom | Left | Right
Size:                   (0, 0)  → fullscreen
Exclusive Zone:         -1      → don't reserve space
Keyboard Interactivity: None    → never steal focus
Namespace:              "claude-overlay"
```

### Multi-Monitor Strategy

**Option A: Single surface spanning all outputs**
- Simpler code
- May have compositor support issues
- One wgpu surface

**Option B: One layer surface per output** (Recommended)
- Better compositor compatibility
- Independent scaling per monitor
- Requires managing multiple surfaces

```
OutputManager
├── outputs: HashMap<OutputId, OutputState>
├── surfaces: HashMap<OutputId, LayerSurface>
└── 
    OutputState
    ├── name: String
    ├── geometry: Rect          # Position in global space
    ├── scale: f32
    ├── refresh: u32
    └── surface: Option<LayerSurface>
```

---

## Rendering Pipeline

### Shader Uniforms

```
struct Uniforms {
    resolution: vec2<f32>,      # Surface dimensions
    cursor_pos: vec2<f32>,      # Where to render Claude
    time: f32,                  # Animation time
    velocity: vec2<f32>,        # For squash/stretch
    hover_amount: f32,          # 0.0-1.0 excitement level
    blink_amount: f32,          # 0.0-1.0 eye openness
    poke_time: f32,             # Time since poke (for reaction)
}
```

### Render Flow

```
┌─────────────────┐
│  Frame Request  │
└────────┬────────┘
         ▼
┌─────────────────┐
│  Update State   │
│  - Smooth position
│  - Update velocity
│  - Advance animations
│  - Update hover transition
└────────┬────────┘
         ▼
┌─────────────────┐
│ Update Uniforms │
└────────┬────────┘
         ▼
┌─────────────────┐
│  Render Pass    │
│  - Clear to transparent
│  - Draw fullscreen quad
│  - Shader renders Claude
└────────┬────────┘
         ▼
┌─────────────────┐
│  Present Frame  │
└────────┬────────┘
         ▼
┌─────────────────┐
│ Damage Tracking │
│ (future optimization)
└─────────────────┘
```

### Frame Timing

```
┌─────────────────────────────────────────────┐
│              Frame Budget                    │
├─────────────────────────────────────────────┤
│ Target: vsync (typically 16.6ms @ 60Hz)     │
│                                             │
│ Event processing:    < 1ms                  │
│ State update:        < 0.5ms                │
│ Uniform upload:      < 0.1ms                │
│ GPU render:          < 2ms (mostly idle)    │
│ Present:             (compositor-bound)     │
│                                             │
│ Total CPU per frame: < 2ms                  │
└─────────────────────────────────────────────┘
```

---

## Animation System

### Smoothing

Cursor position uses exponential smoothing:

```
position += (target - position) * smoothing_factor * dt
```

Where `smoothing_factor` is tuned for responsive but smooth following (around 10-15).

### Velocity Calculation

```
velocity = (position - previous_position) / dt
```

Used for squash/stretch effect in shader.

### Hover Transition

```
if hovering_terminal:
    hover_amount = lerp(hover_amount, 1.0, transition_speed * dt)
else:
    hover_amount = lerp(hover_amount, 0.0, transition_speed * dt)
```

### Blink State Machine

```
        ┌──────────────────────────────────┐
        ▼                                  │
    ┌───────┐   timer    ┌─────────┐      │
    │ Open  │───expires──▶│ Closing │      │
    └───────┘             └────┬────┘      │
        ▲                      │           │
        │                      ▼           │
        │               ┌──────────┐       │
        │               │  Closed  │       │
        │               └────┬─────┘       │
        │                    │             │
        │                    ▼             │
        │               ┌──────────┐       │
        └───────────────│ Opening  │───────┘
                        └──────────┘
                        
Blink duration: ~150ms total
Time between blinks: 3-6 seconds (randomized)
```

---

## Window Detection

### Terminal Identification

Match `app_id` or `title` against known patterns:

```
TERMINAL_PATTERNS = [
    # App IDs (WM_CLASS on X11)
    "alacritty",
    "kitty", 
    "wezterm",
    "foot",
    "gnome-terminal",
    "konsole",
    "xterm",
    "urxvt",
    "st",
    "terminator",
    "tilix",
    "hyper",
    "tabby",
    "iterm2",           # macOS
    "terminal",         # macOS Terminal.app
    "warp",
    
    # Title patterns (fallback)
    "— fish",           # Fish shell in title
    "— bash",
    "— zsh",
]
```

### Geometry Challenge on Wayland

`wlr-foreign-toplevel-management` does NOT provide window geometry. Options:

**Option 1: Compositor-specific protocols**
- KDE has `org_kde_plasma_window_management` with geometry
- Hyprland has IPC with window positions
- Not portable

**Option 2: Heuristics with output info**
- Assume terminal is on same output as cursor
- Mark as "hovering terminal" if any terminal is focused
- Less precise but more portable

**Option 3: Hybrid approach** (Recommended)
- Use compositor-specific when available
- Fall back to "any terminal focused" heuristic
- Allow user configuration

---

## Configuration

### Config File Location

```
Linux:   $XDG_CONFIG_HOME/claude-overlay/config.toml
         ~/.config/claude-overlay/config.toml
macOS:   ~/Library/Application Support/claude-overlay/config.toml
```

### Configuration Schema

```toml
[appearance]
scale = 1.0                     # Size multiplier
glow_intensity = 0.5            # 0.0 - 1.0

[behavior]
smoothing = 12.0                # Cursor following smoothness
hover_transition_speed = 5.0    # How fast excitement ramps

[terminal_detection]
enabled = true
additional_patterns = ["my-custom-terminal"]
excluded_patterns = []

[advanced]
frame_rate = 0                  # 0 = vsync, otherwise target FPS
multi_monitor = "all"           # "all", "primary", "cursor"
```

---

## IPC / Control Interface

Optional Unix socket for external control:

```
Socket: $XDG_RUNTIME_DIR/claude-overlay.sock

Commands:
  show              # Make visible
  hide              # Make invisible
  toggle            # Toggle visibility
  poke              # Trigger poke animation
  set-excitement N  # Force excitement level (0.0-1.0)
  quit              # Exit application
  
Queries:
  status            # → {"visible": true, "hovering": "Alacritty", ...}
```

---

## Error Handling Strategy

### Graceful Degradation

```
┌─────────────────────────────────────────────────────────┐
│                    Startup Sequence                      │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  1. Detect session type (Wayland / X11 / macOS)        │
│         │                                               │
│         ├─── Wayland ──▶ Try layer-shell               │
│         │                    │                          │
│         │                    ├── Success ──▶ Continue   │
│         │                    │                          │
│         │                    └── Fail ──▶ Check XWayland│
│         │                                    │          │
│         │                         ┌──────────┘          │
│         │                         ▼                     │
│         ├─── X11 ──────▶ Use X11 backend               │
│         │                                               │
│         └─── macOS ────▶ Use macOS backend             │
│                                                         │
│  2. Initialize window tracker                           │
│         │                                               │
│         ├── Success ──▶ Full functionality             │
│         │                                               │
│         └── Fail ──▶ Disable hover detection           │
│                       (still works, just no reactions)  │
│                                                         │
│  3. Initialize renderer                                 │
│         │                                               │
│         ├── Vulkan ──▶ Preferred                       │
│         ├── Metal ──▶ macOS                            │
│         └── GL ──▶ Fallback                            │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### Runtime Errors

- **Surface lost**: Recreate surface, continue
- **Output removed**: Destroy that surface, continue on remaining
- **Window tracker disconnect**: Log warning, disable feature, continue
- **Render error**: Skip frame, retry next frame

---

## Build & Distribution

### NixOS (Primary)

```nix
# flake.nix provides:
- devShell with all dependencies
- Package with proper runtime library paths
- NixOS module for system integration (optional)
```

### Other Linux

```
Dependencies:
- Vulkan runtime (libvulkan.so)
- Wayland client libraries
- X11 libraries (for X11 backend)

Static linking where possible.
Runtime library discovery for Vulkan.
```

### macOS

```
App bundle with:
- Metal framework (system)
- Signed for accessibility permissions (window tracking)
```

---

## Future Considerations

### Not In Scope Now, But Designed For

1. **Multiple Claude instances** — State is per-surface, easy to extend
2. **Custom shaders** — Pipeline abstraction allows swapping shaders
3. **Themes** — Uniforms can be extended for color schemes
4. **Interaction modes** — Click handling infrastructure exists
5. **Terminal integration** — IPC socket enables external triggers

### Performance Optimizations (Later)

1. **Damage tracking** — Only redraw changed region
2. **Idle detection** — Reduce frame rate when cursor stationary
3. **Render resolution** — Render smaller, upscale when distant from cursor

---

## Success Criteria

1. ✅ Smooth 60fps cursor following on integrated GPU
2. ✅ < 50MB memory usage
3. ✅ < 2% CPU when cursor moving, < 0.1% when idle
4. ✅ Works on KDE Plasma Wayland (KWin)
5. ✅ Works on Hyprland
6. ✅ Works on Sway
7. ✅ Graceful degradation when compositor lacks protocols
8. ✅ Single binary, runs from `nix run`
