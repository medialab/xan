use std::cmp::{Ordering, Reverse};

use csv::ByteRecord;

use crate::collections::FixedReverseHeapMap;
use crate::moonblade::types::{DynamicNumber, DynamicValue};

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
                } else if value > *max {
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

type ArgExtentEntry = (DynamicNumber, (usize, ByteRecord, Option<DynamicValue>));

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

    pub fn add(
        &mut self,
        index: usize,
        value: DynamicNumber,
        record: &ByteRecord,
        last_value: &Option<DynamicValue>,
    ) {
        match &mut self.extent {
            None => {
                self.extent = Some((
                    (value, (index, record.clone(), last_value.clone())),
                    (value, (index, record.clone(), last_value.clone())),
                ))
            }
            Some(((min, min_arg), (max, max_arg))) => {
                match value.partial_cmp(min).unwrap() {
                    Ordering::Equal => {
                        if min_arg.0 > index {
                            *min_arg = (index, record.clone(), last_value.clone());
                        }
                    }
                    Ordering::Less => {
                        *min = value;
                        *min_arg = (index, record.clone(), last_value.clone());
                    }
                    Ordering::Greater => match value.partial_cmp(max).unwrap() {
                        Ordering::Equal => {
                            if min_arg.0 > index {
                                *min_arg = (index, record.clone(), last_value.clone());
                            }
                        }
                        Ordering::Greater => {
                            *max = value;
                            *max_arg = (index, record.clone(), last_value.clone());
                        }
                        _ => (),
                    },
                };
            }
        }
    }

    pub fn min(&self) -> Option<DynamicNumber> {
        self.extent.as_ref().map(|e| e.0 .0)
    }

    pub fn max(&self) -> Option<DynamicNumber> {
        self.extent.as_ref().map(|e| e.1 .0)
    }

    pub fn argmin(&self) -> Option<&(usize, ByteRecord, Option<DynamicValue>)> {
        self.extent.as_ref().map(|e| &e.0 .1)
    }

    pub fn argmax(&self) -> Option<&(usize, ByteRecord, Option<DynamicValue>)> {
        self.extent.as_ref().map(|e| &e.1 .1)
    }

    pub fn merge(&mut self, other: Self) {
        if let Some(((min, arg_min), (max, arg_max))) = other.extent {
            self.add(arg_min.0, min, &arg_min.1, &arg_min.2);
            self.add(arg_max.0, max, &arg_max.1, &arg_max.2);
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArgTop {
    heap: FixedReverseHeapMap<(DynamicNumber, Reverse<usize>), (ByteRecord, Option<DynamicValue>)>,
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

    pub fn add(
        &mut self,
        index: usize,
        value: DynamicNumber,
        record: &ByteRecord,
        last_value: &Option<DynamicValue>,
    ) {
        self.heap.push_with((value, Reverse(index)), || {
            (record.clone(), last_value.clone())
        });
    }

    pub fn top_indices(&self) -> impl Iterator<Item = usize> {
        self.heap
            .to_sorted_vec()
            .into_iter()
            .map(|((_, Reverse(i)), _)| i)
    }

    pub fn top_records(&self) -> impl Iterator<Item = (usize, ByteRecord, Option<DynamicValue>)> {
        self.heap
            .to_sorted_vec()
            .into_iter()
            .map(|((_, Reverse(i)), r)| (i, r.0, r.1))
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
                } else if value > max.as_str() {
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
