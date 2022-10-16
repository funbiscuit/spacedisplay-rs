use byte_unit::Byte;

#[cfg(target_os = "linux")]
pub use linux::*;
#[cfg(target_os = "macos")]
pub use macos::*;
#[cfg(unix)]
pub use unix::*;
#[cfg(windows)]
pub use windows::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[derive(Debug)]
pub struct MountStats {
    /// Total size of partition
    pub total: Byte,

    /// Available space on partition
    pub available: Byte,

    /// Whether info was requested for mount point (true)
    /// or for some directory inside mount point
    pub is_mount_point: bool,
}
