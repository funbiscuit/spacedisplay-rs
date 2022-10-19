use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use byte_unit::Byte;

use crate::entry::FileEntry;
use crate::tree::FileTree;
use crate::watcher::Watcher;
use crate::{platform, EntryPath, EntrySnapshot, SnapshotConfig, TreeSnapshot};

#[derive(Clone, Debug)]
pub struct ScanStats {
    pub used_size: Byte,
    pub total_size: Option<Byte>,
    pub available_size: Option<Byte>,
    pub is_mount_point: bool,
    pub files: u64,
    pub dirs: u64,
    pub scan_duration: Duration,
}

#[derive(Debug)]
struct ScanState {
    tree: Mutex<FileTree>,

    current_path: Mutex<Option<EntryPath>>,

    is_scanning: AtomicBool,

    scan_flag: AtomicBool,

    scan_duration_ms: AtomicU32,
}

#[derive(Debug)]
struct ScanTask {
    path: EntryPath,
    recursive: bool,
}

#[derive(Debug)]
pub struct Scanner {
    root: EntryPath,

    state: Arc<ScanState>,

    tx: Sender<ScanTask>,

    scan_handle: Option<JoinHandle<()>>,
}

impl Scanner {
    pub fn get_scan_path(&self) -> &EntryPath {
        &self.root
    }

    pub fn get_current_scan_path(&self) -> Option<EntryPath> {
        self.state.current_path.lock().unwrap().clone()
    }

    pub fn get_tree(
        &self,
        root: &EntryPath,
        config: SnapshotConfig,
    ) -> Option<TreeSnapshot<EntrySnapshot>> {
        self.state.tree.lock().unwrap().make_snapshot(root, config)
    }

    pub fn get_tree_wrapped<W: AsRef<EntrySnapshot> + AsMut<EntrySnapshot>>(
        &self,
        root: &EntryPath,
        config: SnapshotConfig,
        wrapper: Box<dyn Fn(EntrySnapshot) -> W>,
    ) -> Option<TreeSnapshot<W>> {
        self.state
            .tree
            .lock()
            .unwrap()
            .make_snapshot_wrapped(root, config, wrapper)
    }

    pub fn is_scanning(&self) -> bool {
        self.state.is_scanning.load(Ordering::SeqCst)
    }

    pub fn new(path: String) -> Self {
        let tree = FileTree::new(path.clone());
        let root = tree.get_root().get_path(tree.get_arena());
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(ScanTask {
            path: root.clone(),
            recursive: true,
        })
        .unwrap();
        let state = Arc::new(ScanState {
            tree: Mutex::new(tree),
            current_path: Mutex::new(None),
            is_scanning: AtomicBool::new(false),
            scan_flag: AtomicBool::new(true),
            scan_duration_ms: AtomicU32::new(0),
        });

        let scan_handle = Scanner::start_scan(path, Arc::clone(&state), tx.clone(), rx);

        Scanner {
            root,
            state,
            tx,
            scan_handle: Some(scan_handle),
        }
    }

    pub fn rescan_path(&self, path: EntryPath) {
        self.tx
            .send(ScanTask {
                path,
                recursive: true,
            })
            .unwrap();
    }

    pub fn stats(&self) -> ScanStats {
        let scan_stats = self.state.tree.lock().unwrap().stats();
        let scan_duration =
            Duration::from_millis(self.state.scan_duration_ms.load(Ordering::SeqCst) as u64);
        if let Some(mount_stats) = platform::get_mount_stats(self.root.get_path()) {
            ScanStats {
                used_size: scan_stats.used_size,
                total_size: Some(mount_stats.total),
                available_size: Some(mount_stats.available),
                is_mount_point: mount_stats.is_mount_point,
                files: scan_stats.files,
                dirs: scan_stats.dirs,
                scan_duration,
            }
        } else {
            ScanStats {
                used_size: scan_stats.used_size,
                total_size: None,
                available_size: None,
                is_mount_point: false,
                files: scan_stats.files,
                dirs: scan_stats.dirs,
                scan_duration,
            }
        }
    }

    fn start_scan(
        root: String,
        state: Arc<ScanState>,
        tx: Sender<ScanTask>,
        rx: Receiver<ScanTask>,
    ) -> JoinHandle<()> {
        thread::spawn(move || {
            // todo logging
            // println!("background scanner started");

            let mut watcher = crate::watcher::new_watcher(root.clone());

            let mut start = Instant::now();

            let mut queue: Vec<_> = vec![];
            let mut children = vec![];

            let excluded = platform::get_excluded_paths();

            while state.scan_flag.load(Ordering::SeqCst) {
                while state.scan_flag.load(Ordering::SeqCst) {
                    // check for events
                    //todo remove duplicates
                    if let Some(w) = &mut watcher {
                        queue.extend(
                            w.read_events()
                                .into_iter()
                                .filter_map(|e| EntryPath::from(&root, &e.updated_path))
                                .map(|path| ScanTask {
                                    recursive: false,
                                    path,
                                }),
                        );
                    }
                    // add all tasks to queue
                    for task in rx.try_iter() {
                        queue.push(task);

                        if !state.is_scanning.load(Ordering::SeqCst) {
                            start = Instant::now();
                            state.is_scanning.store(true, Ordering::SeqCst);
                        }
                    }
                    if !queue.is_empty() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(10));
                }

                let mut rx_empty = true;
                if let Some(task) = queue.pop() {
                    watcher.as_mut().map(|w| w.add_dir(task.path.to_string()));
                    state
                        .current_path
                        .lock()
                        .unwrap()
                        .replace(task.path.clone());
                    let entries: Vec<_> = std::fs::read_dir(&task.path.get_path())
                        .and_then(|dir| dir.collect::<Result<_, _>>())
                        .unwrap_or_default();

                    for entry in entries {
                        if let Ok(metadata) = entry.metadata() {
                            let name = entry.file_name().to_str().unwrap().to_string();
                            if metadata.is_dir()
                                && !metadata.is_symlink()
                                && !excluded.contains(&entry.path())
                            {
                                let mut path = task.path.clone();
                                path.join(name.clone());
                                if task.recursive {
                                    tx.send(ScanTask {
                                        path,
                                        recursive: true,
                                    })
                                    .unwrap();
                                    rx_empty = false;
                                }
                            }

                            let size = platform::get_file_size(&metadata) as i64;
                            children.push(FileEntry::new(name, size, metadata.is_dir()));
                        }
                    }
                    if !children.is_empty() {
                        let mut tree = state.tree.lock().unwrap();
                        //todo process new paths
                        tree.set_children(&task.path, children);
                        children = vec![];
                    }
                }
                if state.is_scanning.load(Ordering::SeqCst) {
                    state
                        .scan_duration_ms
                        .store(start.elapsed().as_millis() as u32, Ordering::SeqCst);
                }
                if queue.is_empty() && rx_empty {
                    state.is_scanning.store(false, Ordering::SeqCst);
                    state.current_path.lock().unwrap().take();
                }
            }
        })
    }
}

impl Drop for Scanner {
    fn drop(&mut self) {
        self.state.scan_flag.store(false, Ordering::SeqCst);
        let _ = self.scan_handle.take().unwrap().join();
    }
}
