use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;

use byte_unit::Byte;

use crate::entry::FileEntry;
use crate::tree::FileTree;
use crate::{utils, EntryPath, EntrySnapshot, SnapshotConfig, TreeSnapshot};

#[derive(Clone, Debug)]
pub struct ScanStats {
    pub used_size: Byte,
    pub files: u64,
    pub dirs: u64,
}

#[derive(Debug)]
pub struct Scanner {
    root: EntryPath,

    //todo use channels for communication with scanner thread
    tree: Arc<Mutex<FileTree>>,

    is_scanning: Arc<AtomicBool>,

    scan_flag: Arc<AtomicBool>,

    scan_handle: Option<JoinHandle<()>>,
}

impl Scanner {
    pub fn get_scan_path(&self) -> &EntryPath {
        &self.root
    }

    pub fn get_tree(
        &self,
        root: &EntryPath,
        config: SnapshotConfig,
    ) -> Option<TreeSnapshot<EntrySnapshot>> {
        self.tree.lock().unwrap().make_snapshot(root, config)
    }

    pub fn get_tree_wrapped<W: AsRef<EntrySnapshot> + AsMut<EntrySnapshot>>(
        &self,
        root: &EntryPath,
        config: SnapshotConfig,
        wrapper: Box<dyn Fn(EntrySnapshot) -> W>,
    ) -> Option<TreeSnapshot<W>> {
        self.tree
            .lock()
            .unwrap()
            .make_snapshot_wrapped(root, config, wrapper)
    }

    pub fn is_scanning(&self) -> bool {
        self.is_scanning.load(Ordering::SeqCst)
    }

    pub fn new(path: String) -> Self {
        let tree = FileTree::new(path);
        let root = tree.get_root().get_path(tree.get_arena());
        let tree = Arc::new(Mutex::new(tree));
        let is_scanning = Arc::new(AtomicBool::new(true));
        let scan_flag = Arc::new(AtomicBool::new(true));

        let scan_handle = Scanner::start_scan(
            Arc::clone(&tree),
            Arc::clone(&is_scanning),
            Arc::clone(&scan_flag),
        );

        Scanner {
            root,
            tree,
            is_scanning,
            scan_flag,
            scan_handle: Some(scan_handle),
        }
    }

    pub fn stats(&self) -> ScanStats {
        let stats = self.tree.lock().unwrap().stats();
        ScanStats {
            used_size: stats.used_size,
            files: stats.files,
            dirs: stats.dirs,
        }
    }

    fn start_scan(
        tree: Arc<Mutex<FileTree>>,
        is_scanning: Arc<AtomicBool>,
        scan_flag: Arc<AtomicBool>,
    ) -> JoinHandle<()> {
        thread::spawn(move || {
            // todo logging
            // println!("background scanner started");

            let root = {
                let tree = tree.lock().unwrap();
                tree.get_root().get_path(tree.get_arena())
            };

            let mut queue: Vec<_> = vec![root];
            let mut children = vec![];

            let excluded = utils::get_excluded_paths();

            while scan_flag.load(Ordering::SeqCst) {
                if let Some(s) = queue.pop() {
                    let entries: Vec<_> = std::fs::read_dir(&s.get_path())
                        .and_then(|dir| dir.collect::<Result<_, _>>())
                        .unwrap_or_default();

                    for entry in entries {
                        let name = entry.file_name().to_str().unwrap().to_string();
                        //todo retrieving metadata can fail
                        let metadata = entry.metadata().unwrap();
                        if metadata.is_dir()
                            && !metadata.is_symlink()
                            && !excluded.contains(&entry.path())
                        {
                            let mut path = s.clone();
                            path.join(name.clone());
                            queue.push(path)
                        }

                        children.push(FileEntry::new(
                            name,
                            metadata.len() as i64,
                            metadata.is_dir(),
                        ));
                    }
                    if !children.is_empty() {
                        let mut tree = tree.lock().unwrap();
                        while let Some(child) = children.pop() {
                            tree.add_child(&s, child);
                        }
                    }
                } else {
                    break;
                }
            }
            // println!("background scanner finished");
            is_scanning.store(false, Ordering::SeqCst);
        })
    }
}

impl Drop for Scanner {
    fn drop(&mut self) {
        self.scan_flag.store(false, Ordering::SeqCst);
        let _ = self.scan_handle.take().unwrap().join();
    }
}
