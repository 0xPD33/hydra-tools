//! Shader uniform definitions

use crate::core::ClaudeState;

/// Shader uniforms passed to the GPU
/// Layout must match WGSL struct alignment (vec2 requires 8-byte alignment)
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    /// Surface dimensions
    pub resolution: [f32; 2],       // offset 0, size 8

    /// Where to render Claude (screen coordinates)
    pub cursor_pos: [f32; 2],       // offset 8, size 8

    /// Animation time (monotonic)
    pub time: f32,                  // offset 16, size 4

    /// Padding for velocity alignment (vec2 needs 8-byte alignment)
    pub _pad0: f32,                 // offset 20, size 4

    /// Raw mouse cursor position (for eye tracking)
    pub mouse_pos: [f32; 2],        // offset 24, size 8

    /// Movement velocity (for squash/stretch)
    pub velocity: [f32; 2],         // offset 32, size 8

    /// Excitement level (0.0-1.0)
    pub hover_amount: f32,          // offset 40, size 4

    /// Eye openness (0.0-1.0)
    pub blink_amount: f32,          // offset 44, size 4

    /// Time since poke (for reaction), -1.0 if no poke
    pub poke_time: f32,             // offset 48, size 4

    /// Overall visibility alpha
    pub alpha: f32,                 // offset 52, size 4

    /// Scale multiplier for mascot size
    pub scale: f32,                 // offset 56, size 4

    /// Outline transition amount (0.0 to 1.0, animated)
    pub outline_amount: f32,        // offset 60, size 4

    /// Translucent transition amount (0.0 to 1.0, animated)
    pub translucent_amount: f32,    // offset 64, size 4

    /// Pickup phase (0.0-1.0 during pickup, -1.0 otherwise)
    pub pickup_phase: f32,          // offset 68, size 4

    /// Settling phase (0.0-1.0 during settling, -1.0 otherwise)
    pub settling_phase: f32,        // offset 72, size 4

    /// Mascot rotation in radians
    pub rotation: f32,              // offset 76, size 4

    /// Eye size multiplier (1.0=normal, 1.3=surprised)
    pub eye_scale_override: f32,    // offset 80, size 4

    /// Padding for squash_stretch alignment (vec2 needs 8-byte alignment)
    pub _pad2: f32,                 // offset 84, size 4

    /// XY scale for state-based squash/stretch
    pub squash_stretch: [f32; 2],   // offset 88, size 8

    /// Final padding to maintain 16-byte alignment
    pub _pad3: [f32; 4],            // offset 96, size 16 -> total 112
}

impl Uniforms {
    /// Create uniforms from current state
    pub fn from_state(
        state: &ClaudeState,
        resolution: (u32, u32),
        scale: f32,
        mouse_pos: (f32, f32),
    ) -> Self {
        let (pickup_phase, settling_phase, rotation, eye_scale, squash_stretch) =
            state.get_drag_uniforms();

        Self {
            resolution: [resolution.0 as f32, resolution.1 as f32],
            cursor_pos: [state.position.x, state.position.y],
            time: state.animation_time,
            _pad0: 0.0,
            mouse_pos: [mouse_pos.0, mouse_pos.1],
            velocity: [state.velocity.x, state.velocity.y],
            hover_amount: state.hover_transition,
            blink_amount: state.blink.blink_amount,
            poke_time: state.poke_time.unwrap_or(-1.0),
            alpha: state.visibility_alpha,
            scale,
            outline_amount: state.outline_transition,
            translucent_amount: state.translucent_transition,
            pickup_phase,
            settling_phase,
            rotation,
            eye_scale_override: eye_scale,
            _pad2: 0.0,
            squash_stretch: [squash_stretch.x, squash_stretch.y],
            _pad3: [0.0, 0.0, 0.0, 0.0],
        }
    }
}
