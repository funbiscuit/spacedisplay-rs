use crate::arena::{Arena, Id};
use crate::entry::FileEntry;
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

impl TreeSnapshot<EntrySnapshot> {
    pub fn create(
        root: Id,
        arena: &Arena<FileEntry>,
        config: SnapshotConfig,
    ) -> TreeSnapshot<EntrySnapshot> {
        TreeSnapshot::create_wrapped(root, arena, config, Box::new(std::convert::identity))
    }
}

impl<W: AsRef<EntrySnapshot> + AsMut<EntrySnapshot>> TreeSnapshot<W> {
    pub fn create_wrapped(
        root: Id,
        arena: &Arena<FileEntry>,
        config: SnapshotConfig,
        wrapper: Box<dyn Fn(EntrySnapshot) -> W>,
    ) -> Self {
        let entry = arena.get(root);
        assert!(entry.is_dir(), "Snapshots can be created only for dirs");
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

        tree.fill_snapshot(root, entry, arena, config, &wrapper);

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
        entry: &FileEntry,
        arena: &Arena<FileEntry>,
        config: SnapshotConfig,
        wrapper: &dyn Fn(EntrySnapshot) -> W,
    ) {
        if config.max_depth == 0 {
            self.arena.get_mut(id).as_mut().set_children(vec![]);
            return;
        }

        let children: Vec<_> = entry
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

                if e.is_dir() {
                    self.fill_snapshot(
                        id,
                        e,
                        arena,
                        SnapshotConfig {
                            max_depth: config.max_depth - 1,
                            ..config.clone()
                        },
                        wrapper,
                    );
                }

                id
            })
            .collect();

        for &child in &children {
            self.arena.get_mut(child).as_mut().set_parent(id);
        }

        let snapshot = self.arena.get_mut(id).as_mut();
        snapshot.set_children(children);
    }
}
