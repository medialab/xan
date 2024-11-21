use std::cmp::{Ordering, Reverse};
use std::collections::HashMap;

use crate::collections::FixedReverseHeap;

#[derive(Debug, Clone)]
pub struct Frequencies {
    counter: HashMap<String, u64>,
}

impl Frequencies {
    pub fn new() -> Self {
        Self {
            counter: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.counter.clear();
    }

    pub fn add_count(&mut self, value: String, count: u64) {
        self.counter
            .entry(value)
            .and_modify(|current| *current += count)
            .or_insert(count);
    }

    pub fn add(&mut self, value: String) {
        self.add_count(value, 1);
    }

    pub fn mode(&self) -> Option<String> {
        let mut max: Option<(u64, &String)> = None;

        for (key, count) in self.counter.iter() {
            max = match max {
                None => Some((*count, key)),
                Some(entry) => {
                    if (*count, key) > entry {
                        Some((*count, key))
                    } else {
                        max
                    }
                }
            }
        }

        max.map(|(_, key)| key.to_string())
    }

    pub fn modes(&self) -> Option<Vec<String>> {
        let mut max: Option<(u64, Vec<&String>)> = None;

        for (key, count) in self.counter.iter() {
            match max.as_mut() {
                None => {
                    max = Some((*count, vec![key]));
                }
                Some(entry) => match count.cmp(&entry.0) {
                    Ordering::Greater => {
                        max = Some((*count, vec![key]));
                    }
                    Ordering::Equal => {
                        entry.1.push(key);
                    }
                    _ => (),
                },
            };
        }

        max.map(|(_, keys)| keys.into_iter().cloned().collect())
    }

    pub fn most_common(&self, k: usize) -> Vec<String> {
        let mut heap = FixedReverseHeap::<(u64, Reverse<&String>)>::with_capacity(k);

        for (key, count) in self.counter.iter() {
            heap.push((*count, Reverse(key)));
        }

        heap.into_sorted_vec()
            .into_iter()
            .map(|(_, Reverse(value))| value.clone())
            .collect()
    }

    pub fn most_common_counts(&self, k: usize) -> Vec<u64> {
        let mut heap = FixedReverseHeap::<(u64, Reverse<&String>)>::with_capacity(k);

        for (key, count) in self.counter.iter() {
            heap.push((*count, Reverse(key)));
        }

        heap.into_sorted_vec()
            .into_iter()
            .map(|(count, _)| count)
            .collect()
    }

    pub fn cardinality(&self) -> usize {
        self.counter.len()
    }

    pub fn join(&self, separator: &str) -> String {
        let mut keys: Vec<_> = self.counter.keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        keys.join(separator)
    }

    pub fn merge(&mut self, other: Self) {
        for (key, count) in other.counter {
            self.add_count(key, count);
        }
    }
}
