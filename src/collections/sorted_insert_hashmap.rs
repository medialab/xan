// A hashmap variant caching its last inserted key to minimize lookups
// when inserting sorted keys.
use std::cell::{Ref, RefCell, RefMut};
use std::collections::{hash_map::Entry, HashMap};
use std::hash::Hash;
use std::rc::Rc;

#[derive(Debug)]
pub struct SortedInsertHashmap<K, V> {
    map: HashMap<Rc<K>, Rc<RefCell<V>>>,
    last_entry: Option<(Rc<K>, Rc<RefCell<V>>)>,
}

impl<K, V> Default for SortedInsertHashmap<K, V> {
    fn default() -> Self {
        Self {
            map: HashMap::new(),
            last_entry: None,
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
        mut callback_update: U,
    ) -> bool
    where
        I: Fn() -> V,
        U: FnMut(RefMut<V>),
    {
        if let Some((last_key, value_cell)) = self.last_entry.as_mut() {
            if last_key.as_ref() == &key {
                // Identical key, we just update
                callback_update(value_cell.borrow_mut());
                return false;
            }
        }

        let key = Rc::new(key);
        let mut key_was_inserted = false;

        let cell = match self.map.entry(key.clone()) {
            Entry::Occupied(mut entry) => {
                callback_update(entry.get_mut().borrow_mut());
                entry.get().clone()
            }
            Entry::Vacant(entry) => {
                key_was_inserted = true;
                let cell = Rc::new(RefCell::new(callback_insert()));
                entry.insert(cell.clone());
                cell
            }
        };

        self.last_entry = Some((key, cell));

        key_was_inserted
    }

    pub fn values(&self) -> impl Iterator<Item = Ref<V>> {
        self.map.values().map(|cell| cell.borrow())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, Ref<V>)> {
        self.map.iter().map(|(k, cell)| (k.as_ref(), cell.borrow()))
    }

    pub fn into_iter(self) -> impl Iterator<Item = (K, V)> {
        self.map.into_iter().map(|(k, v)| {
            (
                Rc::into_inner(k).unwrap(),
                Rc::into_inner(v).unwrap().into_inner(),
            )
        })
    }

    pub fn into_values(self) -> impl Iterator<Item = V> {
        self.map
            .into_iter()
            .map(|(_, v)| Rc::into_inner(v).unwrap().into_inner())
    }
}
