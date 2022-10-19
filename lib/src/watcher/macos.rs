use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use fsevent::{Event, FsEvent, StreamFlags};

use crate::watcher::{FileEvent, Watcher, WatcherError};

struct FsEventWatcher {
    fsevent: FsEvent,
    rx: Receiver<Event>,
}

pub fn new_watcher(root: String) -> Option<impl Watcher> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut fsevent = FsEvent::new(vec![root]);
    fsevent.observe_async(tx).ok()?;

    Some(FsEventWatcher { fsevent, rx })
}

impl Watcher for FsEventWatcher {
    fn add_dir(&mut self, _path: String) -> Result<(), WatcherError> {
        //todo should check if path is actually a subpath of watched dir
        Ok(())
    }

    fn read_events(&mut self) -> Vec<FileEvent> {
        let flags = StreamFlags::from_iter(
            vec![
                StreamFlags::ITEM_CREATED,
                StreamFlags::ITEM_CLONED,
                StreamFlags::ITEM_REMOVED,
                StreamFlags::ITEM_RENAMED,
                StreamFlags::ITEM_MODIFIED,
            ]
            .into_iter(),
        );

        let mut result = vec![];

        for event in self.rx.try_iter() {
            if !event.flag.intersects(flags) {
                continue;
            }
            if let Some(parent) = PathBuf::from(event.path).parent() {
                result.push(FileEvent {
                    updated_path: parent.to_str().unwrap().to_string(),
                })
            }
        }

        result
    }
}

impl Drop for FsEventWatcher {
    fn drop(&mut self) {
        self.fsevent.shutdown_observe();
    }
}
