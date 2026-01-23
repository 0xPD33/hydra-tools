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
pub use worktree::{
    WorktreeInfo, MergeResult, CommitInfo,
    add, remove, exists, list,
    merge, merge_abort, commits_ahead, merge_base, can_merge,
    has_uncommitted_changes, is_merge_in_progress,
    get_current_branch, get_head_commit, branch_exists, get_worktree_path,
};
