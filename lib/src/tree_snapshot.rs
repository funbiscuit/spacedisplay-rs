use std::path::Path;

use crate::arena::{Arena, Id};
use crate::entry::DirEntry;
use crate::entry_snapshot::EntrySnapshotRef;
use crate::EntrySnapshot;

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
        files_getter: &dyn Fn(&Path) -> Vec<(String, i64)>,
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
        // get files for this entry
        let files = files_getter(&path);

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
