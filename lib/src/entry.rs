use std::cmp::Ordering;
use std::fmt::Debug;

use ptree::TreeBuilder;

use crate::arena::{Arena, Id};
use crate::path::{EntryPath, PathCrc};

/// Represents a directory in a directory tree
///
/// Children of [`DirEntry`] are always sorted by size in descending order
/// and can be accessed by [`DirEntry::iter()`]. Children with same size are sorted by name
/// in ascending order
#[derive(Debug)]
pub struct DirEntry {
    /// Name of this directory
    name: String,

    /// Total size of directory (size of all child directories and files)
    size: i64,

    /// Crc of path to this directory
    path_crc: PathCrc,

    /// Parent of directory or `None` for root directory
    parent: Option<Id>,

    /// All child directories
    directories: Vec<Id>,

    /// Number of child files inside this directory
    files: u32,

    /// Whether directory currently marked or not for bulk operations
    is_marked: bool,
}

impl DirEntry {
    /// Adds entry with id `child_id` to children of entry with id `entry_id`
    ///
    /// It is a logic error to add entry with name that is already present in
    /// children.
    pub fn add_child(arena: &mut Arena<DirEntry>, entry_id: Id, child_id: Id) {
        let entry = arena.get(entry_id);
        let path_crc = entry.path_crc;
        let child = arena.get_mut(child_id);
        assert!(child.parent.is_none(), "Entry already has a parent");

        child.parent = Some(entry_id);
        // child didn't have parent before so its path_crc is just name crc
        child.path_crc ^= path_crc;
        let child = arena.get(child_id);
        let child_size = child.size;
        let child_name = &child.name;
        let children = &arena.get(entry_id).directories;

        let idx = Self::find_child(children, arena, child_name, child_size)
            .expect_err("Entry with same size and name is already added to children");
        arena.get_mut(entry_id).directories.insert(idx, child_id);

        if child_size > 0 {
            let entry = arena.get(entry_id);
            if let Some(parent) = entry.parent {
                // size of self will be changed inside this call
                // after it will be reordered in children vec
                Self::on_child_size_changed(arena, parent, entry_id, entry.size + child_size);
            } else {
                arena.get_mut(entry_id).size += child_size;
            }
        }
    }

    /// Compares path of this entry and given `path`
    ///
    /// Same as calling `get_path` and then comparing, but faster
    pub fn compare_path(&self, arena: &Arena<DirEntry>, path: &EntryPath) -> bool {
        let mut parts = path.parts();
        let mut current = self;
        // not comparing CRCs since this function is called after CRCs were compared

        loop {
            if parts.last() != Some(&current.name) {
                return false;
            }
            if let Some(parent) = current.parent {
                current = arena.get(parent);
                parts = &parts[..parts.len() - 1];
            } else {
                return true;
            }
        }
    }

    /// Searches for child position with specified name and size
    ///
    /// Returns Ok(index) if child was found, or Err(index) if child not found
    /// and index is where it should be inserted
    fn find_child(
        children: &[Id],
        arena: &Arena<DirEntry>,
        name: &str,
        size: i64,
    ) -> Result<usize, usize> {
        // find where children with same size begin
        let idx = children.partition_point(|&id| arena.get(id).size > size);

        if idx == children.len() || arena.get(children[idx]).size != size {
            // we found place where child with specified size should be
            Err(idx)
        } else {
            // we found another child with the same size so search for the same name

            // last will be the first entry with size < child_size
            let last = idx + children[idx..].partition_point(|&id| arena.get(id).size == size);
            let idx = idx
                + children[idx..last]
                    .partition_point(|&id| arena.get(id).name.as_str().cmp(name) == Ordering::Less);

            if idx < last && arena.get(children[idx]).name == name {
                Ok(idx)
            } else {
                Err(idx)
            }
        }
    }

    /// Get number of files inside this directory
    pub fn get_files(&self) -> u32 {
        self.files
    }

    /// Name of the entry
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Parent id of the entry
    pub fn get_parent(&self) -> Option<Id> {
        self.parent
    }

    /// Returns full path to this entry
    pub fn get_path(&self, arena: &Arena<DirEntry>) -> EntryPath {
        if let Some(parent) = self.parent {
            let mut path = arena.get(parent).get_path(arena);
            path.join(self.name.clone());
            path
        } else {
            EntryPath::new(self.name.clone())
        }
    }

    /// Size of the entry
    ///
    /// Always >= 0. Signed to keep calculations easier
    pub fn get_size(&self) -> i64 {
        self.size
    }

