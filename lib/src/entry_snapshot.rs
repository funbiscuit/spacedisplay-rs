use std::ops::Deref;

use byte_unit::Byte;

use crate::arena::{Arena, Id};

#[derive(Debug)]
pub struct EntrySnapshot {
    /// id of this entry in Arena
    id: Id,

    name: String,

    size: Byte,

    parent: Option<Id>,

    children: Option<Vec<Id>>,
}

impl EntrySnapshot {
    pub fn get_children_count(&self) -> usize {
        self.children.as_ref().map(|s| s.len()).unwrap_or(0)
    }

    pub fn get_id(&self) -> Id {
        self.id
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_parent_id(&self) -> Option<Id> {
        self.parent
    }

    pub fn get_size(&self) -> Byte {
        self.size
    }

    pub fn is_dir(&self) -> bool {
        self.children.is_some()
    }

    /// Returns new snapshot with given parameters
    ///
    /// Parent and children of snapshot are empty and should be set explicitly
    pub fn new(id: Id, name: String, size: i64) -> Self {
        assert!(size >= 0);
        EntrySnapshot {
            id,
            name,
            size: Byte::from_bytes(size as u64),
            parent: None,
            children: None,
        }
    }

    /// Sets new children of this snapshot
    ///
    /// Ordering of children is not checked and kept as is
    pub fn set_children(&mut self, children: Vec<Id>) {
        self.children = Some(children);
    }

    /// Sets new parent of this snapshot
    pub fn set_parent(&mut self, id: Id) {
        self.parent = Some(id);
    }
}

impl AsRef<EntrySnapshot> for EntrySnapshot {
    fn as_ref(&self) -> &EntrySnapshot {
        self
    }
}

impl AsMut<EntrySnapshot> for EntrySnapshot {
    fn as_mut(&mut self) -> &mut EntrySnapshot {
        self
    }
}

#[derive(Debug)]
pub struct SnapshotRefIterator<'a, W> {
    /// Iterator over children ids
    entries: std::slice::Iter<'a, Id>,

    arena: &'a Arena<W>,
}

impl<'a, W> Iterator for SnapshotRefIterator<'a, W> {
    type Item = EntrySnapshotRef<'a, W>;

    fn next(&mut self) -> Option<Self::Item> {
        self.entries.next().map(|&id| EntrySnapshotRef {
            id,
            arena: self.arena,
        })
    }
}

#[derive(Clone, Debug)]
pub struct EntrySnapshotRef<'a, W> {
    id: Id,
    arena: &'a Arena<W>,
}

impl<'a, W: AsRef<EntrySnapshot>> EntrySnapshotRef<'a, W> {
    pub fn iter(&self) -> impl Iterator<Item = EntrySnapshotRef<'a, W>> {
        self.arena
            .get(self.id)
            .as_ref()
            .children
            .as_ref()
            .expect("iterate inside directory")
            .iter()
            .map(|&id| EntrySnapshotRef {
                id,
                arena: self.arena,
            })
    }

    /// Returns reference to n-th child
    pub fn get_nth_child(&self, n: usize) -> Option<EntrySnapshotRef<'a, W>> {
        self.as_ref()
            .children
            .as_ref()
            .expect("get child of dir")
            .get(n)
            .map(|&id| EntrySnapshotRef {
                id,
                arena: self.arena,
            })
    }

    pub fn get_parent(&self) -> Option<EntrySnapshotRef<'a, W>> {
        self.as_ref().parent.map(|id| EntrySnapshotRef {
            id,
            arena: self.arena,
        })
    }

    pub fn new(id: Id, arena: &'a Arena<W>) -> Option<Self> {
        if arena.contains(id) {
            Some(EntrySnapshotRef { id, arena })
        } else {
            None
        }
    }
}

impl<'a, W> Deref for EntrySnapshotRef<'a, W> {
    type Target = W;

    fn deref(&self) -> &Self::Target {
        self.arena.get(self.id)
    }
}
