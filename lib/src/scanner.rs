use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use byte_unit::Byte;

use crate::entry::DirEntry;
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
    pub used_memory: Option<Byte>,
}

#[derive(Debug)]
struct ScanState {
    tree: Mutex<FileTree>,

    current_path: Mutex<Option<EntryPath>>,

    is_scanning: AtomicBool,

    scan_flag: AtomicBool,

    scan_duration_ms: AtomicU32,
}

#[derive(Debug, Eq, PartialEq)]
struct ScanTask {
    path: EntryPath,
    reset_stopwatch: bool,
    recursive: bool,
}

#[non_exhaustive]
#[derive(Debug, Default)]
pub struct ScannerBuilder;

impl ScannerBuilder {
    pub fn scan(self, path: String) -> Scanner {
        Scanner::new(path)
    }
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
        self.state
            .tree
            .lock()
            .unwrap()
            .make_snapshot(root, config, &Scanner::retrieve_files)
    }

    pub fn get_tree_wrapped<W: AsRef<EntrySnapshot> + AsMut<EntrySnapshot>>(
        &self,
        root: &EntryPath,
        config: SnapshotConfig,
        wrapper: &dyn Fn(EntrySnapshot) -> W,
    ) -> Option<TreeSnapshot<W>> {
        self.state.tree.lock().unwrap().make_snapshot_wrapped(
            root,
            config,
            wrapper,
            &Scanner::retrieve_files,
        )
    }

    pub fn is_scanning(&self) -> bool {
        self.state.is_scanning.load(Ordering::SeqCst)
    }

    pub fn rescan_path(&self, path: EntryPath, reset_stopwatch: bool) {
        info!("Start rescan of '{}'", path);
        self.tx
            .send(ScanTask {
                path,
                reset_stopwatch,
                recursive: true,
            })
            .unwrap();
    }

    pub fn stats(&self) -> ScanStats {
        let scan_stats = self.state.tree.lock().unwrap().stats();
        let scan_duration =
            Duration::from_millis(self.state.scan_duration_ms.load(Ordering::SeqCst) as u64);
        let (total, available, is_mount) = platform::get_mount_stats(self.root.get_path())
            .map(|s| (Some(s.total), Some(s.available), s.is_mount_point))
            .unwrap_or((None, None, false));
        ScanStats {
            used_size: scan_stats.used_size,
            total_size: total,
            available_size: available,
            is_mount_point: is_mount,
            files: scan_stats.files,
            dirs: scan_stats.dirs,
            scan_duration,
            used_memory: platform::get_used_memory(),
        }
    }

    fn merge_to_queue(queue: &mut Vec<ScanTask>, task: ScanTask) {
        // could use Vec::drain_filter, but it's unstable
        let mut i = 0;
        let mut insert = 0;
        while i < queue.len() {
            let existing = &queue[i];

            if &task != existing
                && (!task.recursive
                    || existing
                        .path
                        .partial_cmp(&task.path)
                        .map(|ord| ord == std::cmp::Ordering::Less)
                        .unwrap_or(true))
            {
                // existing task is kept in queue if it is not the same as new task AND
                // new task is not recursive OR it is recursive but existing task is not scanning
                // some subdirectory of new task

                if insert != i {
                    queue.swap(insert, i);
                }
                insert += 1;
            }
            i += 1;
        }

        queue.truncate(insert);
        queue.push(task);
    }

    fn new(path: String) -> Self {
        let tree = FileTree::new(path.clone());
        let root = tree.get_root().get_path(tree.get_arena());
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(ScanTask {
            path: root.clone(),
            reset_stopwatch: true,
            recursive: true,
        })
        .unwrap();
        let state = Arc::new(ScanState {
            tree: Mutex::new(tree),
            current_path: Mutex::new(None),
            is_scanning: AtomicBool::new(true),
            scan_flag: AtomicBool::new(true),
            scan_duration_ms: AtomicU32::new(0),
        });

        let scan_handle = Scanner::start_scan(path, Arc::clone(&state), rx);

        Scanner {
            root,
            state,
            tx,
            scan_handle: Some(scan_handle),
        }
    }

    /// Retrieve list of all files and their sizes at specified path
    /// Files are not sorted in any way
    fn retrieve_files(path: &Path) -> Vec<(String, i64)> {
        std::fs::read_dir(path)
            .and_then(|rd| {
                let mut files = vec![];
                for f in rd {
                    let f = f?;

                    if let Ok(metadata) = f.metadata() {
                        if !metadata.is_dir() || metadata.is_symlink() {
                            let name = f.file_name().to_str().unwrap().to_string();
                            let size = platform::get_file_size(&metadata) as i64;

                            files.push((name, size))
                        }
                    }
                }
                Ok(files)
            })
            .unwrap_or_default()
    }

    fn start_scan(root: String, state: Arc<ScanState>, rx: Receiver<ScanTask>) -> JoinHandle<()> {
        thread::spawn(move || {
            let mut watcher = crate::watcher::new_watcher(root.clone());

            let mut start = Instant::now();

            let mut queue: Vec<ScanTask> = vec![];
            let mut children = vec![];

            let available: HashSet<_> = platform::get_available_mounts().into_iter().collect();
            // excluded paths are all available mounts (excluding root scan path)
            // and all unsupported mounts
            let excluded: HashSet<_> = platform::get_excluded_paths()
                .into_iter()
                .filter_map(|p| p.to_str().map(|s| s.to_string()))
                .chain(available.into_iter())
                .filter(|p| p != &root)
                .collect();

            info!("Start scan of '{}'", root);

            while state.scan_flag.load(Ordering::SeqCst) {
                while state.scan_flag.load(Ordering::SeqCst) {
                    // check for events
                    if let Some(w) = &mut watcher {
                        for task in w
                            .read_events()
                            .into_iter()
                            .filter_map(|e| EntryPath::from(&root, e.updated_path))
                            .map(|path| ScanTask {
                                recursive: false,
                                reset_stopwatch: false,
                                path,
                            })
                        {
                            Scanner::merge_to_queue(&mut queue, task);
                        }
                    }
                    // add all tasks to queue
                    for task in rx.try_iter() {
                        if task.reset_stopwatch && !state.is_scanning.load(Ordering::SeqCst) {
                            start = Instant::now();
                            state.is_scanning.store(true, Ordering::SeqCst);
                        }
                        Scanner::merge_to_queue(&mut queue, task);
                    }
                    if !queue.is_empty() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(10));
                }

                if let Some(task) = queue.pop() {
                    let task_path = task.path.to_string();
                    if excluded.contains(&task_path) {
                        continue;
                    }
                    watcher.as_mut().map(|w| w.add_dir(task_path));
                    state
                        .current_path
                        .lock()
                        .unwrap()
                        .replace(task.path.clone());
                    let entries: Vec<_> = std::fs::read_dir(&task.path.get_path())
                        .and_then(|dir| dir.collect::<Result<_, _>>())
                        .unwrap_or_else(|_| {
                            warn!("Unable to scan '{}'", task.path);
                            vec![]
                        });

                    let mut file_count = 0;
                    let mut files_size = 0;
                    for entry in entries {
                        if let Ok(metadata) = entry.metadata() {
                            let name = entry.file_name().to_str().unwrap().to_string();
                            if task.recursive && metadata.is_dir() && !metadata.is_symlink() {
                                let mut path = task.path.clone();
                                path.join(name.clone());
                                queue.push(ScanTask {
                                    path,
                                    reset_stopwatch: false,
                                    recursive: true,
                                });
                            }

                            if metadata.is_dir() && !metadata.is_symlink() {
                                // treat all directories as zero sized
                                children.push(DirEntry::new_dir(name));
                            } else {
                                file_count += 1;
                                files_size += platform::get_file_size(&metadata) as i64;
                            }
                        } else {
                            warn!("Failed to get metadata for {:?}", entry.path());
                        }
                    }
                    let new_dirs = {
                        let mut tree = state.tree.lock().unwrap();
                        tree.set_children(&task.path, children, file_count, files_size)
                    };

                    if let Some(new_dirs) = new_dirs {
                        if !task.recursive {
                            for dir in new_dirs {
                                let mut path = task.path.clone();
                                path.join(dir);
                                queue.push(ScanTask {
                                    path,
                                    reset_stopwatch: false,
                                    recursive: true,
                                });
                            }
                        }
                    }
                    children = vec![];
                }
                if state.is_scanning.load(Ordering::SeqCst) {
                    let duration = start.elapsed().as_millis() as u32;
                    state.scan_duration_ms.store(duration, Ordering::SeqCst);
                    if queue.is_empty() {
                        let stats = state.tree.lock().unwrap().stats();
                        info!(
                            "Scan finished: {} files {} dirs in {:?}",
                            stats.files,
                            stats.dirs,
                            Duration::from_millis(duration as u64)
                        );
                    }
                }
                if queue.is_empty() {
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
