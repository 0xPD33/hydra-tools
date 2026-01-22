//! Hydra-wt library for worktree management
//!
//! This library provides worktree creation, removal, and port allocation
//! for use by other tools in the hydra ecosystem.

pub mod artifacts;
pub mod config;
pub mod hooks;
pub mod hydra;
pub mod ports;
pub mod template;
pub mod worktree;

// Re-export main types
pub use config::WtConfig;
pub use ports::PortRegistry;
pub use worktree::{WorktreeInfo, add, remove, exists, list};
