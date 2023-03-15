use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use byte_unit::Byte;

use crate::platform::MountStats;

/// Retrieve file size
///
/// On unix return file size on disk since its fast
pub fn get_file_size(metadata: &Metadata) -> u64 {
    metadata.blocks() * 512
}

/// Returns stats about given path
///
/// Returns total and available space of partition that contains path
pub fn get_mount_stats<P: AsRef<Path>>(path: P) -> Option<MountStats> {
    let statvfs = nix::sys::statvfs::statvfs(path.as_ref()).ok()?;
    let stat = nix::sys::stat::stat(path.as_ref()).ok()?;

    // path is considered mount point if it doesn't have parent
    // or it has parent but its device id is different
    let is_mount_point = path
        .as_ref()
        .parent()
        .and_then(|p| nix::sys::stat::stat(p).ok())
        .map(|s| s.st_dev != stat.st_dev)
        .unwrap_or(true);

    // these conversions are required on macos but not needed on linux
    #[allow(clippy::useless_conversion)]
    let total = statvfs.fragment_size() * u64::from(statvfs.blocks());
    #[allow(clippy::useless_conversion)]
    let available = statvfs.fragment_size() * u64::from(statvfs.blocks_available());

    Some(MountStats {
        total: Byte::from_bytes(total),
        available: Byte::from_bytes(available),
        is_mount_point,
    })
}
