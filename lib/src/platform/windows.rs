use std::fs::Metadata;
use std::mem::MaybeUninit;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use byte_unit::Byte;

use widestring::{U16CStr, U16CString};
use windows_sys::Win32::Storage::FileSystem;
use windows_sys::Win32::System::ProcessStatus::PROCESS_MEMORY_COUNTERS;
use windows_sys::Win32::System::{ProcessStatus, WindowsProgramming};

use crate::platform::MountStats;

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

/// Retrieve file size
///
/// On windows return normal file size since retrieving actual size on disk
/// is much slower and not very useful.
///
/// For cloud files not stored locally return 0.
pub fn get_file_size(metadata: &Metadata) -> u64 {
    // The following potentially applicable flags were observed when using cloud storage apps:
    // - Dropbox 172.4.7555:
    //   - "online-only":
    //     - REPARSE_POINT | OFFLINE
    //   - "local":
    //     - most of the time, none of the potentially applicable flags are set
    //     - sometimes: OFFLINE is set
    // - OneDrive 23.076.0409.0001:
    //   - "Available when online":
    //     - RECALL_ON_DATA_ACCESS
    //   - "Available on this device":
    //     - none of the potentially applicable flags are set
    //   - "Sync pending":
    //     - REPARSE_POINT
    // The flags tested as potentially applicable were:
    // - REPARSE_POINT
    // - RECALL_ON_DATA_ACCESS
    // - RECALL_ON_OPEN
    // - VIRTUAL
    // - OFFLINE
    //
    // See also: https://learn.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants

    const VIRTUAL_FILE_ATTRIBUTES: u32 =
        FileSystem::FILE_ATTRIBUTE_REPARSE_POINT | FileSystem::FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS;

    if (metadata.file_attributes() & VIRTUAL_FILE_ATTRIBUTES) == 0 {
        metadata.file_size()
    } else {
        0
    }
}

pub fn get_long_path<T: AsRef<U16CStr>>(str: T) -> Option<U16CString> {
    let str = str.as_ref().as_ptr();
    // SAFETY: str is a valid wide string, this call will return required size of buffer
    let len = unsafe { FileSystem::GetLongPathNameW(str, std::ptr::null_mut(), 0) };
    if len == 0 {
        return None;
    }
    // when buffer is small, returned len includes null terminator
    let mut vec = vec![0u16; len as usize];
    // SAFETY: str is a valid wide string, vec is a valid buffer of required len
    let len = unsafe { FileSystem::GetLongPathNameW(str, vec.as_mut_ptr(), len) };
    // when chars are copied, len does not include null terminator
    if len + 1 == vec.len() as u32 {
        U16CString::from_vec(vec).ok()
    } else {
        None
    }
}

/// Returns stats about given path
///
/// Returns total and available space of partition that contains path
pub fn get_mount_stats<P: AsRef<Path>>(path: P) -> Option<MountStats> {
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

pub fn get_used_memory() -> Option<Byte> {
    // SAFETY: this call is always safe
    let handle = unsafe { windows_sys::Win32::System::Threading::GetCurrentProcess() };
    let mut counters = MaybeUninit::uninit();

    // SAFETY: counters is pointer to uninit memory of necessary size
    // it's okay for it to be uninit
    let status = unsafe {
        ProcessStatus::K32GetProcessMemoryInfo(
            handle,
            counters.as_mut_ptr(),
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
    };

    if status != 0 {
        // SAFETY: previous call returned success value => uninit memory was initialized
        let counters = unsafe { counters.assume_init() };
        Some(Byte::from_bytes(counters.WorkingSetSize as u64))
    } else {
        None
    }
}