    /// Returns an iterator over child entries
    ///
    /// Entries are returned in size descending order. If entries have equal size
    /// then they're ordered by name (in ascending order)
    pub fn iter<'a>(&'a self, arena: &'a Arena<DirEntry>) -> impl Iterator<Item = &'a DirEntry> {
        self.directories.iter().map(|&id| arena.get(id))
    }

    /// Marks all children of entry
    ///
    /// Returns number of child directories and their total size
    pub fn mark_children(arena: &mut Arena<DirEntry>, entry_id: Id) -> (u32, i64) {
        let len = arena.get(entry_id).directories.len();
        let mut dirs_size = 0;
        for i in 0..len {
            let id = arena.get(entry_id).directories[i];
            let entry = arena.get_mut(id);
            entry.is_marked = true;
            dirs_size += entry.size;
        }
        (len as u32, dirs_size)
    }

    /// Create new directory entry with given name
    pub fn new_dir(name: String) -> Self {
        // this entry is not attached yet, so path crc is just name crc
        let path_crc = EntryPath::calc_crc(&[&name]).unwrap();

        DirEntry {
            name,
            size: 0,
            path_crc,
            parent: None,
            directories: vec![],
            files: 0,
            is_marked: false,
        }
    }

    /// Called to indicate that size of some child changed and it should
    /// be reordered
    ///
    /// When called, size of child is not yet changed and will be changed here
    ///
    /// # Panics:
    ///
    /// Panics when child is not present in entry's children
    fn on_child_size_changed(
        arena: &mut Arena<DirEntry>,
        entry_id: Id,
        child_id: Id,
        new_size: i64,
    ) {
        let child = arena.get(child_id);
        let prev_size = child.size;
        if prev_size == new_size {
            return;
        }
        let children = &arena.get(entry_id).directories;
        let idx = if children.len() == 1 {
            // entry has single child, so no swaps are necessary
            0
        } else {
            let prev = Self::find_child(children, arena, &child.name, prev_size).unwrap();
            let mut new = Self::find_child(children, arena, &child.name, new_size).unwrap_err();

            let children = &mut arena.get_mut(entry_id).directories;
            match prev.cmp(&new) {
                Ordering::Less => {
                    children[prev..new].rotate_left(1);
                    // new position is one less because entry was removed from its previous position
                    new -= 1;
                }
                Ordering::Greater => children[new..=prev].rotate_right(1),
                Ordering::Equal => {}
            }
            new
        };
        let children = &arena.get(entry_id).directories;
        arena.get_mut(children[idx]).size = new_size;

        let entry = arena.get(entry_id);
        let new_size = entry.size + new_size - prev_size;
        if let Some(parent) = entry.parent {
            Self::on_child_size_changed(arena, parent, entry_id, new_size);
        } else {
            arena.get_mut(entry_id).size = new_size;
        }
    }

    /// Returns crc of full path to this entry (XOR of parent crc and this crc)
    pub fn path_crc(&self) -> PathCrc {
        self.path_crc
    }

    /// Print this entry to stdout as tree with specified depth
    pub fn print(&self, arena: &Arena<DirEntry>, depth: usize) {
        // helper function to recursively populate entry tree
        fn _print<'a>(
            arena: &'a Arena<DirEntry>,
            entry: &'a DirEntry,
            builder: &mut TreeBuilder,
            depth: usize,
        ) {
            builder.begin_child(format!("d {} {}", entry.size, entry.name));

            if depth == 0 && !entry.directories.is_empty() {
                builder.add_empty_child("...".to_string());
            } else {
                for child in entry.iter(arena) {
                    _print(arena, child, builder, depth - 1);
                }
            }
            builder.end_child();
        }

        let entry = self;
        // Build a dir tree using a TreeBuilder
        let mut builder = TreeBuilder::new(format!("d {} {}", entry.size, entry.name));
        if depth == 0 {
            builder.add_empty_child("...".to_string());
        } else {
            for child in entry.iter(arena) {
                _print(arena, child, &mut builder, depth - 1);
            }
        }
        let tree = builder.build();

