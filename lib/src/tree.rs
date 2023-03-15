use std::collections::HashMap;

use byte_unit::Byte;

use crate::arena::{Arena, Id};
use crate::entry::DirEntry;
use crate::path::{EntryPath, PathCrc};
use crate::tree_snapshot::FilesRetrieverFn;
use crate::{EntrySnapshot, SnapshotConfig, TreeSnapshot};

#[derive(Clone, Debug)]
pub struct Stats {
    pub used_size: Byte,
    pub files: u64,
    pub dirs: u64,
}

#[derive(Debug)]
pub struct FileTree {
    /// Root of file tree
    root: Id,

    /// Arena where all entries are actually stored
    arena: Arena<DirEntry>,

    /// Map of all file entries
    ///
    /// Key is CRC of entry path and value is all entries with same crc
    entries: HashMap<PathCrc, Vec<Id>>,

    files: u64,
    dirs: u64,
}

impl FileTree {
    pub fn find_child(&self, parent_id: Id, name: &str, path_crc: PathCrc) -> Option<Id> {
        let ids = self.entries.get(&path_crc)?;

        ids.iter()
            .find(|&&id| {
                let child = self.arena.get(id);
                child.get_parent() == Some(parent_id) && child.get_name() == name
            })
            .copied()
    }

    pub fn find_entry(&self, path: &EntryPath) -> Option<Id> {
        let root = self.arena.get(self.root);

        //todo can store in field and not create each time
        let root_path = root.get_path(&self.arena);

        if &root_path == path {
            Some(self.root)
        } else if &root_path < path {
            let crc = path.get_crc();
            let ids = self.entries.get(&crc)?;

            ids.iter()
                .find(|&&id| self.arena.get(id).compare_path(&self.arena, path))
                .copied()
        } else {
            None
        }
    }

    pub fn get_arena(&self) -> &Arena<DirEntry> {
        &self.arena
    }

    pub fn get_root(&self) -> &DirEntry {
        self.arena.get(self.root)
    }

    pub fn make_snapshot(
        &self,
        root: &EntryPath,
        config: SnapshotConfig,
        files_getter: &FilesRetrieverFn,
    ) -> Option<TreeSnapshot<EntrySnapshot>> {
        self.make_snapshot_wrapped(root, config, &std::convert::identity, files_getter)
    }

    pub fn make_snapshot_wrapped<W>(
        &self,
        root: &EntryPath,
        config: SnapshotConfig,
        wrapper: &dyn Fn(EntrySnapshot) -> W,
        files_getter: &FilesRetrieverFn,
    ) -> Option<TreeSnapshot<W>>
    where
        W: AsRef<EntrySnapshot> + AsMut<EntrySnapshot>,
    {
        let root = self.find_entry(root)?;
        Some(TreeSnapshot::create_wrapped(
            root,
            &self.arena,
            config,
            wrapper,
            files_getter,
        ))
    }

    /// Creates new [`FileTree`] rooted at specified path
    pub fn new(path: String) -> Self {
        let mut arena = Arena::default();

        let root = arena.put(DirEntry::new_dir(path));
        FileTree {
            root,
            arena,
            entries: HashMap::new(),
            files: 0,
            dirs: 0,
        }
    }

