use crate::platform::MountStats;
use byte_unit::Byte;
use std::path::{Path, PathBuf};
use windows_sys::Win32::Storage::FileSystem;
use windows_sys::Win32::System::WindowsProgramming;

/// Returns all drives that can be scanned
pub fn get_available_mounts() -> Vec<String> {
    // SAFETY: call is always safe
    let mut drive_mask = unsafe { FileSystem::GetLogicalDrives() };

    let mut name = [b'A', b':', b'\\', 0];
    let mut drives = vec![];

    for c in b'A'..=b'Z' {
        if (drive_mask & 1) != 0 {
            name[0] = c;

            // SAFETY: name is always a valid null terminated ascii string
            let drive_type = unsafe { FileSystem::GetDriveTypeA(name.as_ptr()) };
            match drive_type {
                WindowsProgramming::DRIVE_REMOVABLE
                | WindowsProgramming::DRIVE_FIXED
                | WindowsProgramming::DRIVE_REMOTE => {
                    // SAFETY: name is always a valid ascii string with length == 3
                    let name = unsafe { std::str::from_utf8_unchecked(&name.as_slice()[..3]) };
                    drives.push(name.to_string())
                }
                _ => {}
            }
        }
        drive_mask >>= 1;
    }

    drives
}

pub fn get_excluded_paths() -> Vec<PathBuf> {
    vec![]
}

/// Returns stats about given path
///
/// Returns total and available space of partition that contains path
pub fn get_mount_stats<P: AsRef<Path>>(path: P) -> Option<MountStats> {
    use widestring::U16CString;
    let is_mount_point = path.as_ref().parent().is_none();
    let path = U16CString::from_os_str(path.as_ref()).ok()?;

    let mut free_bytes = 0u64;
    let mut total_bytes = 0u64;
    // SAFETY: path is a valid widechar str and is null terminated
    // pointers to output variables are valid u64 pointers
    let status = unsafe {
        FileSystem::GetDiskFreeSpaceExW(
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
