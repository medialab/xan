mod clustered_insert_hashmap;
mod context_buffer;
mod counter;
mod heap;
mod incremental_id;
mod union_find;

pub use clustered_insert_hashmap::ClusteredInsertHashmap;
pub use context_buffer::ContextBuffer;
pub use counter::Counter;
pub use heap::{DynamicOrd, TopKHeap, TopKHeapMap, TopKHeapMapWithTies};
pub use incremental_id::IncrementalId;
pub use union_find::UnionFind;

pub use ahash::AHashMap as HashMap;
pub use ahash::AHashSet as HashSet;

pub mod hash_map {
    pub use std::collections::hash_map::Entry;
}

pub type IndexMap<K, V> = indexmap::IndexMap<K, V, ahash::RandomState>;
pub type IndexSet<T> = indexmap::IndexSet<T, ahash::RandomState>;

pub fn new_index_map<K, V>() -> IndexMap<K, V> {
    IndexMap::with_hasher(ahash::RandomState::new())
}

pub fn new_index_set<T>() -> IndexSet<T> {
    IndexSet::with_hasher(ahash::RandomState::new())
}

pub mod index_map {
    pub use indexmap::map::Entry;
}
