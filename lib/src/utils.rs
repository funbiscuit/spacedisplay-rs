use std::path::{Path, PathBuf};

use byte_unit::Byte;

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

/// Returns all mount points in system
///
/// Some of them might be supported for scanning but should be excluded when
/// scanning another mount point
#[cfg(unix)]
pub fn get_excluded_paths() -> Vec<PathBuf> {
    //todo can add more supported fs
    let supported_fs = vec!["ext2", "ext3", "ext4", "vfat", "ntfs", "fuseblk"];

    let mut mounts: Vec<_> = proc_mounts::MountIter::new()
        .unwrap()
        .map(|mount| mount.unwrap())
        .filter(|mount| !supported_fs.contains(&mount.fstype.as_str()))
        .map(|mount| mount.dest)
        .collect();
    mounts.sort();

    let mut excluded = vec![];

    // collect only non overlapping mounts so we have less items
    // for example /dev/sda will not be added because /dev already skips /dev/sda
    for mount in mounts {
        if !excluded.iter().any(|p| mount.starts_with(p)) {
            excluded.push(mount);
        }
    }

    excluded
}

/// Returns stats about given path
///
/// Returns total and available space of partition that contains path
#[cfg(unix)]
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

    let total = statvfs.fragment_size() * statvfs.blocks();
    let available = statvfs.fragment_size() * statvfs.blocks_available();

    Some(MountStats {
        total: Byte::from_bytes(total as u64),
        available: Byte::from_bytes(available as u64),
        is_mount_point,
    })
}

#[cfg(windows)]
pub fn get_excluded_paths() -> Vec<PathBuf> {
    vec![]
}

/// Returns stats about given path
///
/// Returns total and available space of partition that contains path
#[cfg(windows)]
pub fn get_mount_stats<P: AsRef<Path>>(path: P) -> Option<MountStats> {
    use widestring::U16CString;
    let is_mount_point = path.as_ref().parent().is_none();
    let path = U16CString::from_os_str(path.as_ref()).ok()?;

    let mut free_bytes = 0u64;
    let mut total_bytes = 0u64;
    // SAFETY: path is a valid widechar str and is null terminated
    // pointers to output variables are valid u64 pointers
    let status = unsafe {
        windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            path.as_ptr(),
            &mut free_bytes,
            &mut total_bytes,
            std::ptr::null_mut(),
        )
    };
    if status == 0 {
        None
    } else {
        Some(MountStats {
            is_mount_point,
            available: Byte::from_bytes(free_bytes),
            total: Byte::from_bytes(total_bytes),
        })
    }
}
