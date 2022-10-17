use std::cmp::Ordering;
use std::fmt::Debug;

use ptree::TreeBuilder;

use crate::arena::{Arena, Id};
use crate::path::{EntryPath, PathCrc};

/// Represents a file (or dir) in a file tree
///
/// Children of [`FileEntry`] are always sorted by size in descending order
/// and can be accessed by [`FileEntry::iter()`]. Children with same size are sorted by name
/// in ascending order
#[derive(Debug)]
pub struct FileEntry {
    name: String,
    size: i64,
    path_crc: PathCrc,
    parent: Option<Id>,
    children: Option<Vec<Id>>,
    is_marked: bool,
}

impl FileEntry {
    /// Adds entry with id `child_id` to children of entry with id `entry_id`
    ///
    /// It is a logic error to add entry with name that is already present in
    /// children.
    pub fn add_child(arena: &mut Arena<FileEntry>, entry_id: Id, child_id: Id) {
        let entry = arena.get(entry_id);
        assert!(entry.is_dir(), "Can't add child to a file");
        let path_crc = entry.path_crc;
        let child = arena.get_mut(child_id);
        assert!(child.parent.is_none(), "Entry already has a parent");

        child.parent = Some(entry_id);
        // child didn't have parent before so its path_crc is just name crc
        child.path_crc ^= path_crc;
        let child = arena.get(child_id);
        let child_size = child.size;
        let child_name = &child.name;
        // already checked that entry is dir, so unwrap is safe
        let children = arena.get(entry_id).children.as_ref().unwrap();

        let idx = FileEntry::find_child(children, arena, child_name, child_size)
            .expect_err("Entry with same size and name is already added to children");
        // already checked that entry is dir, so unwrap is safe
        arena
            .get_mut(entry_id)
            .children
            .as_mut()
            .unwrap()
            .insert(idx, child_id);

        if child_size > 0 {
            let entry = arena.get(entry_id);
            if let Some(parent) = entry.parent {
                // size of self will be changed inside this call
                // after it will be reordered in children vec
                FileEntry::on_child_size_changed(arena, parent, entry_id, entry.size + child_size);
            } else {
                arena.get_mut(entry_id).size += child_size;
            }
        }
    }

