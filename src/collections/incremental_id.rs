use std::hash::Hash;

use crate::collections::HashMap;

#[derive(Debug)]
pub struct IncrementalId<K> {
    map: HashMap<K, usize>,
}

impl<K> IncrementalId<K> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

impl<K: Eq + Hash> IncrementalId<K> {
    pub fn get(&mut self, key: K) -> usize {
        let next_id = self.map.len();

        *self.map.entry(key).or_insert(next_id)
    }
}
