// An IndexMap variant checking last inserted key before
// performing any lookup.
// This is an optimization strategy that minimizes hashmap
// lookup when the data is clustered by key.
// Note that the output order of the map will be deterministic
// but that it is not guaranteed to be the insertion order!
use std::hash::Hash;

use indexmap::{map::Entry as IndexMapEntry, IndexMap};

#[derive(Debug, Clone)]
pub struct ClusteredInsertHashmap<K, V> {
    map: IndexMap<K, V>,
}

impl<K, V> Default for ClusteredInsertHashmap<K, V> {
    fn default() -> Self {
        Self {
            map: IndexMap::new(),
        }
    }
}

impl<K: Eq + Hash, V> ClusteredInsertHashmap<K, V> {
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
        if let Some(mut last_entry) = self.map.last_entry() {
            if last_entry.key() == &key {
                // Identical key, we just update
                callback_update(last_entry.get_mut());

                // Not inserted
                return false;
            }
        }

        let len = self.map.len();

        match self.map.entry(key) {
            IndexMapEntry::Vacant(entry) => {
                entry.insert(callback_insert());

                // Inserted
                true
            }
            IndexMapEntry::Occupied(mut entry) => {
                callback_update(entry.get_mut());

                // NOTE: here, we know we are not the last entry, so we need to
                // swap the entry to move it to last position
                // NOTE: here we also know that there are more than 2 elements in the map
                // so this minus 1 is safe.
                debug_assert!(len > 1);
                entry.swap_indices(len - 1);

                // Not inserted
                false
            }
        }
    }

    pub fn insert_or_update_with<F>(&mut self, key: K, value: V, callback: F) -> bool
    where
        F: FnOnce(&mut V, V),
    {
        if let Some(mut last_entry) = self.map.last_entry() {
            if last_entry.key() == &key {
                // Identical key, we just update
                callback(last_entry.get_mut(), value);

                // Not inserted
                return false;
            }
        }

        let len = self.map.len();

        match self.map.entry(key) {
            IndexMapEntry::Vacant(entry) => {
                entry.insert(value);

                // Inserted
                true
            }
            IndexMapEntry::Occupied(mut entry) => {
                callback(entry.get_mut(), value);

                // NOTE: here, we know we are not the last entry, so we need to
                // swap the entry to move it to last position
                // NOTE: here we also know that there are more than 2 elements in the map
                // so this minus 1 is safe.
                debug_assert!(len > 1);
                entry.swap_indices(len - 1);

                // Not inserted
                false
            }
        }
    }

    pub fn insert_with<F>(&mut self, key: K, callback: F) -> &mut V
    where
        F: FnOnce() -> V,
    {
        let index = 'index: {
            let len = self.map.len();

            if let Some((last_key, _)) = self.map.last() {
                if last_key == &key {
                    break 'index len - 1;
                }
            }

            match self.map.entry(key) {
                IndexMapEntry::Vacant(entry) => {
                    entry.insert(callback());
                    len
                }
                IndexMapEntry::Occupied(entry) => {
                    debug_assert!(len > 1);
                    entry.swap_indices(len - 1);
                    len - 1
                }
            }
        };

        &mut self.map.as_mut_slice()[index]
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.map.get(key)
    }

    // pub fn values(&self) -> impl Iterator<Item = &V> {
    //     self.map.values()
    // }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.map.iter()
    }

    pub fn into_iter(self) -> impl Iterator<Item = (K, V)> {
        self.map.into_iter()
    }

    pub fn into_values(self) -> impl Iterator<Item = V> {
        self.map.into_values()
    }
}
