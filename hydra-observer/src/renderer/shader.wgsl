// Claude asterisk shader - cute animated mascot
// Matches the HTML/WebGL reference implementation

struct Uniforms {
    resolution: vec2<f32>,       // offset 0
    cursor_pos: vec2<f32>,       // offset 8 (mascot position)
    time: f32,                   // offset 16
    _pad0: f32,                  // offset 20 (alignment padding)
    mouse_pos: vec2<f32>,        // offset 24 (actual cursor position for eye tracking)
    velocity: vec2<f32>,         // offset 32
    hover_amount: f32,           // offset 40
    blink_amount: f32,           // offset 44
    poke_time: f32,              // offset 48
    alpha: f32,                  // offset 52
    scale: f32,                  // offset 56
    outline_amount: f32,         // offset 60 (0.0 to 1.0, animated)
    translucent_amount: f32,     // offset 64 (0.0 to 1.0, animated)
    pickup_phase: f32,           // offset 68 (0.0-1.0 during pickup, -1.0 otherwise)
    settling_phase: f32,         // offset 72 (0.0-1.0 during settling, -1.0 otherwise)
    rotation: f32,               // offset 76 (mascot rotation in radians)
    eye_scale_override: f32,     // offset 80 (eye size multiplier)
    _pad2: f32,                  // offset 84 (vec2 alignment)
    squash_stretch: vec2<f32>,   // offset 88 (XY scale for deformation)
    _pad3: vec4<f32>,            // offset 96 (16 bytes) -> total 112
}

@group(0) @binding(0)
var<uniform> u: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

const PI: f32 = 3.14159265359;
const NUM_SPOKES: i32 = 20;

// Rotation matrix
fn rot(a: f32) -> mat2x2<f32> {
    let c = cos(a);
    let s = sin(a);
    return mat2x2<f32>(c, -s, s, c);
}

// Spoke SDF
fn sd_spoke(p: vec2<f32>, len: f32, width1: f32, width2: f32) -> f32 {
    let t = clamp(p.y / len, 0.0, 1.0);
    let w = mix(width1, width2, t);
    var q = p;
    q.y -= clamp(p.y, 0.0, len);
    return length(q) - w;
}

// Circle SDF
fn sd_circle(p: vec2<f32>, r: f32) -> f32 {
    return length(p) - r;
}

