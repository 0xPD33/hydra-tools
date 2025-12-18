//! Input handling - window picking and tmux detection

pub mod kwin_window_picker;
pub mod tmux_monitor;

pub use kwin_window_picker::WindowInfo;
pub use tmux_monitor::TmuxMonitor;
