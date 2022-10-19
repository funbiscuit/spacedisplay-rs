use std::collections::HashMap;

use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask};
use nix::libc::ENOSPC;

use crate::watcher::{FileEvent, Watcher, WatcherError};

const BUFFER_LEN: usize = 64 * 1024;

struct InotifyWatcher {
    inotify: Inotify,
    buffer: Vec<u8>,
    map: HashMap<WatchDescriptor, String>,
}

pub fn new_watcher(root: String) -> Option<impl Watcher> {
    let inotify = Inotify::init().ok()?;
    let buffer = vec![0; BUFFER_LEN];

    let mut watcher = InotifyWatcher {
        inotify,
        buffer,
        map: HashMap::new(),
    };
    watcher.add_dir(root).ok()?;
    Some(watcher)
}

impl Watcher for InotifyWatcher {
    fn add_dir(&mut self, path: String) -> Result<(), WatcherError> {
        //not using DELETE_SELF and MOVE_SELF since these events should be detected by parent directory
        let wd = self
            .inotify
            .add_watch(
                &path,
                WatchMask::MODIFY | WatchMask::MOVE | WatchMask::CREATE | WatchMask::DELETE,
            )
            .map_err(|e| {
                if e.raw_os_error() == Some(ENOSPC) {
                    WatcherError::DirLimitReached
                } else {
                    WatcherError::Unknown
                }
            })?;

        self.map.insert(wd, path);

        Ok(())
    }

    fn read_events(&mut self) -> Vec<FileEvent> {
        let mut result = vec![];
        let mm = EventMask::from_iter(
            vec![
                EventMask::CREATE,
                EventMask::DELETE,
                EventMask::MODIFY,
                EventMask::MOVED_FROM,
                EventMask::MOVED_TO,
            ]
            .into_iter(),
        );
        if let Ok(events) = self.inotify.read_events(&mut self.buffer) {
            for event in events {
                if event.mask.contains(EventMask::IGNORED) {
                    // watch was removed so remove it from our map
                    self.map.remove(&event.wd);
                }
                if !event.mask.intersects(mm) {
                    continue;
                }
                result.push(FileEvent {
                    updated_path: self.map[&event.wd].clone(),
                })
            }
        }
        result
    }
}
