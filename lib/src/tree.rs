use std::collections::HashMap;

use byte_unit::Byte;

use crate::arena::{Arena, Id};
use crate::entry::FileEntry;
use crate::path::{EntryPath, PathCrc};
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
    arena: Arena<FileEntry>,

    /// Map of all file entries
    ///
    /// Key is CRC of entry path and value is all entries with same crc
    entries: HashMap<PathCrc, Vec<Id>>,

    files: u64,
    dirs: u64,
}

impl FileTree {
    /// Add child to specific path
    ///
    /// Path must be an existing directory in this tree
    pub fn add_child(&mut self, path: &EntryPath, child: FileEntry) -> Option<Id> {
        //todo use Result as return
        let parent_id = self.find_entry(path)?;
        let child_id = self.arena.put(child);

        FileEntry::add_child(&mut self.arena, parent_id, child_id);

        let child = self.arena.get(child_id);
        if child.is_dir() {
            self.dirs += 1;
        } else {
            self.files += 1;
        }
        // store new entry in path crc map
        let bin = self.entries.entry(child.path_crc()).or_insert(vec![]);
        bin.push(child_id);

        Some(child_id)
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

    pub fn get_arena(&self) -> &Arena<FileEntry> {
        &self.arena
    }

    pub fn get_root(&self) -> &FileEntry {
        self.arena.get(self.root)
    }

    pub fn make_snapshot(
        &self,
        root: &EntryPath,
        config: SnapshotConfig,
    ) -> Option<TreeSnapshot<EntrySnapshot>> {
        let root = self.find_entry(root)?;
        Some(TreeSnapshot::create(root, &self.arena, config))
    }

    pub fn make_snapshot_wrapped<W>(
        &self,
        root: &EntryPath,
        config: SnapshotConfig,
        wrapper: Box<dyn Fn(EntrySnapshot) -> W>,
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
        ))
    }

    /// Creates new [`FileTree`] rooted at specified path
    pub fn new(path: String) -> Self {
        let mut arena = Arena::default();

        let root = arena.put(FileEntry::new_dir(path, 0));
        FileTree {
            root,
            arena,
            entries: HashMap::new(),
            files: 0,
            dirs: 0,
        }
    }

    /// Return size of tree (number of files and dirs)
    pub fn stats(&self) -> Stats {
        Stats {
            files: self.files,
            dirs: self.dirs,
            used_size: Byte::from_bytes(self.arena.get(self.root).get_size() as u64),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::entry::FileEntry;
    use crate::path::EntryPath;
    use crate::tree::FileTree;
    use crate::SnapshotConfig;

    fn new_dir<T: Into<String>>(name: T) -> FileEntry {
        FileEntry::new_dir(name.into(), 0)
    }

    fn new_file<T: Into<String>>(name: T, size: i64) -> FileEntry {
        FileEntry::new_file(name.into(), size)
    }

    fn path<R: Into<PathBuf>, P: Into<PathBuf>>(root: R, path: P) -> EntryPath {
        EntryPath::from(&root.into(), &path.into()).unwrap()
    }

    fn root_path(tree: &FileTree) -> EntryPath {
        tree.get_root().get_path(&tree.arena)
    }

    fn sample_tree() -> FileTree {
        let root = "/data/mnt".to_string();
        let mut tree = FileTree::new(root.clone());
        tree.add_child(&path(&root, "/data/mnt"), new_file("file1", 15));
        tree.add_child(&path(&root, "/data/mnt"), new_file("file2", 10));
        tree.add_child(&path(&root, "/data/mnt"), new_dir("dir1"));
        tree.add_child(&path(&root, "/data/mnt/dir1"), new_dir("dir2"));
        tree.add_child(&path(&root, "/data/mnt/dir1"), new_file("file3", 25));
        tree.add_child(&path(&root, "/data/mnt/dir1/dir2"), new_file("file4", 5));
        tree.add_child(&path(&root, "/data/mnt/dir1/dir2"), new_file("file5", 10));
        tree.add_child(&path(&root, "/data/mnt/dir1/dir2"), new_file("file6", 10));
        tree
    }

    #[test]
    fn tree_building() {
        let root = "/data/mnt".to_string();
        let mut tree = FileTree::new(root.clone());

        let file1 = tree.add_child(&path(&root, "/data/mnt"), new_file("file1", 15));
        let file2 = tree.add_child(&path(&root, "/data/mnt"), new_file("file2", 10));
        let dir1 = tree.add_child(&path(&root, "/data/mnt"), new_dir("dir1"));
        let file3 = tree.add_child(&path(&root, "/data/mnt/dir1"), new_file("file3", 25));

        tree.arena.get(tree.root).print(&tree.arena, 5);

        assert!(file1.is_some());
        assert!(file2.is_some());
        assert!(dir1.is_some());
        assert!(file3.is_some());

        assert_eq!(tree.find_entry(&path(&root, "/data/mnt")), Some(tree.root));
        assert_eq!(tree.find_entry(&path(&root, "/data/mnt/file1")), file1);
        assert_eq!(tree.find_entry(&path(&root, "/data/mnt/file2")), file2);
        assert_eq!(tree.find_entry(&path(&root, "/data/mnt/dir1")), dir1);
        assert_eq!(tree.find_entry(&path(&root, "/data/mnt/dir1/file3")), file3);

        assert_eq!(tree.find_entry(&path(&root, "/data/mnt/test")), None);
        assert_eq!(tree.find_entry(&path("/", "/data2")), None);
        assert_eq!(tree.find_entry(&path("/", "/dat")), None);

        let stats = tree.stats();
        assert_eq!(stats.used_size.get_bytes(), 50);
        assert_eq!(stats.files, 3);
        assert_eq!(stats.dirs, 1);
    }

    #[test]
    fn create_from_root() {
        let tree = sample_tree();
        tree.get_root().print(tree.get_arena(), 5);

        let snapshot = tree
            .make_snapshot(&root_path(&tree), SnapshotConfig::default())
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
    fn create_from_child() {
        let tree = sample_tree();
        tree.get_root().print(tree.get_arena(), 5);

        let snapshot = tree
            .make_snapshot(
                &path("/data/mnt", "/data/mnt/dir1"),
                SnapshotConfig::default(),
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
    fn create_with_depth_constraint() {
        let tree = sample_tree();
        tree.get_root().print(tree.get_arena(), 5);

        let snapshot = tree
            .make_snapshot(
                &root_path(&tree),
                SnapshotConfig {
                    max_depth: 1,
                    ..SnapshotConfig::default()
                },
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
    fn create_with_size_constraint() {
        let tree = sample_tree();
        tree.get_root().print(tree.get_arena(), 5);

        let snapshot = tree
            .make_snapshot(
                &root_path(&tree),
                SnapshotConfig {
                    min_size: 15,
                    ..SnapshotConfig::default()
                },
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
