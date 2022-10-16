use std::path::PathBuf;

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
