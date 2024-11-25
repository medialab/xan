#![allow(dead_code)]

extern crate serde_derive;

extern crate csv;
extern crate filetime;
extern crate rand;

use std::fmt;
use std::mem::transmute;
use std::ops;

macro_rules! svec[
    ($($x:expr),*) => (
        vec![$($x),*].into_iter()
                     .map(|s: &str| s.to_string())
                     .collect::<Vec<String>>()
    );
    ($($x:expr,)*) => (svec![$($x),*]);
];

mod workdir;

mod test_agg;
mod test_behead;
mod test_cat;
mod test_count;
mod test_dedup;
mod test_enumerate;
mod test_explode;
mod test_filter;
mod test_fixlengths;
mod test_flatmap;
mod test_fmt;
mod test_frequency;
mod test_groupby;
mod test_headers;
mod test_implode;
mod test_index;
mod test_join;
mod test_map;
mod test_merge;
mod test_parallel;
mod test_partition;
mod test_range;
mod test_rename;
mod test_reverse;
mod test_sample;
mod test_search;
mod test_select;
mod test_shuffle;
mod test_slice;
mod test_sort;
mod test_split;
mod test_stats;
mod test_to;
mod test_tokenize;
mod test_top;
mod test_transform;
mod test_vocab;

pub type CsvVecs = Vec<Vec<String>>;

pub trait Csv {
    fn to_vecs(self) -> CsvVecs;
    fn from_vecs(_vecs: CsvVecs) -> Self;
}

impl Csv for CsvVecs {
    fn to_vecs(self) -> CsvVecs {
        self
    }
    fn from_vecs(vecs: CsvVecs) -> CsvVecs {
        vecs
    }
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
struct CsvRecord(Vec<String>);

impl CsvRecord {
    fn unwrap(self) -> Vec<String> {
        let CsvRecord(v) = self;
        v
    }
}

impl ops::Deref for CsvRecord {
    type Target = [String];
    fn deref<'a>(&'a self) -> &'a [String] {
        &self.0
    }
}

impl fmt::Debug for CsvRecord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let bytes: Vec<_> = self.iter().map(|s| s.as_bytes()).collect();
        write!(f, "{:?}", bytes)
    }
}

impl Csv for Vec<CsvRecord> {
    fn to_vecs(self) -> CsvVecs {
        unsafe { transmute(self) }
    }
    fn from_vecs(vecs: CsvVecs) -> Vec<CsvRecord> {
        unsafe { transmute(vecs) }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialOrd)]
struct CsvData {
    data: Vec<CsvRecord>,
}

impl CsvData {
    fn unwrap(self) -> Vec<CsvRecord> {
        self.data
    }

    fn len(&self) -> usize {
        (**self).len()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl ops::Deref for CsvData {
    type Target = [CsvRecord];
    fn deref<'a>(&'a self) -> &'a [CsvRecord] {
        &self.data
    }
}

impl Csv for CsvData {
    fn to_vecs(self) -> CsvVecs {
        unsafe { transmute(self.data) }
    }
    fn from_vecs(vecs: CsvVecs) -> CsvData {
        CsvData {
            data: unsafe { transmute(vecs) },
        }
    }
}

impl PartialEq for CsvData {
    fn eq(&self, other: &CsvData) -> bool {
        (self.data.is_empty() && other.data.is_empty()) || self.data == other.data
    }
}