    /// Compares path of this entry and given `path`
    ///
    /// Same as calling `get_path` and then comparing, but faster
    pub fn compare_path(&self, arena: &Arena<FileEntry>, path: &EntryPath) -> bool {
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
        arena: &Arena<FileEntry>,
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

    /// Name of the entry
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Parent id of the entry
    pub fn get_parent(&self) -> Option<Id> {
        self.parent
    }

    /// Returns full path to this entry
    pub fn get_path(&self, arena: &Arena<FileEntry>) -> EntryPath {
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

    /// Whether entry is directory (`true`) or file (`false`)
    pub fn is_dir(&self) -> bool {
        self.children.is_some()
    }

    /// Returns an iterator over child entries
    ///
    /// Entries are returned in size descending order. If entries have equal size
    /// then they're ordered by name (in ascending order)
    pub fn iter<'a>(&'a self, arena: &'a Arena<FileEntry>) -> impl Iterator<Item = &'a FileEntry> {
        self.children
            .as_ref()
            .expect("Can iterate only inside directory")
            .iter()
            .map(|&id| arena.get(id))
    }

    /// Marks all children of entry (expects it to be a directory)
    ///
    /// Returns number of marked directories and files
    pub fn mark_children(arena: &mut Arena<FileEntry>, entry_id: Id) -> (u32, u32) {
        let len = arena
            .get(entry_id)
            .children
            .as_ref()
            .expect("mark directory children")
            .len();
        let mut dirs = 0;
        let mut files = 0;
        for i in 0..len {
            let id = arena.get(entry_id).children.as_ref().unwrap()[i];
            let entry = arena.get_mut(id);
            entry.is_marked = true;
            if entry.is_dir() {
                dirs += 1;
            } else {
                files += 1;
            }
        }
        (dirs, files)
    }

    /// Create new entry with given name and size.
    pub fn new(name: String, size: i64, is_dir: bool) -> Self {
        assert!(size >= 0, "Entry size must be >= 0");

        // this entry is not attached yet, so path crc is just name crc
        let path_crc = EntryPath::calc_crc(&[&name]).unwrap();

        FileEntry {
            name,
            size,
            path_crc,
            parent: None,
            children: if is_dir { Some(vec![]) } else { None },
            is_marked: false,
        }
    }

    /// Create new directory with given name and size.
    pub fn new_dir(name: String, size: i64) -> Self {
        FileEntry::new(name, size, true)
    }

    /// Create new file with given name and size.
    pub fn new_file(name: String, size: i64) -> Self {
        FileEntry::new(name, size, false)
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
        arena: &mut Arena<FileEntry>,
        entry_id: Id,
        child_id: Id,
        new_size: i64,
    ) {
        let child = arena.get(child_id);
        let prev_size = child.size;
        if prev_size == new_size {
            return;
        }
        let children = arena.get(entry_id).children.as_ref().unwrap();
        let idx = if children.len() == 1 {
            // entry has single child, so no swaps are necessary
            0
        } else {
            let prev = FileEntry::find_child(children, arena, &child.name, prev_size).unwrap();
            let mut new =
                FileEntry::find_child(children, arena, &child.name, new_size).unwrap_err();

            let children = arena.get_mut(entry_id).children.as_mut().unwrap();
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
        let children = arena.get(entry_id).children.as_ref().unwrap();
        arena.get_mut(children[idx]).size = new_size;

        let entry = arena.get(entry_id);
        let new_size = entry.size + new_size - prev_size;
        if let Some(parent) = entry.parent {
            FileEntry::on_child_size_changed(arena, parent, entry_id, new_size);
        } else {
            arena.get_mut(entry_id).size = new_size;
        }
    }

    /// Returns crc of full path to this entry (XOR of parent crc and this crc)
    pub fn path_crc(&self) -> PathCrc {
        self.path_crc
    }

    /// Print this entry to stdout as tree with specified depth
    pub fn print(&self, arena: &Arena<FileEntry>, depth: usize) {
        // helper function to recursively populate entry tree
        fn _print<'a>(
            arena: &'a Arena<FileEntry>,
            entry: &'a FileEntry,
            builder: &mut TreeBuilder,
            depth: usize,
        ) {
            if entry.is_dir() {
                builder.begin_child(format!("d {} {}", entry.size, entry.name));

                if depth == 0 && !entry.children.as_ref().unwrap().is_empty() {
                    builder.add_empty_child("...".to_string());
                } else {
                    for child in entry.iter(arena) {
                        _print(arena, child, builder, depth - 1);
                    }
                }
                builder.end_child();
            } else {
                builder.add_empty_child(format!("f {} {}", entry.size, entry.name));
            }
        }

        let entry = self;
        if entry.is_dir() {
            // Build a file tree using a TreeBuilder
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
        } else {
            println!("f {} {}", entry.size, entry.name)
        }
    }

    /// Removes children vec from directory
    ///
    /// Entry is left in non consistent state (sizes are not updated)
    /// This function is used only in cleanup before entry is destroyed
    pub fn remove_children(&mut self) -> Option<Vec<Id>> {
        self.children.take()
    }

    /// Removes all marked children and returns them
    ///
    /// Returned ids are not removed from arena so cleanup is required
    /// Entries that were removed will be kept marked
    #[must_use]
    pub fn remove_marked(arena: &mut Arena<FileEntry>, entry_id: Id, expected: u32) -> Vec<Id> {
        let entry = arena.get_mut(entry_id);
        let mut children = entry.children.take().expect("directory has children");
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
        arena.get_mut(entry_id).children = Some(children);
        FileEntry::set_size(arena, entry_id, new_size);

        removed
    }

    /// Set new size of given entry
    pub fn set_size(arena: &mut Arena<FileEntry>, entry_id: Id, new_size: i64) {
        let entry = arena.get_mut(entry_id);
        if let Some(parent) = entry.parent {
            // size of self will be changed inside this call
            // after it will be reordered in children vec
            if entry.size != new_size {
                FileEntry::on_child_size_changed(arena, parent, entry_id, new_size);
            }
        } else {
            entry.size = new_size;
        }
    }

    pub fn unmark(&mut self) {
        self.is_marked = false;
    }
}

#[cfg(test)]
mod tests {
    use crate::arena::{Arena, Id};
    use crate::entry::FileEntry;
    use crate::path::EntryPath;

    fn new_dir<T: Into<String>>(arena: &mut Arena<FileEntry>, name: T) -> Id {
        arena.put(FileEntry::new_dir(name.into(), 0))
    }

    fn new_file<T: Into<String>>(arena: &mut Arena<FileEntry>, name: T, size: i64) -> Id {
        arena.put(FileEntry::new_file(name.into(), size))
    }

    #[test]
    fn add_child() {
        let mut arena = Arena::default();

        let root = new_dir(&mut arena, "root");
        let dir1 = new_dir(&mut arena, "dir1");
        FileEntry::add_child(&mut arena, root, dir1);
        let dir2 = new_dir(&mut arena, "dir2");
        let file0 = new_file(&mut arena, "file0", 5);
        FileEntry::add_child(&mut arena, dir2, file0);
        FileEntry::add_child(&mut arena, root, dir2);
        let file1 = new_file(&mut arena, "file1", 10);
        FileEntry::add_child(&mut arena, root, file1);
        let file2 = new_file(&mut arena, "file2", 10);
        FileEntry::add_child(&mut arena, root, file2);
        let file3 = new_file(&mut arena, "file3", 30);
        FileEntry::add_child(&mut arena, root, file3);
        let dir3 = new_dir(&mut arena, "dir3");
        FileEntry::add_child(&mut arena, root, dir3);

        let dir4 = new_dir(&mut arena, "dir4");

        let dir5 = new_dir(&mut arena, "dir5");

        let file7 = new_file(&mut arena, "file7", 35);
        FileEntry::add_child(&mut arena, dir5, file7);
        let file8 = new_file(&mut arena, "file8", 10);
        FileEntry::add_child(&mut arena, dir5, file8);
        FileEntry::add_child(&mut arena, dir4, dir5);

        let file9 = new_file(&mut arena, "file9", 15);
        FileEntry::add_child(&mut arena, dir4, file9);
        let file10 = new_file(&mut arena, "file10", 20);
        FileEntry::add_child(&mut arena, dir4, file10);
        FileEntry::add_child(&mut arena, root, dir4);

        let root = arena.get(root);
        root.print(&arena, 5);

        assert_eq!(root.size, 135);
        let mut iter = root.iter(&arena);

        let entry = iter.next().unwrap();
        assert_eq!(entry.name, "dir4");
        assert_eq!(entry.size, 80);

        {
            let mut iter = entry.iter(&arena);

            let entry = iter.next().unwrap();
            assert_eq!(entry.name, "dir5");
            assert_eq!(entry.size, 45);

            {
                let mut iter = entry.iter(&arena);

                let entry = iter.next().unwrap();
                assert_eq!(entry.name, "file7");
                assert_eq!(entry.size, 35);

                let entry = iter.next().unwrap();
                assert_eq!(entry.name, "file8");
                assert_eq!(entry.size, 10);

                assert!(iter.next().is_none());
            }

            let entry = iter.next().unwrap();
            assert_eq!(entry.name, "file10");
            assert_eq!(entry.size, 20);

            let entry = iter.next().unwrap();
            assert_eq!(entry.name, "file9");
            assert_eq!(entry.size, 15);

            assert!(iter.next().is_none());
        }

        let entry = iter.next().unwrap();
        assert_eq!(entry.name, "file3");
        assert_eq!(entry.size, 30);

        let entry = iter.next().unwrap();
        assert_eq!(entry.name, "file1");
        assert_eq!(entry.size, 10);

        let entry = iter.next().unwrap();
        assert_eq!(entry.name, "file2");
        assert_eq!(entry.size, 10);

        let entry = iter.next().unwrap();
        assert_eq!(entry.name, "dir2");
        assert_eq!(entry.size, 5);
        {
            let mut iter = entry.iter(&arena);

            let entry = iter.next().unwrap();
            assert_eq!(entry.name, "file0");
            assert_eq!(entry.size, 5);

            assert!(iter.next().is_none());
        }

        let entry = iter.next().unwrap();
        assert_eq!(entry.name, "dir1");
        assert_eq!(entry.size, 0);

        let entry = iter.next().unwrap();
        assert_eq!(entry.name, "dir3");
        assert_eq!(entry.size, 0);

        assert!(iter.next().is_none());
    }

    #[test]
    fn compare_path() {
        let mut arena = Arena::default();

        let root = new_dir(&mut arena, "root");
        let dir1 = new_dir(&mut arena, "dir1");
        let dir2 = new_dir(&mut arena, "dir2");
        FileEntry::add_child(&mut arena, dir1, dir2);
        FileEntry::add_child(&mut arena, root, dir1);

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
        let files = vec![
            ("file1", 7),
            ("file2", 6),
            ("file3", 4),
            ("file4", 4),
            ("file5", 2),
        ];
        let children: Vec<_> = files
            .iter()
            .map(|&(file, size)| new_file(&mut arena, file, size))
            .collect();

        for &(search_name, _) in &files {
            for search_size in 1..=8 {
                match FileEntry::find_child(&children, &arena, search_name, search_size) {
                    Ok(pos) => assert_eq!(files[pos], (search_name, search_size)),
                    Err(pos) if pos < files.len() => {
                        let (found_name, found_size) = files[pos];
                        if search_size == found_size {
                            assert!(search_name < found_name);
                        } else {
                            assert!(search_size > found_size);
                        }
                    }
                    Err(_) => {
                        let &(last_name, last_size) = files.last().unwrap();
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
        let dir2 = new_dir(&mut arena, "dir2");
        let file1 = new_file(&mut arena, "file1", 15);
        let file2 = new_file(&mut arena, "file2", 25);
        let file3 = new_file(&mut arena, "file3", 10);
        let file4 = new_file(&mut arena, "file4", 40);
        FileEntry::add_child(&mut arena, root, dir1);
        FileEntry::add_child(&mut arena, dir1, dir2);
        FileEntry::add_child(&mut arena, dir1, file1);
        FileEntry::add_child(&mut arena, dir1, file2);
        FileEntry::add_child(&mut arena, dir1, file3);
        FileEntry::add_child(&mut arena, root, file4);
        arena.get(root).print(&arena, 5);

        let (dirs, files) = FileEntry::mark_children(&mut arena, dir1);
        assert_eq!((dirs, files), (1, 3));

        arena.get_mut(dir2).unmark();
        arena.get_mut(file1).unmark();

        let removed = FileEntry::remove_marked(&mut arena, dir1, 0);
        arena.get(root).print(&arena, 5);
        assert_eq!(removed.len(), 2);
        assert!(removed.contains(&file2));
        assert!(removed.contains(&file3));

        let new_dir1 = arena.get(dir1);
        assert_eq!(new_dir1.size, 15);
        let left = new_dir1.children.as_ref().unwrap();
        assert_eq!(left.len(), 2);
        assert_eq!(left[0], file1);
        assert_eq!(left[1], dir2);

        let root = arena.get(root).children.as_ref().unwrap();
        assert_eq!(root.len(), 2);
        assert_eq!(root[0], file4);
        assert_eq!(root[1], dir1);
    }

    #[test]
    fn child_size_changed() {
        let mut arena = Arena::default();

        let files = vec![
            ("file1", 6),
            ("file2", 5),
            ("file3", 3),
            ("file4", 3),
            ("file5", 2),
        ];

        let root = new_dir(&mut arena, "root");
        for &(file, size) in &files {
            let id = new_file(&mut arena, file, size);
            FileEntry::add_child(&mut arena, root, id);
        }
        arena.get(root).print(&arena, 5);

        for (file, _) in files {
            let file = arena
                .get(root)
                .children
                .as_ref()
                .unwrap()
                .iter()
                .copied()
                .find(|&id| arena.get(id).get_name() == file)
                .unwrap();
            let initial_size = arena.get(file).size;

            for new_size in 1..=7 {
                for size in [new_size, initial_size] {
                    FileEntry::on_child_size_changed(&mut arena, root, file, size);

                    let children = arena.get(root).children.as_ref().cloned().unwrap();
                    let mut sorted = children.clone();
                    sorted.sort_by(|&a, &b| {
                        let a = arena.get(a);
                        let b = arena.get(b);
                        if a.size == b.size {
                            a.name.cmp(&b.name)
                        } else {
                            b.size.cmp(&a.size)
                        }
                    });
                    assert_eq!(children, sorted);
                }
            }
        }
    }
}
