use std::path::Path;

use byte_unit::Byte;
use ptree::TreeBuilder;

use crate::arena::{Arena, Id};
use crate::entry::DirEntry;
use crate::entry_snapshot::EntrySnapshotRef;
use crate::EntrySnapshot;

/// Function that is used to retrieve files
/// and their sizes at specified path
pub type FilesRetrieverFn = dyn Fn(&Path) -> Vec<(String, i64)>;

#[derive(Clone, Debug)]
pub struct SnapshotConfig {
    pub max_depth: usize,

    pub min_size: u64,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        SnapshotConfig {
            max_depth: 3,
            min_size: 0,
        }
    }
}

#[derive(Debug)]
pub struct TreeSnapshot<W: AsRef<EntrySnapshot> + AsMut<EntrySnapshot>> {
    /// Root of file tree
    root: Id,

    /// Arena where all entries are actually stored
    arena: Arena<W>,
}

impl<W: AsRef<EntrySnapshot> + AsMut<EntrySnapshot>> TreeSnapshot<W> {
    pub fn create_wrapped(
        root: Id,
        arena: &Arena<DirEntry>,
        config: SnapshotConfig,
        wrapper: &dyn Fn(EntrySnapshot) -> W,
        files_getter: &FilesRetrieverFn,
    ) -> Self {
        let entry = arena.get(root);
        let mut snapshots = Arena::default();

        let root = snapshots.put_with_id(|id| {
            wrapper(EntrySnapshot::new(
                id,
                entry.get_name().to_string(),
                entry.get_size(),
            ))
        });

        let mut tree = TreeSnapshot {
            root,
            arena: snapshots,
        };

        tree.fill_snapshot(root, entry, arena, config, wrapper, files_getter);

        tree
    }

    pub fn get_entry(&self, id: Id) -> Option<EntrySnapshotRef<'_, W>> {
        EntrySnapshotRef::new(id, &self.arena)
    }

    pub fn get_root(&self) -> EntrySnapshotRef<'_, W> {
        self.get_entry(self.root).unwrap()
    }

    /// Print this snapshot to stdout as tree with specified depth
    pub fn print(&self, size_formatter: &dyn Fn(Byte) -> String, depth: usize) {
        fn _entry_title(
            entry: &'_ EntrySnapshot,
            size_formatter: &dyn Fn(Byte) -> String,
        ) -> String {
            let t = if entry.is_dir() { "d" } else { "f" };
            let size = size_formatter(entry.get_size());
            format!("{} {} {}", t, size, entry.get_name())
        }

        // helper function to recursively populate entry tree
        fn _print<W2: AsRef<EntrySnapshot> + AsMut<EntrySnapshot>>(
            entry: EntrySnapshotRef<'_, W2>,
            size_formatter: &dyn Fn(Byte) -> String,
            builder: &mut TreeBuilder,
            depth: usize,
        ) {
            builder.begin_child(_entry_title(entry.as_ref(), size_formatter));

            if depth > 0 && entry.as_ref().is_dir() {
                for child in entry.iter() {
                    _print(child, size_formatter, builder, depth - 1);
                }
            }
            builder.end_child();
        }

        let entry = self.get_root();
        // Build a dir tree using a TreeBuilder
        let mut builder = TreeBuilder::new(_entry_title(entry.as_ref(), size_formatter));
        if depth > 0 {
            for child in entry.iter() {
                _print(child, size_formatter, &mut builder, depth - 1);
            }
        }
        let tree = builder.build();

        // write out the tree using default formatting
        let _ = ptree::print_tree(&tree);
    }

    fn fill_snapshot(
        &mut self,
        id: Id,
        entry: &DirEntry,
        arena: &Arena<DirEntry>,
        config: SnapshotConfig,
        wrapper: &dyn Fn(EntrySnapshot) -> W,
        files_getter: &dyn Fn(&Path) -> Vec<(String, i64)>,
    ) {
        if config.max_depth == 0 {
            self.arena.get_mut(id).as_mut().set_children(vec![]);
            return;
        }

        let mut children: Vec<_> = entry
            .iter(arena)
            .take_while(|e| e.get_size() >= config.min_size as i64)
            .map(|e| {
                let id = self.arena.put_with_id(|id| {
                    wrapper(EntrySnapshot::new(
                        id,
                        e.get_name().to_string(),
                        e.get_size(),
                    ))
                });

                self.fill_snapshot(
                    id,
                    e,
                    arena,
                    SnapshotConfig {
                        max_depth: config.max_depth - 1,
                        ..config.clone()
                    },
                    wrapper,
                    files_getter,
                );

                id
            })
            .collect();
        let path = entry.get_path(arena).get_path();
        // get files for this entry (only if it had any)
        let files = if entry.get_files() > 0 {
            files_getter(&path)
        } else {
            vec![]
        };

        children.extend(
            files
                .into_iter()
                // files are not sorted by size, so using filter instead of takeWhile
                .filter(|(_, size)| *size >= config.min_size as i64)
                .map(|(name, size)| {
                    self.arena
                        .put_with_id(|id| wrapper(EntrySnapshot::new(id, name, size)))
                }),
        );
        // need to sort after combining directories with files
        children.sort_by(|&a, &b| {
            let a = self.arena.get(a).as_ref();
            let b = self.arena.get(b).as_ref();

            b.get_size()
                .cmp(&a.get_size())
                .then_with(|| a.get_name().cmp(b.get_name()))
        });

        for &child in &children {
            self.arena.get_mut(child).as_mut().set_parent(id);
        }

        self.arena.get_mut(id).as_mut().set_children(children);
    }
}