        // write out the tree using default formatting
        let _ = ptree::print_tree(&tree);
    }

    /// Removes all marked children and returns them
    ///
    /// Returned ids are not removed from arena so cleanup is required
    /// Entries that were removed will be kept marked
    #[must_use]
    pub fn remove_marked(arena: &mut Arena<DirEntry>, entry_id: Id, expected: u32) -> Vec<Id> {
        let entry = arena.get_mut(entry_id);
        let mut children = std::mem::take(&mut entry.directories);
        let mut removed = Vec::with_capacity(expected as usize);

        // could use Vec::drain_filter, but it's unstable
        let mut i = 0;
        let mut insert = 0;
        let mut new_size = entry.size;
        while i < children.len() {
            let child_id = children[i];
            let child = arena.get(child_id);
            if child.is_marked {
                removed.push(child_id);
                new_size -= child.size;
            } else {
                children[insert] = child_id;
                insert += 1;
            }
            i += 1;
        }
        children.truncate(insert);
        arena.get_mut(entry_id).directories = children;
        Self::set_size(arena, entry_id, new_size);

        removed
    }

    /// Set number of files inside this directory
    pub fn set_files(&mut self, files: u32) {
        self.files = files
    }

    /// Set new size (size of all directories and files) of given directory
    pub fn set_size(arena: &mut Arena<DirEntry>, entry_id: Id, new_size: i64) {
        let entry = arena.get_mut(entry_id);
        if let Some(parent) = entry.parent {
            // size of self will be changed inside this call
            // after it will be reordered in children vec
            if entry.size != new_size {
                Self::on_child_size_changed(arena, parent, entry_id, new_size);
            }
        } else {
            entry.size = new_size;
        }
    }

    /// Removes children vec from directory
    ///
    /// Upon calling this function, directory should be already removed
    /// from tree (thus it takes ownership of directory).
    /// Children are returned so they can be cleaned up too
    pub fn take_children(self) -> Vec<Id> {
        self.directories
    }

    pub fn unmark(&mut self) {
        self.is_marked = false;
    }
}

#[cfg(test)]
mod tests {
    use crate::arena::{Arena, Id};
    use crate::entry::DirEntry;
    use crate::path::EntryPath;

    fn new_dir<T: Into<String>>(arena: &mut Arena<DirEntry>, name: T) -> Id {
        new_sized_dir(arena, name, 0)
    }

    fn new_sized_dir<T: Into<String>>(arena: &mut Arena<DirEntry>, name: T, size: i64) -> Id {
        let id = arena.put(DirEntry::new_dir(name.into()));
        DirEntry::set_size(arena, id, size);
        id
    }

    #[test]
    fn add_child() {
        let mut arena = Arena::default();

        let root = new_dir(&mut arena, "root");
        let dir1 = new_dir(&mut arena, "dir1");
        DirEntry::add_child(&mut arena, root, dir1);

        let dir2 = new_dir(&mut arena, "dir2");
        let dir21 = new_sized_dir(&mut arena, "dir21", 5);
        let dir22 = new_sized_dir(&mut arena, "dir22", 15);
        let dir23 = new_sized_dir(&mut arena, "dir23", 10);
        DirEntry::add_child(&mut arena, dir2, dir21);
        DirEntry::add_child(&mut arena, dir2, dir22);
        DirEntry::add_child(&mut arena, dir2, dir23);
        DirEntry::add_child(&mut arena, root, dir2);

        let dir4 = new_sized_dir(&mut arena, "dir4", 5);
        DirEntry::add_child(&mut arena, root, dir4);

        let dir3 = new_sized_dir(&mut arena, "dir3", 5);
        DirEntry::add_child(&mut arena, root, dir3);

        let root = arena.get(root);
        root.print(&arena, 5);

        assert_eq!(root.size, 40);
        let mut iter = root.iter(&arena);

        let entry = iter.next().unwrap();
        assert_eq!(entry.name, "dir2");
        assert_eq!(entry.size, 30);

        {
            let children: Vec<_> = entry
                .iter(&arena)
                .map(|e| (e.name.clone(), e.size))
                .collect();

            assert_eq!(
                children,
                vec![
                    ("dir22".to_owned(), 15),
                    ("dir23".to_owned(), 10),
                    ("dir21".to_owned(), 5),
                ]
            );
        }

        let entry = iter.next().unwrap();
        assert_eq!(entry.name, "dir3");
        assert_eq!(entry.size, 5);

        let entry = iter.next().unwrap();
        assert_eq!(entry.name, "dir4");
        assert_eq!(entry.size, 5);

        let entry = iter.next().unwrap();
        assert_eq!(entry.name, "dir1");
        assert_eq!(entry.size, 0);

        assert!(iter.next().is_none());
    }

