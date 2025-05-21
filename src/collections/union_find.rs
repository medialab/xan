// use crate::collections::{hash_map::Entry, HashMap};
// use std::hash::Hash;

#[derive(Debug)]
struct UnionFindEntry {
    parent: usize,
    size: usize,
}

#[derive(Debug)]
pub struct UnionFind {
    entries: Vec<UnionFindEntry>,
}

impl UnionFind {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn make_set(&mut self) -> usize {
        let i = self.entries.len();

        self.entries.push(UnionFindEntry { parent: i, size: 1 });

        i
    }

    pub fn find(&self, x: usize) -> usize {
        let mut root = x;

        loop {
            let parent = self.entries[root].parent;

            if parent == root {
                break;
            }

            root = parent;
        }

        root
    }

    fn find_mut(&mut self, mut x: usize) -> usize {
        let mut root = x;

        loop {
            let parent = self.entries[root].parent;

            if parent == root {
                break;
            }

            root = parent;
        }

        // Path compression
        loop {
            let entry = &mut self.entries[x];

            if entry.parent == root {
                break;
            }

            let parent = entry.parent;
            entry.parent = root;
            x = parent;
        }

        root
    }

    pub fn union(&mut self, mut x: usize, mut y: usize) {
        x = self.find_mut(x);
        y = self.find_mut(y);

        if x == y {
            return;
        }

        let x_size = self.entries[x].size;
        let y_size = self.entries[y].size;

        if x_size > y_size {
            self.entries[y].parent = x;
            self.entries[x].size += y_size;
        } else {
            self.entries[x].parent = y;
            self.entries[y].size += x_size;
        }
    }

    fn leaders(&self) -> impl Iterator<Item = &UnionFindEntry> {
        self.entries.iter().enumerate().filter_map(|(i, entry)| {
            if i != entry.parent {
                None
            } else {
                Some(entry)
            }
        })
    }

    pub fn largest(&self) -> Option<usize> {
        let mut max: Option<&UnionFindEntry> = None;

        for entry in self.leaders() {
            match max {
                None => {
                    max = Some(entry);
                }
                Some(current_entry) => {
                    if entry.size > current_entry.size {
                        max = Some(entry);
                    }
                }
            }
        }

        max.map(|entry| entry.parent)
    }

    // fn sizes(&self) -> impl Iterator<Item = usize> + '_ {
    //     self.leaders().map(|entry| entry.size)
    // }
}

// #[derive(Debug)]
// pub struct UnionFindMap<K: Hash + Eq> {
//     inner: UnionFind,
//     map: HashMap<K, usize>,
// }

// impl<K: Hash + Eq> UnionFindMap<K> {
//     pub fn new() -> Self {
//         Self {
//             inner: UnionFind::new(),
//             map: HashMap::new(),
//         }
//     }

//     pub fn len(&self) -> usize {
//         self.map.len()
//     }

//     pub fn is_empty(&self) -> bool {
//         self.len() == 0
//     }

//     fn get(&mut self, node: K) -> usize {
//         match self.map.entry(node) {
//             Entry::Occupied(entry) => *entry.get(),
//             Entry::Vacant(entry) => *entry.insert(self.inner.make_set()),
//         }
//     }

//     pub fn union(&mut self, source: K, target: K) {
//         let x = self.get(source);
//         let y = self.get(target);

//         self.inner.union(x, y);
//     }

//     pub fn nodes(self) -> impl Iterator<Item = (K, usize)> {
//         self.map.into_iter().map(move |(node, i)| {
//             let label = self.inner.find(i);

//             (node, label)
//         })
//     }

//     pub fn largest_component(self) -> impl Iterator<Item = K> {
//         let largest = self.inner.largest().unwrap();

//         self.map.into_iter().flat_map(move |(node, i)| {
//             if self.inner.find(i) == largest {
//                 Some(node)
//             } else {
//                 None
//             }
//         })
//     }

//     pub fn sizes(&self) -> impl Iterator<Item = usize> + '_ {
//         self.inner.sizes()
//     }
// }
