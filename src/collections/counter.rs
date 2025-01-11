use std::cmp::Reverse;
use std::collections::HashMap;
use std::hash::Hash;

use rayon::prelude::*;
use topk::FilteredSpaceSaving;

use super::fixed_reverse_heap::FixedReverseHeap;

pub struct ExactCounter<K: Eq + Hash + Send + Ord> {
    map: HashMap<K, u64>,
}

impl<K: Eq + Hash + Send + Ord> ExactCounter<K> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn add(&mut self, key: K) {
        self.map
            .entry(key)
            .and_modify(|count| *count += 1)
            .or_insert(1);
    }

    pub fn into_total_and_sorted_vec(self, parallel: bool) -> (u64, Vec<(K, u64)>) {
        let mut total: u64 = 0;

        let mut items = self
            .map
            .into_iter()
            .map(|(v, c)| {
                total += c;
                (v, c)
            })
            .collect::<Vec<_>>();

        if parallel {
            items.par_sort_unstable_by(|a, b| a.1.cmp(&b.1).reverse().then_with(|| a.0.cmp(&b.0)));
        } else {
            items.sort_unstable_by(|a, b| a.1.cmp(&b.1).reverse().then_with(|| a.0.cmp(&b.0)));
        }

        (total, items)
    }

    pub fn into_total_and_top(self, k: usize, parallel: bool) -> (u64, Vec<(K, u64)>) {
        if k < (self.map.len() as f64 / 2.0).floor() as usize {
            let (total, mut items) = self.into_total_and_sorted_vec(parallel);
            items.truncate(k);

            return (total, items);
        }

        let mut heap: FixedReverseHeap<(u64, Reverse<K>)> = FixedReverseHeap::with_capacity(k);
        let mut total: u64 = 0;

        for (value, count) in self.map {
            total += count;

            heap.push((count, Reverse(value)));
        }

        let items = heap
            .into_sorted_vec()
            .into_iter()
            .map(|(count, Reverse(value))| (value, count))
            .collect::<Vec<_>>();

        (total, items)
    }

    pub fn into_total_and_items(
        self,
        limit: Option<usize>,
        parallel: bool,
    ) -> (u64, Vec<(K, u64)>) {
        if let Some(k) = limit {
            self.into_total_and_top(k, parallel)
        } else {
            self.into_total_and_sorted_vec(parallel)
        }
    }
}

pub struct ApproxCounter<K: Eq + Hash + Send + Ord> {
    map: FilteredSpaceSaving<K>,
}

impl<K: Eq + Hash + Send + Ord> ApproxCounter<K> {
    pub fn new(k: usize) -> Self {
        Self {
            map: FilteredSpaceSaving::new(k),
        }
    }

    pub fn add(&mut self, key: K) {
        self.map.insert(key, 1);
    }

    pub fn into_total_and_top(self) -> (u64, Vec<(K, u64)>) {
        let total = self.map.count();
        let items = self
            .map
            .into_sorted_iter()
            .map(|(k, c)| (k, c.estimated_count()))
            .collect();

        (total, items)
    }
}

pub enum Counter<K: Eq + Hash + Send + Ord> {
    Exact(ExactCounter<K>),
    Approximate(Box<ApproxCounter<K>>),
}

impl<K: Eq + Hash + Send + Ord> Counter<K> {
    pub fn new(approx_capacity: Option<usize>) -> Self {
        match approx_capacity {
            Some(k) => Self::Approximate(Box::new(ApproxCounter::new(k))),
            None => Self::Exact(ExactCounter::new()),
        }
    }

    pub fn add(&mut self, key: K) {
        match self {
            Self::Exact(inner) => {
                inner.add(key);
            }
            Self::Approximate(inner) => {
                inner.add(key);
            }
        }
    }

    pub fn into_total_and_items(
        self,
        limit: Option<usize>,
        parallel: bool,
    ) -> (u64, Vec<(K, u64)>) {
        match self {
            Self::Exact(inner) => inner.into_total_and_items(limit, parallel),
            Self::Approximate(inner) => inner.into_total_and_top(),
        }
    }
}