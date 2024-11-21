use std::cmp::Reverse;

use csv::ByteRecord;

use crate::collections::FixedReverseHeapMap;
use crate::moonblade::types::DynamicNumber;

#[derive(Debug, Clone)]
pub struct Extent<T: Copy + PartialOrd> {
    extent: Option<(T, T)>,
}

impl<T: Copy + PartialOrd> Extent<T> {
    pub fn new() -> Self {
        Self { extent: None }
    }

    pub fn clear(&mut self) {
        self.extent = None;
    }

    pub fn add(&mut self, value: T) {
        match &mut self.extent {
            None => self.extent = Some((value, value)),
            Some((min, max)) => {
                if value < *min {
                    *min = value;
                }

                if value > *max {
                    *max = value;
                }
            }
        }
    }

    pub fn min(&self) -> Option<T> {
        self.extent.map(|e| e.0)
    }

    pub fn max(&self) -> Option<T> {
        self.extent.map(|e| e.1)
    }

    pub fn merge(&mut self, other: Self) {
        match self.extent.as_mut() {
            None => {
                self.extent = other.extent;
            }
            Some((min, max)) => {
                if let Some((other_min, other_max)) = other.extent {
                    if other_min < *min {
                        *min = other_min;
                    }
                    if other_max > *max {
                        *max = other_max;
                    }
                }
            }
        }
    }
}

pub type NumericExtent = Extent<DynamicNumber>;

type ArgExtentEntry = (DynamicNumber, (usize, ByteRecord));

#[derive(Debug, Clone)]
pub struct ArgExtent {
    extent: Option<(ArgExtentEntry, ArgExtentEntry)>,
}

impl ArgExtent {
    pub fn new() -> Self {
        Self { extent: None }
    }

    pub fn clear(&mut self) {
        self.extent = None;
    }

    pub fn add(&mut self, index: usize, value: DynamicNumber, arg: &ByteRecord) {
        match &mut self.extent {
            None => {
                self.extent = Some(((value, (index, arg.clone())), (value, (index, arg.clone()))))
            }
            Some(((min, min_arg), (max, max_arg))) => {
                if value < *min {
                    *min = value;
                    *min_arg = (index, arg.clone());
                }

                if value > *max {
                    *max = value;
                    *max_arg = (index, arg.clone());
                }
            }
        }
    }

    pub fn min(&self) -> Option<DynamicNumber> {
        self.extent.as_ref().map(|e| e.0 .0)
    }

    pub fn max(&self) -> Option<DynamicNumber> {
        self.extent.as_ref().map(|e| e.1 .0)
    }

    pub fn argmin(&self) -> Option<&(usize, ByteRecord)> {
        self.extent.as_ref().map(|e| &e.0 .1)
    }

    pub fn argmax(&self) -> Option<&(usize, ByteRecord)> {
        self.extent.as_ref().map(|e| &e.1 .1)
    }

    pub fn merge(&mut self, other: Self) {
        match self.extent.as_mut() {
            None => {
                self.extent = other.extent;
            }
            Some(((min, arg_min), (max, arg_max))) => {
                if let Some(((other_min, arg_other_min), (other_max, arg_other_max))) = other.extent
                {
                    if other_min < *min {
                        *min = other_min;
                        *arg_min = arg_other_min;
                    }
                    if other_max > *max {
                        *max = other_max;
                        *arg_max = arg_other_max;
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArgTop {
    heap: FixedReverseHeapMap<(DynamicNumber, Reverse<usize>), ByteRecord>,
}

impl ArgTop {
    pub fn new(k: usize) -> Self {
        Self {
            heap: FixedReverseHeapMap::with_capacity(k),
        }
    }

    pub fn capacity(&self) -> usize {
        self.heap.capacity()
    }

    pub fn clear(&mut self) {
        self.heap.clear();
    }

    pub fn add(&mut self, index: usize, value: DynamicNumber, arg: &ByteRecord) {
        self.heap.push_with((value, Reverse(index)), || arg.clone());
    }

    pub fn top_indices(&self) -> impl Iterator<Item = usize> {
        self.heap
            .to_sorted_vec()
            .into_iter()
            .map(|((_, Reverse(i)), _)| i)
    }

    pub fn top_records(&self) -> impl Iterator<Item = (usize, ByteRecord)> {
        self.heap
            .to_sorted_vec()
            .into_iter()
            .map(|((_, Reverse(i)), r)| (i, r))
    }

    pub fn top_values(&self) -> impl Iterator<Item = DynamicNumber> {
        self.heap.to_sorted_vec().into_iter().map(|((v, _), _)| v)
    }

    pub fn merge(&mut self, other: Self) {
        for (k, v) in other.heap.into_unordered_iter() {
            self.heap.push_with(k, || v);
        }
    }
}

#[derive(Debug, Clone)]
pub struct LexicographicExtent {
    extent: Option<(String, String)>,
}

impl LexicographicExtent {
    pub fn new() -> Self {
        Self { extent: None }
    }

    pub fn clear(&mut self) {
        self.extent = None;
    }

    pub fn add(&mut self, value: &str) {
        match &mut self.extent {
            None => self.extent = Some((value.to_string(), value.to_string())),
            Some((min, max)) => {
                if value < min.as_str() {
                    min.replace_range(.., value);
                }

                if value > max.as_str() {
                    max.replace_range(.., value);
                }
            }
        }
    }

    pub fn first(&self) -> Option<String> {
        self.extent.as_ref().map(|e| e.0.clone())
    }

    pub fn last(&self) -> Option<String> {
        self.extent.as_ref().map(|e| e.1.clone())
    }

    pub fn merge(&mut self, other: Self) {
        match self.extent.as_mut() {
            None => {
                self.extent = other.extent;
            }
            Some((min, max)) => {
                if let Some((other_min, other_max)) = other.extent {
                    if other_min < *min {
                        *min = other_min;
                    }
                    if other_max > *max {
                        *max = other_max;
                    }
                }
            }
        }
    }
}
