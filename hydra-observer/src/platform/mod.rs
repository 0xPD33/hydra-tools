//! Platform abstraction layer

#[cfg(feature = "wayland")]
pub mod wayland;

#[cfg(feature = "x11")]
pub mod x11;

/// Detected platform type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlatformType {
    Wayland,
    X11,
}
