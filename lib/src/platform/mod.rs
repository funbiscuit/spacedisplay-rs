use std::path::Path;

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

pub fn delete_path<T: AsRef<Path>>(path: T) -> bool {
    if !path.as_ref().exists() {
        false
    } else if path.as_ref().is_dir() {
        std::fs::remove_dir_all(path.as_ref()).is_ok()
    } else {
        std::fs::remove_file(path.as_ref()).is_ok()
    }
}