    /// Sets children for specified path
    ///
    /// All existing directories at path, if not present in given vec, are removed (recursively)
    /// Updates number of files at given path and their total size
    /// All new directories are returned
    pub fn set_children(
        &mut self,
        path: &EntryPath,
        directories: Vec<DirEntry>,
        file_count: u64,
        files_size: i64,
    ) -> Option<Vec<String>> {
        let parent_id = self.find_entry(path)?;
        //todo probably can increase speed by presorting children
        // and inserting them in bulk
        let mut new_dirs = vec![];

        let (mut deleted_dirs, dirs_size) = DirEntry::mark_children(&mut self.arena, parent_id);
        // updated total file count
        self.files -= self.arena.get(parent_id).get_files() as u64;
        self.files += file_count;
        self.arena.get_mut(parent_id).set_files(file_count as u32);
        DirEntry::set_size(&mut self.arena, parent_id, dirs_size + files_size);

        let has_children = deleted_dirs > 0;
        let parent_crc = self.arena.get(parent_id).path_crc();
        for dir in directories {
            if has_children {
                let existing =
                    self.find_child(parent_id, dir.get_name(), parent_crc ^ dir.path_crc());

                if let Some(existing) = existing {
                    let child = self.arena.get_mut(existing);
                    child.unmark();
                    deleted_dirs -= 1;
                    // size of dir is sum of children sizes, so nothing to do here
                    continue;
                }
            }

            // entry was not found, add it
            let child_id = self.arena.put(dir);
            DirEntry::add_child(&mut self.arena, parent_id, child_id);

            let child = self.arena.get(child_id);
            self.dirs += 1;
            new_dirs.push(child.get_name().to_string());
            // store new entry in path crc map
            self.entries
                .entry(child.path_crc())
                .or_default()
                .push(child_id);
        }

        if has_children {
            let removed = DirEntry::remove_marked(&mut self.arena, parent_id, deleted_dirs);
            self.cleanup_removed(removed);
        }

        Some(new_dirs)
    }

    /// Return size of tree (number of files and dirs)
    pub fn stats(&self) -> Stats {
        Stats {
            files: self.files,
            dirs: self.dirs,
            used_size: Byte::from_bytes(self.arena.get(self.root).get_size() as u64),
        }
    }

