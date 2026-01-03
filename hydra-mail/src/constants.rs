//! Hydra Mail constants

/// Maximum size for a single message in bytes (10KB)
pub const MAX_MESSAGE_SIZE: usize = 10_240;

/// Maximum size for stdin input in bytes (100KB)
pub const MAX_STDIN_SIZE: usize = 102_400;

/// Replay buffer capacity (messages per channel)
pub const REPLAY_BUFFER_CAPACITY: usize = 100;

/// Broadcast channel capacity (concurrent in-flight messages)
pub const BROADCAST_CHANNEL_CAPACITY: usize = 1024;

/// Socket file permissions (owner read/write only)
pub const SOCKET_PERMISSIONS: u32 = 0o600;

/// Hydra directory permissions (owner read/write/execute only)
pub const HYDRA_DIR_PERMISSIONS: u32 = 0o700;

/// Daemon binary permissions (owner read/write/execute only)
pub const DAEMON_BINARY_PERMISSIONS: u32 = 0o700;

/// Config shell script permissions (owner read/write/execute only)
pub const CONFIG_SH_PERMISSIONS: u32 = 0o755;
