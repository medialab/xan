use std::cmp::Reverse;
use std::convert::TryFrom;
use std::fmt;
use std::hash::Hash;

use rayon::prelude::*;
use topk::FilteredSpaceSaving;

use crate::collections::HashMap;

use super::heap::TopKHeap;

#[derive(Debug, Clone)]
pub struct ExactCounter<K: Eq + Hash + Send + Ord> {
    map: HashMap<K, u64>,
}

impl<K: Eq + Hash + Send + Ord> ExactCounter<K> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn add_n(&mut self, key: K, count: u64) {
        self.map
            .entry(key)
            .and_modify(|c| *c += count)
            .or_insert(count);
    }

    pub fn add(&mut self, key: K) {
        self.add_n(key, 1);
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

        let mut heap: TopKHeap<(u64, Reverse<K>)> = TopKHeap::with_capacity(k);
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

    pub fn merge(&mut self, mut other: Self) {
        if other.map.len() > self.map.len() {
            std::mem::swap(self, &mut other);
        }

        for (v, c) in other.map.into_iter() {
            self.add_n(v, c);
        }
    }

    fn iter(&self) -> impl Iterator<Item = (&K, u64)> {
        self.map.iter().map(|(k, c)| (k, *c))
    }

    fn cardinality(&self) -> u64 {
        self.map.len() as u64
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(try_from = "String")]
pub enum ApproxCounterAlgorithm {
    SpaceSaving,
    HeavyKeeper,
}

impl TryFrom<String> for ApproxCounterAlgorithm {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(match value.as_str() {
            "spacesaving" | "space_saving" | "space-saving" | "ss" => Self::SpaceSaving,
            "heavykeeper" | "heavy_keeper" | "heavy-keeper" | "hk" => Self::HeavyKeeper,
            _ => return Err(format!("unknown --approx-method {}", value)),
        })
    }
}

#[derive(Clone)]
pub struct SpaceSavingCounter<K: Eq + Hash + Send + Ord> {
    map: FilteredSpaceSaving<K>,
}

impl<K: Eq + Hash + Send + Ord> fmt::Debug for SpaceSavingCounter<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpaceSavingCounter").finish()
    }
}

impl<K: Eq + Hash + Send + Ord + Clone> SpaceSavingCounter<K> {
    pub fn new(k: usize) -> Self {
        Self {
            map: FilteredSpaceSaving::new(k),
        }
    }

    #[inline]
    pub fn add_n(&mut self, key: K, count: u64) {
        self.map.insert(key, count);
    }

    #[inline]
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

    fn cardinality(&self) -> u64 {
        self.map.count()
    }

    fn iter(&self) -> impl Iterator<Item = (&K, u64)> {
        self.map.iter().map(|(k, c)| (k, c.estimated_count()))
    }

    pub fn merge(&mut self, other: Self) {
        self.map.merge(&other.map).unwrap()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CounterSpec {
    Exact,
    SpaceSaving(usize),
}

#[derive(Debug, Clone)]
pub enum Counter<K: Eq + Hash + Send + Ord> {
    Exact(ExactCounter<K>),
    SpaceSaving(Box<SpaceSavingCounter<K>>),
}

impl<K: Eq + Hash + Send + Ord + Clone> Counter<K> {
    pub fn new(spec: CounterSpec) -> Self {
        match spec {
            CounterSpec::SpaceSaving(k) => Self::SpaceSaving(Box::new(SpaceSavingCounter::new(k))),
            CounterSpec::Exact => Self::Exact(ExactCounter::new()),
        }
    }

    pub fn cardinality(&self) -> u64 {
        match self {
            Self::Exact(inner) => inner.cardinality(),
            Self::SpaceSaving(inner) => inner.cardinality(),
        }
    }

    pub fn add(&mut self, key: K) {
        match self {
            Self::Exact(inner) => {
                inner.add(key);
            }
            Self::SpaceSaving(inner) => {
                inner.add(key);
            }
        }
    }

    pub fn add_n(&mut self, key: K, count: u64) {
        match self {
            Self::Exact(inner) => {
                inner.add_n(key, count);
            }
            Self::SpaceSaving(inner) => {
                inner.add_n(key, count);
            }
        }
    }

    pub fn iter(&self) -> Box<dyn Iterator<Item = (&K, u64)> + '_> {
        match self {
            Self::Exact(inner) => Box::new(inner.iter()),
            Self::SpaceSaving(inner) => Box::new(inner.iter()),
        }
    }

    pub fn into_total_and_items(
        self,
        limit: Option<usize>,
        parallel: bool,
    ) -> (u64, Vec<(K, u64)>) {
        match self {
            Self::Exact(inner) => inner.into_total_and_items(limit, parallel),
            Self::SpaceSaving(inner) => inner.into_total_and_top(),
        }
    }

    pub fn merge(&mut self, other: Self) {
        match (self, other) {
            (Self::Exact(inner_self), Self::Exact(inner_other)) => inner_self.merge(inner_other),
            (Self::SpaceSaving(inner_self), Self::SpaceSaving(inner_other)) => {
                inner_self.merge(*inner_other)
            }
            _ => unreachable!(),
        };
    }
}
