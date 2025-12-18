//! Drag handling - mascot is clickable for drag toggling
//!
//! Simple approach: click on mascot to pick it up, click again to place it down.

use std::sync::mpsc;

/// Events from drag interaction
#[derive(Debug, Clone, Copy)]
pub enum DragEvent {
    /// Toggle drag mode (click on mascot)
    Toggle,
}

/// Create a channel for drag events (handled in wayland module via pointer events)
pub fn create_drag_channel() -> (mpsc::Sender<DragEvent>, mpsc::Receiver<DragEvent>) {
    mpsc::channel()
}

/// Calculate mascot bounding box for hit testing
/// Returns (x, y, width, height) in surface coordinates
pub fn mascot_bounds(cursor_x: f32, cursor_y: f32, screen_width: u32, screen_height: u32) -> (i32, i32, i32, i32) {
    // Mascot size calculation matching shader.wgsl
    // The shader scales by 2.0 (half size), and base size is ~0.35 of min dimension
    let min_res = screen_width.min(screen_height) as f32;
    let base_radius = min_res * 0.35 / 2.0; // Half size due to *2.0 scaling in shader

    // Add some padding for easier clicking
    let padding = 20.0;
    let size = (base_radius * 2.0 + padding) as i32;

    let x = (cursor_x - size as f32 / 2.0) as i32;
    let y = (cursor_y - size as f32 / 2.0) as i32;

    (x.max(0), y.max(0), size, size)
}
