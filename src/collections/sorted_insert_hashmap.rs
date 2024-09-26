// A hashmap variant caching its last inserted key to minimize lookups
// when inserting sorted runs of keys.
// What's more it is able to iterate over its components in insertion
// order by relying on an auxilliary Vec. It cannot be made to support
// deletions efficiently but it was not designed for this anyway.
use std::collections::{hash_map::Entry, HashMap};
use std::hash::Hash;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct SortedInsertHashmap<K, V> {
    map: HashMap<Rc<K>, usize>,
    last_entry: Option<usize>,
    order: Vec<(Rc<K>, V)>,
}

impl<K, V> Default for SortedInsertHashmap<K, V> {
    fn default() -> Self {
        Self {
            map: HashMap::new(),
            last_entry: None,
            order: Vec::new(),
        }
    }
}

impl<K: Eq + Hash, V> SortedInsertHashmap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn insert_with_or_else<I, U>(
        &mut self,
        key: K,
        callback_insert: I,
        callback_update: U,
    ) -> bool
    where
        I: FnOnce() -> V,
        U: FnOnce(&mut V),
    {
        if let Some(item_index) = self.last_entry {
            let (last_key, value) = &mut self.order[item_index];

            if last_key.as_ref() == &key {
                // Identical key, we just update
                callback_update(value);
                return false;
            }
        }

        let key = Rc::new(key);
        let mut key_was_inserted = false;

        let item_index = match self.map.entry(key.clone()) {
            Entry::Occupied(entry) => {
                let item_index = *entry.get();
                let value = &mut self.order[item_index].1;
                callback_update(value);
                item_index
            }
            Entry::Vacant(entry) => {
                key_was_inserted = true;
                let item_index = self.order.len();
                entry.insert(item_index);
                self.order.push((key, callback_insert()));
                item_index
            }
        };

        self.last_entry = Some(item_index);

        key_was_inserted
    }

    pub fn insert_with<F>(&mut self, key: K, callback: F) -> &mut V
    where
        F: FnOnce() -> V,
    {
        self.insert_with_or_else(key, callback, |_| {});
        &mut self.order[self.last_entry.unwrap()].1
    }

    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.order.iter().map(|(k, _)| k.as_ref())
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.order.iter().map(|(_, cell)| cell)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.order.iter().map(|(k, cell)| (k.as_ref(), cell))
    }

    pub fn into_iter(self) -> impl Iterator<Item = (K, V)> {
        // NOTE: I don't really understand why but in the map function
        // `self.map` has already been dropped, and the strong count
        // of the Rc instances is 1 so we can into_inner them.
        self.order
            .into_iter()
            .map(|(k, v)| (Rc::into_inner(k).unwrap(), v))
    }

    pub fn into_values(self) -> impl Iterator<Item = V> {
        self.order.into_iter().map(|(_, v)| v)
    }
}