    #[test]
    fn compare_path() {
        let mut arena = Arena::default();

        let root = new_dir(&mut arena, "root");
        let dir1 = new_dir(&mut arena, "dir1");
        let dir2 = new_dir(&mut arena, "dir2");
        DirEntry::add_child(&mut arena, dir1, dir2);
        DirEntry::add_child(&mut arena, root, dir1);

        let mut path = EntryPath::new("root".to_string());
        assert!(arena.get(root).compare_path(&arena, &path));
        path.join("dir1".to_string());
        assert!(arena.get(dir1).compare_path(&arena, &path));
        path.join("dir2".to_string());
        assert!(arena.get(dir2).compare_path(&arena, &path));
        assert_eq!(arena.get(dir2).get_path(&arena), path);
    }

    #[test]
    fn find_child() {
        let mut arena = Arena::default();
        let dirs = vec![
            ("dir2", 7),
            ("dir1", 6),
            ("dir3", 4),
            ("dir4", 4),
            ("dir5", 2),
        ];
        let children: Vec<_> = dirs
            .iter()
            .map(|&(name, size)| new_sized_dir(&mut arena, name, size))
            .collect();

        for &(search_name, _) in &dirs {
            for search_size in 1..=8 {
                match DirEntry::find_child(&children, &arena, search_name, search_size) {
                    Ok(pos) => assert_eq!(dirs[pos], (search_name, search_size)),
                    Err(pos) if pos < dirs.len() => {
                        let (found_name, found_size) = dirs[pos];
                        if search_size == found_size {
                            assert!(search_name < found_name);
                        } else {
                            assert!(search_size > found_size);
                        }
                    }
                    Err(_) => {
                        let &(last_name, last_size) = dirs.last().unwrap();
                        if search_size == last_size {
                            assert!(search_name > last_name);
                        } else {
                            assert!(search_size < last_size);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn mark_and_remove() {
        let mut arena = Arena::default();

        let root = new_dir(&mut arena, "root");
        let dir1 = new_dir(&mut arena, "dir1");
        let dir11 = new_sized_dir(&mut arena, "dir11", 15);
        let dir12 = new_sized_dir(&mut arena, "dir12", 25);
        let dir13 = new_sized_dir(&mut arena, "dir13", 10);
        let dir14 = new_sized_dir(&mut arena, "dir14", 10);
        let dir3 = new_sized_dir(&mut arena, "dir3", 45);
        DirEntry::add_child(&mut arena, root, dir1);
        DirEntry::add_child(&mut arena, dir1, dir11);
        DirEntry::add_child(&mut arena, dir1, dir12);
        DirEntry::add_child(&mut arena, dir1, dir13);
        DirEntry::add_child(&mut arena, dir1, dir14);
        DirEntry::add_child(&mut arena, root, dir3);
        arena.get(root).print(&arena, 5);

        let (dirs, dirs_size) = DirEntry::mark_children(&mut arena, dir1);
        assert_eq!(dirs, 4);
        assert_eq!(dirs_size, 60);

        arena.get_mut(dir11).unmark();
        arena.get_mut(dir12).unmark();

        let removed = DirEntry::remove_marked(&mut arena, dir1, 0);
        arena.get(root).print(&arena, 5);
        assert_eq!(removed.len(), 2);
        assert!(removed.contains(&dir13));
        assert!(removed.contains(&dir14));

        let new_dir1 = arena.get(dir1);
        assert_eq!(new_dir1.size, 40);
        let left = &new_dir1.directories;
        assert_eq!(left, &vec![dir12, dir11]);

        let root = &arena.get(root).directories;
        assert_eq!(root, &vec![dir3, dir1]);
    }

    #[test]
    fn child_size_changed() {
        let mut arena = Arena::default();

        let dirs = vec![
            ("dir1", 6),
            ("dir2", 5),
            ("dir3", 3),
            ("dir4", 3),
            ("dir5", 2),
        ];

        let root = new_dir(&mut arena, "root");
        for &(name, size) in &dirs {
            let id = new_sized_dir(&mut arena, name, size);
            DirEntry::add_child(&mut arena, root, id);
        }
        arena.get(root).print(&arena, 5);

        for (name, _) in dirs {
            let dir = arena
                .get(root)
                .directories
                .iter()
                .copied()
                .find(|&id| arena.get(id).get_name() == name)
                .unwrap();
            let initial_size = arena.get(dir).size;

            for new_size in 1..=7 {
                for size in [new_size, initial_size] {
                    DirEntry::on_child_size_changed(&mut arena, root, dir, size);

                    let children = arena.get(root).directories.clone();
                    let mut sorted = children.clone();
                    sorted.sort_by(|&a, &b| {
                        let a = arena.get(a);
                        let b = arena.get(b);
                        b.size.cmp(&a.size).then(a.name.cmp(&b.name))
                    });
                    assert_eq!(children, sorted);
                }
            }
        }
    }
}
