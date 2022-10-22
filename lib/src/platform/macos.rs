use std::path::PathBuf;

use byte_unit::Byte;

use libproc::libproc::pid_rusage;
use libproc::libproc::pid_rusage::{PIDRUsage, RUsageInfoV0};

/// Returns all mount points that can be scanned
pub fn get_available_mounts() -> Vec<String> {
    mountpoints::mountpaths()
        .unwrap()
        .into_iter()
        .map(|p| p.to_str().unwrap().to_string())
        .collect()
}

/// Returns all mount points in system
///
/// Some of them might be supported for scanning but should be excluded when
/// scanning another mount point
pub fn get_excluded_paths() -> Vec<PathBuf> {
    mountpoints::mountpaths().unwrap()
}

pub fn get_used_memory() -> Option<Byte> {
    let info: RUsageInfoV0 = pid_rusage::pidrusage(std::process::id() as i32).ok()?;
    Some(Byte::from_bytes(info.memory_used() as u64))
}
