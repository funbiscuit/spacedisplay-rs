use std::num::NonZeroU32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Id(NonZeroU32);

impl Id {
    fn id(index: usize) -> Self {
        Id(NonZeroU32::new((index + 1) as u32).unwrap())
    }

    fn index(&self) -> usize {
        (self.0.get() - 1) as usize
    }
}

#[derive(Debug)]
pub struct Arena<T> {
    items: Vec<Option<T>>,
    unused: Vec<Id>,
}

impl<T> Arena<T> {
    /// Checks whether given id contains in Arena
    pub fn contains(&self, id: Id) -> bool {
        self.try_get(id).is_some()
    }

    /// Returns shared reference to an item if id is valid
    pub fn get(&self, id: Id) -> &T {
        self.try_get(id).expect("id is invalid")
    }

    /// Returns mutable reference to an item if id is valid
    pub fn get_mut(&mut self, id: Id) -> &mut T {
        self.try_get_mut(id).expect("id is invalid")
    }

    /// Adds new item to Arena and returns its id
    ///
    /// Returned id is unique only among other items in this Arena.
    /// It can be the same as id of some other item that was removed from Arena.
    pub fn put(&mut self, item: T) -> Id {
        if let Some(id) = self.unused.pop() {
            self.items[id.index()] = Some(item);
            id
        } else {
            self.items.push(Some(item));
            Id::id(self.items.len() - 1)
        }
    }

    /// Adds new item to Arena that requires its id at construction time
    ///
    /// Returned id is unique only among other items in this Arena.
    /// It can be the same as id of some other item that was removed from Arena.
    pub fn put_with_id<F: FnOnce(Id) -> T>(&mut self, supplier: F) -> Id {
        if let Some(id) = self.unused.pop() {
            self.items[id.index()] = Some(supplier(id));
            id
        } else {
            let id = Id::id(self.items.len());
            self.items.push(Some(supplier(id)));
            id
        }
    }

    /// Remove item with specified id from Arena
    ///
    /// Given id will be reused for next pushed element so accessing it later
    /// might give results other than None
    pub fn remove(&mut self, id: Id) -> Option<T> {
        if let Some(item) = self.items.get_mut(id.index()).and_then(|it| it.take()) {
            // save this id as unused so it can be reused later
            self.unused.push(id);
            Some(item)
        } else {
            None
        }
    }

    /// Returns shared reference to an item if id is valid
    pub fn try_get(&self, id: Id) -> Option<&T> {
        self.items.get(id.index()).and_then(|e| e.as_ref())
    }

    /// Returns mutable reference to an item if id is valid
    pub fn try_get_mut(&mut self, id: Id) -> Option<&mut T> {
        self.items.get_mut(id.index()).and_then(|e| e.as_mut())
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Arena {
            items: vec![],
            unused: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tree::arena::Arena;

    #[test]
    fn put_and_remove() {
        let mut arena = Arena::default();
        let id1 = arena.put("test".to_string());
        let id2 = arena.put("test2".to_string());

        assert_eq!(arena.get(id1), "test");
        assert_eq!(arena.get(id2), "test2");

        assert_eq!(arena.remove(id1), Some("test".to_string()));

        let id3 = arena.put("test3".to_string());
        assert_eq!(arena.get(id3), "test3");

        // removed item should be reused
        assert_eq!(arena.items.len(), 2);
    }

    #[test]
    fn put_with_id() {
        let mut arena = Arena::default();
        let mut actual = None;
        let id = arena.put_with_id(|id| {
            actual = Some(id);
            "test".to_string()
        });

        assert_eq!(Some(id), actual);
        assert_eq!(arena.get(id), "test");
        assert_eq!(arena.items.len(), 1);
    }
}
