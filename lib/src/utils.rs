use std::path::PathBuf;

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

#[cfg(windows)]
pub fn get_excluded_paths() -> Vec<PathBuf> {
    vec![]
}