    /// Cleans up removed ids recursively
    fn cleanup_removed(&mut self, entries: Vec<Id>) {
        self.dirs -= entries.len() as u64;
        for id in entries {
            // remove entry from index
            let path_crc = self.arena.get(id).path_crc();
            let bin = self.entries.get_mut(&path_crc).unwrap();
            if bin.len() == 1 {
                self.entries.remove(&path_crc);
            } else {
                let pos = bin.iter().position(|&i| i == id).unwrap();
                bin.swap_remove(pos);
            }

            let children = self.arena.remove(id).unwrap().take_children();
            self.cleanup_removed(children);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fmt::Debug;
    use std::path::PathBuf;

    use crate::entry::DirEntry;
    use crate::path::EntryPath;
    use crate::tree::FileTree;
    use crate::tree_snapshot::FilesRetrieverFn;
    use crate::SnapshotConfig;

    fn new_dir<T: Into<String>>(name: T) -> DirEntry {
        DirEntry::new_dir(name.into())
    }

    fn path<R: Into<PathBuf>, P: Into<PathBuf>>(root: R, path: P) -> EntryPath {
        EntryPath::from(root.into(), path.into()).unwrap()
    }

    fn root_path(tree: &FileTree) -> EntryPath {
        tree.get_root().get_path(&tree.arena)
    }

    fn files_getter<
        S: Into<Vec<(&'static str, i64)>> + Debug,
        T: Into<HashMap<&'static str, S>>,
    >(
        files: T,
    ) -> Box<FilesRetrieverFn> {
        let map = files.into();
        let map: HashMap<_, _> = map
            .into_iter()
            .map(|(key, val)| (key, val.into()))
            .collect();
        Box::new(move |path| {
            // replace all backslashes since in tests we use only '/'
            let path = path.to_str().unwrap().replace('\\', "/");
            map.get(path.as_str())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|(name, size)| (name.to_string(), size))
                .collect()
        })
    }

    fn sample_tree() -> FileTree {
        let root = "/data/mnt".to_string();
        let mut tree = FileTree::new(root.clone());
        tree.set_children(&path(&root, "/data/mnt"), vec![new_dir("dir1")], 2, 25);
        tree.set_children(&path(&root, "/data/mnt/dir1"), vec![new_dir("dir2")], 1, 25);
        tree.set_children(&path(&root, "/data/mnt/dir1/dir2"), vec![], 3, 25);
        tree
    }

    fn sample_getter() -> Box<FilesRetrieverFn> {
        files_getter([
            ("/data/mnt", [("file2", 10), ("file1", 15)].as_ref()),
            ("/data/mnt/dir1", [("file3", 25)].as_ref()),
            (
                "/data/mnt/dir1/dir2",
                [("file4", 5), ("file5", 10), ("file6", 10)].as_ref(),
            ),
        ])
    }

    #[test]
    fn tree_building() {
        let root = "/data/mnt".to_string();
        let mut tree = FileTree::new(root.clone());

        tree.set_children(&path(&root, "/data/mnt"), vec![new_dir("dir1")], 2, 25);
        tree.set_children(&path(&root, "/data/mnt/dir1"), vec![new_dir("dir2")], 1, 25);

        tree.arena.get(tree.root).print(&tree.arena, 5);

        assert_eq!(tree.find_entry(&path(&root, "/data/mnt")), Some(tree.root));

        let dir1 = tree.find_entry(&path(&root, "/data/mnt/dir1")).unwrap();
        let dir2 = tree
            .find_entry(&path(&root, "/data/mnt/dir1/dir2"))
            .unwrap();

        assert_eq!(tree.arena.get(dir1).get_name(), "dir1");
        assert_eq!(tree.arena.get(dir2).get_name(), "dir2");

        assert_eq!(tree.find_entry(&path(&root, "/data/mnt/test")), None);
        assert_eq!(tree.find_entry(&path("/", "/data2")), None);
        assert_eq!(tree.find_entry(&path("/", "/dat")), None);

        let stats = tree.stats();
        assert_eq!(stats.used_size.get_bytes(), 50);
        assert_eq!(stats.files, 3);
        assert_eq!(stats.dirs, 2);
    }

    #[test]
    fn set_children_from_empty() {
        let root = "/data/mnt".to_string();
        let mut tree = FileTree::new(root);

        let new_dirs = tree
            .set_children(&root_path(&tree), vec![new_dir("dir1")], 2, 20)
            .unwrap();
        assert_eq!(new_dirs.len(), 1);
        assert_eq!(new_dirs[0], "dir1");

        let snapshot = tree
            .make_snapshot(
                &root_path(&tree),
                SnapshotConfig::default(),
                &files_getter([("/data/mnt", [("file1", 5), ("file2", 15)])]),
            )
            .unwrap();
        let mut it = snapshot.get_root().iter();
        assert_eq!(it.next().unwrap().get_name(), "file2");
        assert_eq!(it.next().unwrap().get_name(), "file1");
        assert_eq!(it.next().unwrap().get_name(), "dir1");
        assert!(it.next().is_none());
    }

    #[test]
    fn set_children_to_empty() {
        let mut tree = sample_tree();
        tree.get_root().print(tree.get_arena(), 5);

        tree.set_children(&root_path(&tree), vec![], 0, 0);
        tree.get_root().print(tree.get_arena(), 5);
        let snapshot = tree
            .make_snapshot(&root_path(&tree), SnapshotConfig::default(), &|_| vec![])
            .unwrap();
        let root = snapshot.get_root();
        assert!(root.iter().next().is_none());
    }

    #[test]
    fn set_children_update() {
        let mut tree = sample_tree();
        tree.get_root().print(tree.get_arena(), 5);

        let new_dirs = tree
            .set_children(
                &path("/data/mnt", "/data/mnt/dir1"),
                vec![new_dir("dir2"), new_dir("dir3"), new_dir("dir4")],
                1,
                30,
            )
            .unwrap();
        tree.get_root().print(tree.get_arena(), 5);
        assert_eq!(new_dirs.len(), 2);
        assert!(new_dirs.contains(&"dir3".to_string()));
        assert!(new_dirs.contains(&"dir4".to_string()));
        assert_eq!(tree.stats().dirs, 4);
        assert_eq!(tree.stats().files, 6);
        assert_eq!(tree.stats().used_size.get_bytes(), 80);

        let snapshot = tree
            .make_snapshot(
                &path("/data/mnt", "/data/mnt/dir1"),
                SnapshotConfig::default(),
                &files_getter([("/data/mnt/dir1", [("file1", 30)])]),
            )
            .unwrap();
        let children: Vec<_> = snapshot
            .get_root()
            .iter()
            .map(|e| (e.get_name().to_string(), e.get_size().get_bytes()))
            .collect();
        assert_eq!(
            children,
            vec![
                ("file1".to_string(), 30),
                ("dir2".to_string(), 25),
                ("dir3".to_string(), 0),
                ("dir4".to_string(), 0),
            ]
        );

        tree.set_children(&path("/data/mnt", "/data/mnt/dir1/dir2"), vec![], 2, 50);
        assert_eq!(tree.stats().dirs, 4);
        assert_eq!(tree.stats().files, 5);
        assert_eq!(tree.stats().used_size.get_bytes(), 105);
        tree.get_root().print(tree.get_arena(), 5);

        let children: Vec<_> = tree
            .make_snapshot(
                &path("/data/mnt", "/data/mnt/dir1/dir2"),
                SnapshotConfig::default(),
                &files_getter([("/data/mnt/dir1/dir2", [("file6", 5), ("file7", 45)])]),
            )
            .unwrap()
            .get_root()
            .iter()
            .map(|e| (e.get_name().to_string(), e.get_size().get_bytes()))
            .collect();
        assert_eq!(
            children,
            vec![("file7".to_string(), 45), ("file6".to_string(), 5)]
        );
    }

    #[test]
    fn snapshot_from_root() {
        let tree = sample_tree();
        tree.get_root().print(tree.get_arena(), 5);

        let snapshot = tree
            .make_snapshot(
                &root_path(&tree),
                SnapshotConfig::default(),
                &sample_getter(),
            )
            .unwrap();

        let root = snapshot.get_root();
        assert_eq!(root.get_name(), "/data/mnt");
        assert_eq!(root.get_size(), 75u64.into());
        assert!(root.is_dir());
        assert_eq!(root.get_parent_id(), None);

        let dir1 = root.iter().next();
        assert!(dir1.is_some());
        let dir1 = dir1.unwrap();
        assert_eq!(dir1.get_name(), "dir1");
        assert_eq!(dir1.get_size(), 50u64.into());
        assert!(dir1.is_dir());
        assert_eq!(dir1.get_parent_id(), Some(root.get_id()));

        let dir2 = dir1.iter().next();
        assert!(dir2.is_some());
        let dir2 = dir2.unwrap();
        assert_eq!(dir2.get_name(), "dir2");
        assert_eq!(dir2.get_size(), 25u64.into());
        assert!(dir2.is_dir());
        assert_eq!(dir2.get_parent_id(), Some(dir1.get_id()));

        let file5 = dir2.iter().next();
        assert!(file5.is_some());
        let file5 = file5.unwrap();
        assert_eq!(file5.get_name(), "file5");
        assert_eq!(file5.get_size(), 10u64.into());
        assert!(!file5.is_dir());
        assert_eq!(file5.get_parent_id(), Some(dir2.get_id()));
    }

    #[test]
    fn snapshot_from_child() {
        let tree = sample_tree();
        tree.get_root().print(tree.get_arena(), 5);

        let snapshot = tree
            .make_snapshot(
                &path("/data/mnt", "/data/mnt/dir1"),
                SnapshotConfig::default(),
                &sample_getter(),
            )
            .unwrap();

        let root = snapshot.get_root();
        assert_eq!(root.get_name(), "dir1");
        assert_eq!(root.get_size(), 50u64.into());
        assert!(root.is_dir());
        assert_eq!(root.get_parent_id(), None);

        let dir2 = root.iter().next();
        assert!(dir2.is_some());
        let dir2 = dir2.unwrap();
        assert_eq!(dir2.get_name(), "dir2");
        assert_eq!(dir2.get_size(), 25u64.into());
        assert!(dir2.is_dir());
        assert_eq!(dir2.get_parent_id(), Some(root.get_id()));

        let mut dir2_iter = dir2.iter();
        let file5 = dir2_iter.next();
        assert!(file5.is_some());
        let file5 = file5.unwrap();
        assert_eq!(file5.get_name(), "file5");
        assert_eq!(file5.get_size(), 10u64.into());
        assert!(!file5.is_dir());
        assert_eq!(file5.get_parent_id(), Some(dir2.get_id()));

        let file6 = dir2_iter.next();
        assert!(file6.is_some());
        let file6 = file6.unwrap();
        assert_eq!(file6.get_name(), "file6");
        assert_eq!(file6.get_size(), 10u64.into());
        assert!(!file6.is_dir());
        assert_eq!(file6.get_parent_id(), Some(dir2.get_id()));
    }

    #[test]
    fn snapshot_with_depth_constraint() {
        let tree = sample_tree();
        tree.get_root().print(tree.get_arena(), 5);

        let snapshot = tree
            .make_snapshot(
                &root_path(&tree),
                SnapshotConfig {
                    max_depth: 1,
                    ..SnapshotConfig::default()
                },
                &sample_getter(),
            )
            .unwrap();

        let root = snapshot.get_root();
        assert_eq!(root.get_name(), "/data/mnt");
        assert_eq!(root.get_size(), 75u64.into());
        assert!(root.is_dir());
        assert_eq!(root.get_parent_id(), None);

        let dir1 = root.iter().next();
        assert!(dir1.is_some());
        let dir1 = dir1.unwrap();
        assert_eq!(dir1.get_name(), "dir1");
        assert_eq!(dir1.get_size(), 50u64.into());
        assert!(dir1.is_dir());
        assert_eq!(dir1.get_parent_id(), Some(root.get_id()));

        assert!(dir1.iter().next().is_none());
    }

    #[test]
    fn snapshot_with_size_constraint() {
        let tree = sample_tree();
        tree.get_root().print(tree.get_arena(), 5);

        let snapshot = tree
            .make_snapshot(
                &root_path(&tree),
                SnapshotConfig {
                    min_size: 12,
                    ..SnapshotConfig::default()
                },
                &sample_getter(),
            )
            .unwrap();

        let root = snapshot.get_root();
        assert_eq!(root.get_name(), "/data/mnt");
        assert_eq!(root.get_size(), 75u64.into());
        assert!(root.is_dir());
        assert_eq!(root.get_parent_id(), None);

        let mut root_iter = root.iter();
        let dir1 = root_iter.next();
        assert!(dir1.is_some());
        let dir1 = dir1.unwrap();
        assert_eq!(dir1.get_name(), "dir1");
        assert_eq!(dir1.get_size(), 50u64.into());
        assert!(dir1.is_dir());
        assert_eq!(dir1.get_parent_id(), Some(root.get_id()));

        let dir2 = dir1.iter().next();
        assert!(dir2.is_some());
        let dir2 = dir2.unwrap();
        assert_eq!(dir2.get_name(), "dir2");
        assert_eq!(dir2.get_size(), 25u64.into());
        assert!(dir2.is_dir());
        assert_eq!(dir2.get_parent_id(), Some(dir1.get_id()));

        assert!(dir2.iter().next().is_none());

        let file1 = root_iter.next();
        assert!(file1.is_some());
        let file1 = file1.unwrap();
        assert_eq!(file1.get_name(), "file1");
        assert_eq!(file1.get_size(), 15u64.into());
        assert!(!file1.is_dir());
        assert_eq!(file1.get_parent_id(), Some(root.get_id()));

        assert!(root_iter.next().is_none());
    }
}
