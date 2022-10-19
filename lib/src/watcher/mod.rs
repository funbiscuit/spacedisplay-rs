#[cfg(target_os = "linux")]
pub use linux::new_watcher;
#[cfg(target_os = "macos")]
pub use macos::new_watcher;
#[cfg(target_os = "windows")]
pub use windows::new_watcher;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[allow(dead_code)]
#[derive(Debug)]
pub enum WatcherError {
    /// Used when unable to add inotify watch in linux
    DirLimitReached,
    Unknown,
}

#[derive(Debug)]
pub struct FileEvent {
    pub updated_path: String,
}

pub trait Watcher {
    fn add_dir(&mut self, path: String) -> Result<(), WatcherError>;

    fn read_events(&mut self) -> Vec<FileEvent>;
}
