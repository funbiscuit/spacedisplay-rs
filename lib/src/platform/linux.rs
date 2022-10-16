use std::path::PathBuf;

//todo can add more supported fs
const SUPPORTED_FS: &[&str] = &["ext2", "ext3", "ext4", "vfat", "ntfs", "fuseblk"];

/// Returns all mount points that can be scanned
pub fn get_available_mounts() -> Vec<String> {
    let mut mounts: Vec<_> = proc_mounts::MountIter::new()
        .unwrap()
        .map(|mount| mount.unwrap())
        .filter(|mount| SUPPORTED_FS.contains(&mount.fstype.as_str()))
        .filter_map(|mount| mount.dest.to_str().map(|s| s.to_string()))
        .collect();
    mounts.sort();

    mounts
}

/// Returns all mount points in system
///
/// Some of them might be supported for scanning but should be excluded when
/// scanning another mount point
pub fn get_excluded_paths() -> Vec<PathBuf> {
    let mut mounts: Vec<_> = proc_mounts::MountIter::new()
        .unwrap()
        .map(|mount| mount.unwrap())
        .filter(|mount| !SUPPORTED_FS.contains(&mount.fstype.as_str()))
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