// Smooth minimum
fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
    );

    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = positions[vertex_index] * 0.5 + 0.5;
    out.uv.y = 1.0 - out.uv.y;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let min_res = min(u.resolution.x, u.resolution.y);
    let uv = (in.uv * u.resolution - 0.5 * u.resolution) / min_res;

    let t = u.time;
    let hover_excitement = u.hover_amount;

    // Center follows cursor
    let center = (u.cursor_pos - 0.5 * u.resolution) / min_res;
    var p = (uv - center) * (2.0 / u.scale);  // Apply user-configurable scale

    // 1. Apply state-based rotation (drag tilt)
    p = rot(u.rotation) * p;

    // 2. Apply state-based squash/stretch (pickup/settle animations)
    p.x /= u.squash_stretch.x;
    p.y /= u.squash_stretch.y;

    // 3. Squash and stretch based on velocity (subtle, existing)
    let speed = length(u.velocity) * 0.0003;
    var vel_dir = u.velocity + vec2<f32>(0.001, 0.0);
    vel_dir = normalize(vel_dir);
    let stretch = 1.0 + speed * 0.1;
    let squash = 1.0 / sqrt(stretch);

    // Apply velocity squash/stretch in velocity direction
    let vel_angle = atan2(vel_dir.y, vel_dir.x);
    p = rot(-vel_angle) * p;
    p.x /= stretch;
    p.y /= squash;
    p = rot(vel_angle) * p;

    // Breathing - more pronounced when hovering
    let breathe_speed = 1.5 + hover_excitement * 1.5;
    let breathe_amt = 0.02 + hover_excitement * 0.015;
    let breathe = 1.0 + sin(t * breathe_speed) * breathe_amt;
    p /= breathe;

    // Eye look direction (toward actual mouse cursor)
    let mouse_center = (u.mouse_pos - 0.5 * u.resolution) / min_res;
    let look_dir_world = mouse_center - center;
    let look_distance = length(look_dir_world);

    // Normalize or use zero vector if too close
    var look_dir: vec2<f32>;
    if (look_distance > 0.01) {
        look_dir = normalize(look_dir_world);
    } else {
        look_dir = vec2<f32>(0.0, 0.0);
    }

    // === ASTERISK SPOKES ===
    var d: f32 = 1000.0;

    let spoke_width = 0.016;
    let core_radius = 0.048;
    let wiggle_amt = 0.025 + hover_excitement * 0.02;

    for (var i: i32 = 0; i < NUM_SPOKES; i++) {
        let fi = f32(i);
        var angle = fi * 2.0 * PI / f32(NUM_SPOKES);

        // Varying lengths
        let length_mod = 0.65 + 0.35 * sin(fi * 2.3 + 0.7);
        var spoke_len = 0.12 * length_mod;

        // Wobble
        let wobble_speed = 1.5 + hover_excitement * 0.8;
        let wobble = sin(t * wobble_speed + fi * 0.7) * (0.008 + hover_excitement * 0.006);
        spoke_len += wobble;

        let angle_wobble = sin(t * 1.2 + fi * 1.4) * wiggle_amt;
        angle += angle_wobble;

        var sp = rot(-angle) * p;
        sp.y -= core_radius;

        let base_w = spoke_width * (1.0 + sin(t * 0.9 + fi) * 0.12);
        let tip_w = base_w * 0.35;

        let spoke = sd_spoke(sp, spoke_len, base_w, tip_w);
        d = smin(d, spoke, 0.014);
    }

    // Core
    let core = sd_circle(p, core_radius);
    d = smin(d, core, 0.022);

    // === COLORS ===
    let claude_orange = vec3<f32>(0.851, 0.467, 0.341);
    let claude_orange_light = vec3<f32>(0.95, 0.6, 0.5);

    var col = vec3<f32>(0.0);
    var alpha: f32 = 0.0;

    // Outer glow (subtle)
    var glow = exp(-d * 10.0) * 0.3;
    glow += exp(-d * 25.0) * 0.2;
    col += claude_orange * glow;
    alpha += glow * 0.5;

    // Excited extra glow
    if (hover_excitement > 0.0) {
        let excited_glow = exp(-d * 6.0) * 0.15 * hover_excitement;
        col += claude_orange_light * excited_glow;
        alpha += excited_glow * 0.3;
    }

    // Main shape
    let shape = 1.0 - smoothstep(-0.002, 0.003, d);
    col = mix(col, claude_orange, shape);
    alpha = mix(alpha, 1.0, shape);

    // Inner gradient for depth
    let inner_grad = smoothstep(0.15, 0.0, length(p));
    col = mix(col, claude_orange_light, inner_grad * 0.15 * shape);

    // === CUTE EYES ===
    let eye_spacing = 0.024;
    let eye_y = 0.007;
    let eye_radius = 0.020;
    let pupil_radius = 0.011;

    let left_eye_pos = vec2<f32>(-eye_spacing, eye_y);
    let right_eye_pos = vec2<f32>(eye_spacing, eye_y);

    // Pupils follow cursor
    let max_pupil_move = 0.007;
    let pupil_offset = look_dir * max_pupil_move;

    // Eye scale combines excitement and state override (surprise/normal)
    let excitement_scale = 1.0 + hover_excitement * 0.15;
    let combined_scale = excitement_scale * u.eye_scale_override;
    let this_eye_radius = eye_radius * combined_scale;
    let this_pupil_radius = pupil_radius * combined_scale;

    // Blink (use uniform or compute)
    var blink = u.blink_amount;

    // Happy squint when hovering
    if (hover_excitement > 0.0) {
        blink *= mix(1.0, 0.7, hover_excitement * (0.5 + 0.5 * sin(t * 3.0)));
    }

    // Eye whites
    var left_eye_p = p - left_eye_pos;
    var right_eye_p = p - right_eye_pos;
    left_eye_p.y /= max(blink, 0.08);
    right_eye_p.y /= max(blink, 0.08);

    let left_eye_d = sd_circle(left_eye_p, this_eye_radius);
    let right_eye_d = sd_circle(right_eye_p, this_eye_radius);

    let eye_white_l = (1.0 - smoothstep(-0.002, 0.003, left_eye_d)) * shape;
    let eye_white_r = (1.0 - smoothstep(-0.002, 0.003, right_eye_d)) * shape;

    col = mix(col, vec3<f32>(1.0), eye_white_l);
    col = mix(col, vec3<f32>(1.0), eye_white_r);

    // Pupils
    var left_pupil_p = p - left_eye_pos - pupil_offset;
    var right_pupil_p = p - right_eye_pos - pupil_offset;
    left_pupil_p.y /= max(blink, 0.08);
    right_pupil_p.y /= max(blink, 0.08);

    let left_pupil_d = sd_circle(left_pupil_p, this_pupil_radius);
    let right_pupil_d = sd_circle(right_pupil_p, this_pupil_radius);

    let pupil_l = (1.0 - smoothstep(-0.002, 0.003, left_pupil_d)) * eye_white_l;
    let pupil_r = (1.0 - smoothstep(-0.002, 0.003, right_pupil_d)) * eye_white_r;

    let pupil_color = vec3<f32>(0.15, 0.1, 0.18);
    col = mix(col, pupil_color, pupil_l);
    col = mix(col, pupil_color, pupil_r);

    // Highlights - two per eye for extra cuteness
    let hl_off1 = vec2<f32>(-0.005, 0.006);
    let hl_off2 = vec2<f32>(0.003, -0.003);

    let hl_l1 = sd_circle(p - left_eye_pos - pupil_offset + hl_off1, 0.004);
    let hl_r1 = sd_circle(p - right_eye_pos - pupil_offset + hl_off1, 0.004);
    let hl_l2 = sd_circle(p - left_eye_pos - pupil_offset + hl_off2, 0.002);
    let hl_r2 = sd_circle(p - right_eye_pos - pupil_offset + hl_off2, 0.002);

    let highlight_l1 = (1.0 - smoothstep(-0.001, 0.002, hl_l1)) * pupil_l;
    let highlight_r1 = (1.0 - smoothstep(-0.001, 0.002, hl_r1)) * pupil_r;
    let highlight_l2 = (1.0 - smoothstep(-0.001, 0.002, hl_l2)) * pupil_l;
    let highlight_r2 = (1.0 - smoothstep(-0.001, 0.002, hl_r2)) * pupil_r;

    col = mix(col, vec3<f32>(1.0), highlight_l1);
    col = mix(col, vec3<f32>(1.0), highlight_r1);
    col = mix(col, vec3<f32>(1.0), highlight_l2 * 0.7);
    col = mix(col, vec3<f32>(1.0), highlight_r2 * 0.7);

    // Blush when hovering - subtle pink cheeks
    if (hover_excitement > 0.0) {
        let left_cheek = vec2<f32>(-0.045, -0.012);
        let right_cheek = vec2<f32>(0.045, -0.012);
        let blush_l = exp(-length(p - left_cheek) * 40.0);
        let blush_r = exp(-length(p - right_cheek) * 40.0);
        let blush_color = vec3<f32>(1.0, 0.4, 0.5);
        col = mix(col, blush_color, (blush_l + blush_r) * 0.35 * hover_excitement * shape);
    }

    // Poke reaction - bounce effect
    if (u.poke_time >= 0.0 && u.poke_time < 0.5) {
        let poke_t = u.poke_time / 0.5;
        let bounce = sin(poke_t * PI) * 0.02 * (1.0 - poke_t);
        // Could apply additional visual effect here
    }

    // Hover outline - bright ring around mascot when pointer is over it
    if (u.outline_amount > 0.01) {
        // Create an outline by checking if we're near the edge of the shape
        let outline_thickness = 0.012;
        let outline_inner = smoothstep(0.0, 0.004, d);
        let outline_outer = 1.0 - smoothstep(outline_thickness, outline_thickness + 0.004, d);
        let outline_strength = outline_inner * outline_outer;

        // Pulsing effect for visibility (subtle)
        let pulse = 0.5 + 0.2 * sin(t * 3.0);
        let outline_color = vec3<f32>(0.9, 0.7, 1.0); // Soft purple/white
        col = mix(col, outline_color, outline_strength * pulse * 0.3 * u.outline_amount);
        alpha = max(alpha, outline_strength * pulse * 0.4 * u.outline_amount);
    }

    // Apply global alpha
    alpha *= u.alpha;

    // Make translucent when in picking mode (dragging + shift)
    if (u.translucent_amount > 0.01) {
        // Fade from opaque (1.0) to 40% opacity (0.4) based on transition
        let target_opacity = mix(1.0, 0.4, u.translucent_amount);
        alpha *= target_opacity;
    }

    return vec4<f32>(col * alpha, alpha);
}
