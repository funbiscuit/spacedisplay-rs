use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::thread::JoinHandle;
use widestring::{U16CString, U16String};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem;
use windows_sys::Win32::Storage::FileSystem::{
    ReadDirectoryChangesW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OVERLAPPED, FILE_LIST_DIRECTORY,
    FILE_NOTIFY_CHANGE_DIR_NAME, FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_SIZE,
    FILE_NOTIFY_INFORMATION, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};

use crate::watcher::{FileEvent, Watcher, WatcherError};

const BUFFER_LEN: usize = 48 * 1024;

struct WindowsWatcher {
    rx: Receiver<FileEvent>,
    dir_handle: HANDLE,
    join_handle: Option<JoinHandle<()>>,
    should_stop: Arc<AtomicBool>,
}

pub fn new_watcher(root: String) -> Option<impl Watcher> {
    let path = U16CString::from_str(&root).ok()?;

    let dir_handle = unsafe {
        FileSystem::CreateFileW(
            path.as_ptr(),
            FILE_LIST_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED,
            0,
        )
    };
    if dir_handle == INVALID_HANDLE_VALUE {
        return None;
    }

    let should_stop = Arc::new(AtomicBool::new(false));

    if let Some((rx, join_handle)) =
        watch_changes(root.into(), dir_handle, Arc::clone(&should_stop))
    {
        Some(WindowsWatcher {
            rx,
            dir_handle,
            should_stop,
            join_handle: Some(join_handle),
        })
    } else {
        // SAFETY: handle is valid
        unsafe { CloseHandle(dir_handle) };
        None
    }
}

impl Watcher for WindowsWatcher {
    fn add_dir(&mut self, _path: String) -> Result<(), WatcherError> {
        //todo should check if path is actually a subpath of watched dir
        Ok(())
    }

    fn read_events(&mut self) -> Vec<FileEvent> {
        self.rx.try_iter().collect()
    }
}

impl Drop for WindowsWatcher {
    fn drop(&mut self) {
        if self.dir_handle != INVALID_HANDLE_VALUE {
            self.should_stop.store(true, Ordering::SeqCst);
            // SAFETY: handle is valid
            unsafe { CloseHandle(self.dir_handle) };
            if let Some(handle) = self.join_handle.take() {
                let _ = handle.join();
            }
        }
    }
}

fn watch_changes(
    root: PathBuf,
    dir_handle: HANDLE,
    should_stop: Arc<AtomicBool>,
) -> Option<(Receiver<FileEvent>, JoinHandle<()>)> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut buffer = vec![0u32; BUFFER_LEN];
    let join_handle = std::thread::spawn(move || {
        while !should_stop.load(Ordering::SeqCst) {
            let mut bytes_returned = 0u32;

            let status = unsafe {
                ReadDirectoryChangesW(
                    dir_handle,
                    buffer.as_mut_ptr() as *mut c_void,
                    buffer.len() as u32,
                    1,
                    FILE_NOTIFY_CHANGE_DIR_NAME
                        | FILE_NOTIFY_CHANGE_FILE_NAME
                        | FILE_NOTIFY_CHANGE_SIZE,
                    &mut bytes_returned,
                    std::ptr::null_mut(),
                    None,
                )
            };
            if status == 0 || bytes_returned < 16 {
                continue;
            }

            let mut current_dword: usize = 0;

            // buffer is filled with objects of structure FILE_NOTIFY_INFORMATION
            // first DWORD holds offset to the next record (in bytes!)
            // second DWORD holds Action
            // third DWORD holds filename length
            // then wchar buffer of filename length (without null terminating char)
            loop {
                let info = unsafe {
                    &*(buffer[current_dword..].as_ptr() as *const FILE_NOTIFY_INFORMATION)
                };

                let filename = unsafe {
                    // FileNameLength is in bytes, so divide by 2 to get number of u16 elements
                    U16String::from_ptr(info.FileName.as_ptr(), (info.FileNameLength / 2) as usize)
                };

                let full_path = root.join(filename.to_os_string());
                if let Some(parent) = full_path
                    .parent()
                    .and_then(|p| U16CString::from_os_str(p).ok())
                    .and_then(|p| crate::platform::get_long_path(&p))
                    .and_then(|p| p.to_string().ok())
                {
                    tx.send(FileEvent {
                        updated_path: parent,
                    })
                    .unwrap();
                }

                //offset to next this should be dword aligned
                let next = info.NextEntryOffset / 4;
                if next == 0 {
                    break;
                }
                current_dword += next as usize;
            }
        }
    });

    Some((rx, join_handle))
}
