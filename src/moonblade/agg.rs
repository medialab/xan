use std::cmp::{Ordering, Reverse};
use std::collections::HashMap;

use csv::ByteRecord;
use jiff::civil::DateTime;
use rayon::prelude::*;

use crate::collections::{FixedReverseHeap, FixedReverseHeapMap, SortedInsertHashmap};

use super::error::{ConcretizationError, EvaluationError, InvalidArity, SpecifiedEvaluationError};
use super::interpreter::{concretize_expression, eval_expression, ConcreteExpr, EvaluationContext};
use super::parser::{parse_aggregations, Aggregation, Aggregations};
use super::types::{Arity, DynamicNumber, DynamicValue};

#[derive(Debug, Clone)]
enum CountType {
    Empty,
    NonEmpty,
}

#[derive(Debug, Clone)]
struct Count {
    non_empty: usize,
    empty: usize,
}

impl Count {
    fn new() -> Self {
        Self {
            non_empty: 0,
            empty: 0,
        }
    }

    fn clear(&mut self) {
        self.non_empty = 0;
        self.empty = 0;
    }

    fn add_non_empty(&mut self) {
        self.non_empty += 1;
    }

    fn add_empty(&mut self) {
        self.empty += 1
    }

    fn get_non_empty(&self) -> usize {
        self.non_empty
    }

    fn get_empty(&self) -> usize {
        self.empty
    }

    fn get(&self, t: &CountType) -> usize {
        match t {
            CountType::Empty => self.get_empty(),
            CountType::NonEmpty => self.get_non_empty(),
        }
    }

    fn merge(&mut self, other: Self) {
        self.non_empty += other.non_empty;
        self.empty += other.empty;
    }
}

#[derive(Debug, Clone)]
struct AllAny {
    all: bool,
    any: bool,
}

impl AllAny {
    fn new() -> Self {
        Self {
            all: true,
            any: false,
        }
    }

    fn clear(&mut self) {
        self.all = true;
        self.any = false;
    }

    fn add(&mut self, new_bool: bool) {
        self.all = self.all && new_bool;
        self.any = self.any || new_bool;
    }

    fn all(&self) -> bool {
        self.all
    }

    fn any(&self) -> bool {
        self.any
    }

    fn merge(&mut self, other: Self) {
        self.all = self.all && other.all;
        self.any = self.any || other.any;
    }
}

// NOTE: I am splitting first and last because first can be more efficient
// This is typically not the case for extents where the amount of copying
// is mostly arbitrary
#[derive(Debug, Clone)]
struct First {
    item: Option<(usize, DynamicValue)>,
}

impl First {
    fn new() -> Self {
        Self { item: None }
    }

    fn clear(&mut self) {
        self.item = None;
    }

    fn add(&mut self, index: usize, next_value: &DynamicValue) {
        if self.item.is_none() {
            self.item = Some((index, next_value.clone()));
        }
    }

    fn first(&self) -> Option<DynamicValue> {
        self.item.as_ref().map(|p| p.1.clone())
    }

    fn merge(&mut self, other: Self) {
        match self.item.as_ref() {
            None => self.item = other.item,
            Some((i, _)) => {
                if let Some((j, _)) = other.item.as_ref() {
                    if i > j {
                        self.item = other.item;
                    }
                }
            }
        };
    }
}

#[derive(Debug, Clone)]
struct Last {
    item: Option<(usize, DynamicValue)>,
}

impl Last {
    fn new() -> Self {
        Self { item: None }
    }

    fn clear(&mut self) {
        self.item = None;
    }

    fn add(&mut self, index: usize, next_value: &DynamicValue) {
        self.item = Some((index, next_value.clone()));
    }

    fn last(&self) -> Option<DynamicValue> {
        self.item.as_ref().map(|p| p.1.clone())
    }

    fn merge(&mut self, other: Self) {
        match self.item.as_ref() {
            None => self.item = other.item,
            Some((i, _)) => {
                if let Some((j, _)) = other.item.as_ref() {
                    if i < j {
                        self.item = other.item;
                    }
                }
            }
        };
    }
}

#[derive(Debug, Clone)]
struct Sum {
    current: Option<DynamicNumber>,
}

impl Sum {
    fn new() -> Self {
        Self { current: None }
    }

    fn clear(&mut self) {
        self.current = None;
    }

    fn add(&mut self, value: DynamicNumber) {
        match self.current.as_mut() {
            None => self.current = Some(value),
            Some(current_sum) => {
                match current_sum {
                    DynamicNumber::Float(a) => match value {
                        DynamicNumber::Float(b) => *a += b,
                        DynamicNumber::Integer(b) => *a += b as f64,
                    },
                    DynamicNumber::Integer(a) => match value {
                        DynamicNumber::Float(b) => {
                            self.current = Some(DynamicNumber::Float((*a as f64) + b));
                        }
                        DynamicNumber::Integer(b) => *a += b,
                    },
                };
            }
        };
    }

    fn get(&self) -> Option<DynamicNumber> {
        self.current
    }

    fn merge(&mut self, other: Self) {
        if let Some(other_sum) = other.current {
            self.add(other_sum);
        }
    }
}

#[derive(Debug, Clone)]
struct GenericExtent<T: Copy + PartialOrd> {
    extent: Option<(T, T)>,
}

impl<T: Copy + PartialOrd> GenericExtent<T> {
    fn new() -> Self {
        Self { extent: None }
    }

    fn clear(&mut self) {
        self.extent = None;
    }

    fn add(&mut self, value: T) {
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

    fn min(&self) -> Option<T> {
        self.extent.map(|e| e.0)
    }

    fn max(&self) -> Option<T> {
        self.extent.map(|e| e.1)
    }

    fn merge(&mut self, other: Self) {
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

type Extent = GenericExtent<DynamicNumber>;

type ArgExtentEntry = (DynamicNumber, (usize, ByteRecord));

#[derive(Debug, Clone)]
struct ArgExtent {
    extent: Option<(ArgExtentEntry, ArgExtentEntry)>,
}

impl ArgExtent {
    fn new() -> Self {
        Self { extent: None }
    }

    fn clear(&mut self) {
        self.extent = None;
    }

    fn add(&mut self, index: usize, value: DynamicNumber, arg: &ByteRecord) {
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

    fn min(&self) -> Option<DynamicNumber> {
        self.extent.as_ref().map(|e| e.0 .0)
    }

    fn max(&self) -> Option<DynamicNumber> {
        self.extent.as_ref().map(|e| e.1 .0)
    }

    fn argmin(&self) -> Option<&(usize, ByteRecord)> {
        self.extent.as_ref().map(|e| &e.0 .1)
    }

    fn argmax(&self) -> Option<&(usize, ByteRecord)> {
        self.extent.as_ref().map(|e| &e.1 .1)
    }

    fn merge(&mut self, other: Self) {
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
struct ArgTop {
    heap: FixedReverseHeapMap<(DynamicNumber, Reverse<usize>), ByteRecord>,
}

impl ArgTop {
    fn new(k: usize) -> Self {
        Self {
            heap: FixedReverseHeapMap::with_capacity(k),
        }
    }

    fn capacity(&self) -> usize {
        self.heap.capacity()
    }

    fn clear(&mut self) {
        self.heap.clear();
    }

    fn add(&mut self, index: usize, value: DynamicNumber, arg: &ByteRecord) {
        self.heap.push_with((value, Reverse(index)), || arg.clone());
    }

    fn top_indices(&self) -> impl Iterator<Item = usize> {
        self.heap
            .to_sorted_vec()
            .into_iter()
            .map(|((_, Reverse(i)), _)| i)
    }

    fn top_records(&self) -> impl Iterator<Item = (usize, ByteRecord)> {
        self.heap
            .to_sorted_vec()
            .into_iter()
            .map(|((_, Reverse(i)), r)| (i, r))
    }

    fn top_values(&self) -> impl Iterator<Item = DynamicNumber> {
        self.heap.to_sorted_vec().into_iter().map(|((v, _), _)| v)
    }

    fn merge(&mut self, other: Self) {
        for (k, v) in other.heap.into_unordered_iter() {
            self.heap.push_with(k, || v);
        }
    }
}

#[derive(Debug, Clone)]
struct LexicographicExtent {
    extent: Option<(String, String)>,
}

impl LexicographicExtent {
    fn new() -> Self {
        Self { extent: None }
    }

    fn clear(&mut self) {
        self.extent = None;
    }

    fn add(&mut self, value: &str) {
        match &mut self.extent {
            None => self.extent = Some((value.to_string(), value.to_string())),
            Some((min, max)) => {
                if value < min.as_str() {
                    *min = value.to_string();
                }

                if value > max.as_str() {
                    *max = value.to_string();
                }
            }
        }
    }

    fn first(&self) -> Option<String> {
        self.extent.as_ref().map(|e| e.0.clone())
    }

    fn last(&self) -> Option<String> {
        self.extent.as_ref().map(|e| e.1.clone())
    }

    fn merge(&mut self, other: Self) {
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

#[derive(Debug, Clone)]
enum MedianType {
    Interpolation,
    Low,
    High,
}

#[derive(Debug, Clone)]
struct Numbers {
    numbers: Vec<DynamicNumber>,
}

impl Numbers {
    fn new() -> Self {
        Self {
            numbers: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.numbers.clear();
    }

    fn add(&mut self, number: DynamicNumber) {
        self.numbers.push(number);
    }

    fn finalize(&mut self, parallel: bool) {
        let cmp = |a: &DynamicNumber, b: &DynamicNumber| a.partial_cmp(b).unwrap();

        if parallel {
            self.numbers.par_sort_unstable_by(cmp);
        } else {
            self.numbers.sort_unstable_by(cmp);
        }
    }

    fn median(&self, median_type: &MedianType) -> Option<DynamicNumber> {
        let count = self.numbers.len();

        if count == 0 {
            return None;
        }

        let median = match median_type {
            MedianType::Low => {
                let mut midpoint = count / 2;

                if count % 2 == 0 {
                    midpoint -= 1;
                }

                self.numbers[midpoint]
            }
            MedianType::High => {
                let midpoint = count / 2;

                self.numbers[midpoint]
            }
            MedianType::Interpolation => {
                let midpoint = count / 2;

                if count % 2 == 1 {
                    self.numbers[midpoint]
                } else {
                    let down = &self.numbers[midpoint - 1];
                    let up = &self.numbers[midpoint];

                    (*down + *up) / DynamicNumber::Float(2.0)
                }
            }
        };

        Some(median)
    }

    // NOTE: using the inclusive method from https://github.com/python/cpython/blob/3.12/Lib/statistics.py
    fn quantiles(&self, n: usize) -> Option<Vec<DynamicNumber>> {
        let l = self.numbers.len();

        if l < 2 {
            return None;
        }

        let mut result: Vec<DynamicNumber> = Vec::new();

        let m = l - 1;

        for i in 1..n {
            let c = i * m;
            let j = c.div_euclid(n);
            let delta = c.rem_euclid(n);

            let interpolated = (self.numbers[j] * DynamicNumber::Integer((n - delta) as i64)
                + self.numbers[j + 1] * DynamicNumber::Integer(delta as i64))
                / DynamicNumber::Integer(n as i64);

            result.push(interpolated);
        }

        Some(result)
    }

    fn quartiles(&self) -> Option<Vec<DynamicNumber>> {
        self.quantiles(4)
    }

    // NOTE: from https://github.com/simple-statistics/simple-statistics/blob/main/src/quantile_sorted.js
    fn quantile(&self, p: f64) -> Option<DynamicNumber> {
        let n = &self.numbers;
        let l = n.len();

        if !(0.0..=1.0).contains(&p) {
            None
        } else if p == 1.0 {
            Some(n[l - 1])
        } else if p == 0.0 {
            Some(n[0])
        } else {
            let idx = (l as f64) * p;

            if idx.fract() != 0.0 {
                Some(n[idx.ceil() as usize - 1])
            } else {
                let idx = idx.floor() as usize;

                if l % 2 == 0 {
                    Some((n[idx - 1] + n[idx]) / DynamicNumber::Integer(2))
                } else {
                    Some(n[idx])
                }
            }
        }
    }

    fn merge(&mut self, other: Self) {
        self.numbers.extend(other.numbers);
    }
}

#[derive(Debug, Clone)]
struct Frequencies {
    counter: HashMap<String, u64>,
}

impl Frequencies {
    fn new() -> Self {
        Self {
            counter: HashMap::new(),
        }
    }

    fn clear(&mut self) {
        self.counter.clear();
    }

    fn add_count(&mut self, value: String, count: u64) {
        self.counter
            .entry(value)
            .and_modify(|current| *current += count)
            .or_insert(count);
    }

    fn add(&mut self, value: String) {
        self.add_count(value, 1);
    }

    fn mode(&self) -> Option<String> {
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

    fn modes(&self) -> Option<Vec<String>> {
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

    fn most_common(&self, k: usize) -> Vec<String> {
        let mut heap = FixedReverseHeap::<(u64, Reverse<&String>)>::with_capacity(k);

        for (key, count) in self.counter.iter() {
            heap.push((*count, Reverse(key)));
        }

        heap.into_sorted_vec()
            .into_iter()
            .map(|(_, Reverse(value))| value.clone())
            .collect()
    }

    fn most_common_counts(&self, k: usize) -> Vec<u64> {
        let mut heap = FixedReverseHeap::<(u64, Reverse<&String>)>::with_capacity(k);

        for (key, count) in self.counter.iter() {
            heap.push((*count, Reverse(key)));
        }

        heap.into_sorted_vec()
            .into_iter()
            .map(|(count, _)| count)
            .collect()
    }

    fn cardinality(&self) -> usize {
        self.counter.len()
    }

    fn join(&self, separator: &str) -> String {
        let mut keys: Vec<_> = self.counter.keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        keys.join(separator)
    }

    fn merge(&mut self, other: Self) {
        for (key, count) in other.counter {
            self.add_count(key, count);
        }
    }
}

// NOTE: this is an implementation of Welford's online algorithm
// Ref: https://en.wikipedia.org/wiki/Algorithms_for_calculating_variance
// Ref: https://en.wikipedia.org/wiki/Standard_deviation
#[derive(Debug, Clone)]
pub struct Welford {
    count: usize,
    mean: f64,
    m2: f64,
}

impl Welford {
    pub fn new() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
        }
    }

    fn clear(&mut self) {
        self.count = 0;
        self.mean = 0.0;
        self.m2 = 0.0;
    }

    pub fn add(&mut self, value: f64) {
        let (mut count, mut mean, mut m2) = (self.count, self.mean, self.m2);
        count += 1;
        let delta = value - mean;
        mean += delta / count as f64;
        let delta2 = value - mean;
        m2 += delta * delta2;

        self.count = count;
        self.mean = mean;
        self.m2 = m2;
    }

    pub fn mean(&self) -> Option<f64> {
        if self.count == 0 {
            return None;
        }

        Some(self.mean)
    }

    fn variance(&self) -> Option<f64> {
        if self.count < 1 {
            return None;
        }

        Some(self.m2 / self.count as f64)
    }

    fn sample_variance(&self) -> Option<f64> {
        if self.count < 2 {
            return None;
        }

        Some(self.m2 / (self.count - 1) as f64)
    }

    fn stdev(&self) -> Option<f64> {
        self.variance().map(|v| v.sqrt())
    }

    fn sample_stdev(&self) -> Option<f64> {
        self.sample_variance().map(|v| v.sqrt())
    }

    fn merge(&mut self, other: Self) {
        self.count += other.count;

        let count1 = self.count as f64;
        let count2 = self.count as f64;

        let total = count1 + count2;

        let mean_diff_squared = (self.mean - other.mean).powi(2);
        self.mean = ((count1 * self.mean) + (count2 * other.mean)) / total;

        self.m2 = (((count1 * self.m2) + (count2 * other.m2)) / total)
            + ((count1 * count2 * mean_diff_squared) / (total * total));
    }
}

#[derive(Debug, Clone)]
struct Values {
    values: Vec<String>,
}

impl Values {
    fn new() -> Self {
        Self { values: Vec::new() }
    }

    fn clear(&mut self) {
        self.values.clear()
    }

    fn add(&mut self, string: String) {
        self.values.push(string);
    }

    fn join(&self, separator: &str) -> String {
        self.values.join(separator)
    }

    fn merge(&mut self, other: Self) {
        self.values.extend(other.values);
    }
}

const TYPE_EMPTY: u8 = 0;
const TYPE_STRING: u8 = 1;
const TYPE_FLOAT: u8 = 2;
const TYPE_INT: u8 = 3;
const TYPE_DATE: u8 = 4;
const TYPE_URL: u8 = 5;

#[derive(Debug, Clone)]
struct Types {
    bitset: u8,
}

impl Types {
    fn new() -> Self {
        Self { bitset: 0 }
    }

    fn set(&mut self, pos: u8) {
        self.bitset |= 1 << pos;
    }

    fn set_empty(&mut self) {
        self.set(TYPE_EMPTY);
    }

    fn set_string(&mut self) {
        self.set(TYPE_STRING);
    }

    fn set_float(&mut self) {
        self.set(TYPE_FLOAT);
    }

    fn set_int(&mut self) {
        self.set(TYPE_INT);
    }

    fn set_date(&mut self) {
        self.set(TYPE_DATE);
    }

    fn set_url(&mut self) {
        self.set(TYPE_URL);
    }

    fn has(&self, pos: u8) -> bool {
        ((self.bitset >> pos) & 1) == 1
    }

    fn has_empty(&self) -> bool {
        self.has(TYPE_EMPTY)
    }

    fn has_string(&self) -> bool {
        self.has(TYPE_STRING)
    }

    fn has_float(&self) -> bool {
        self.has(TYPE_FLOAT)
    }

    fn has_int(&self) -> bool {
        self.has(TYPE_INT)
    }

    fn has_date(&self) -> bool {
        self.has(TYPE_DATE)
    }

    fn has_url(&self) -> bool {
        self.has(TYPE_URL)
    }

    fn most_likely_type(&self) -> Option<&str> {
        Some(if self.has_string() {
            "string"
        } else if self.has_float() {
            "float"
        } else if self.has_int() {
            "int"
        } else if self.has_url() {
            "url"
        } else if self.has_date() {
            "date"
        } else if self.has_empty() {
            "empty"
        } else {
            return None;
        })
    }

    fn sorted_types(&self) -> Vec<&str> {
        let mut result = Vec::new();

        if self.has_int() {
            result.push("int");
        }
        if self.has_float() {
            result.push("float");
        }
        if self.has_string() {
            result.push("string");
        }
        if self.has_date() {
            result.push("date");
        }
        if self.has_url() {
            result.push("url");
        }
        if self.has_empty() {
            result.push("empty");
        }

        result
    }

    fn clear(&mut self) {
        self.bitset = 0;
        self.set_empty();
    }

    fn merge(&mut self, other: Self) {
        self.bitset |= other.bitset;
    }
}

macro_rules! build_aggregation_method_enum {
    ($($variant: ident,)+) => {
        #[derive(Debug, Clone)]
        enum Aggregator {
            $(
                $variant($variant),
            )+
        }

        impl Aggregator {
            fn clear(&mut self) {
                match self {
                    $(
                        Self::$variant(inner) => inner.clear(),
                    )+
                };
            }

            fn merge(&mut self, other: Self) {
                match (self, other) {
                    $(
                        (Self::$variant(inner), Self::$variant(inner_other)) => inner.merge(inner_other),
                    )+
                    _ => unreachable!(),
                };
            }

            fn finalize(&mut self, parallel: bool) {
                match self {
                    Self::Numbers(inner) => {
                        inner.finalize(parallel);
                    }
                    _ => (),
                }
            }
        }
    };
}

build_aggregation_method_enum!(
    AllAny,
    ArgExtent,
    ArgTop,
    Count,
    Extent,
    First,
    Last,
    Values,
    LexicographicExtent,
    Frequencies,
    Numbers,
    Sum,
    Types,
    Welford,
);

impl Aggregator {
    fn get_final_value(
        &self,
        method: &ConcreteAggregationMethod,
        context: &EvaluationContext,
    ) -> Result<DynamicValue, SpecifiedEvaluationError> {
        Ok(match (method, self) {
            (ConcreteAggregationMethod::All, Self::AllAny(inner)) => {
                DynamicValue::from(inner.all())
            }
            (ConcreteAggregationMethod::Any, Self::AllAny(inner)) => {
                DynamicValue::from(inner.any())
            }
            (ConcreteAggregationMethod::ArgTop(_, expr_opt, separator), Self::ArgTop(inner)) => {
                DynamicValue::from(match expr_opt {
                    None => inner
                        .top_indices()
                        .map(|i| i.to_string())
                        .collect::<Vec<_>>()
                        .join(separator),
                    Some(expr) => {
                        let mut strings = Vec::new();

                        for (index, record) in inner.top_records() {
                            let value = eval_expression(expr, Some(index), &record, context)?;

                            strings.push(
                                value
                                    .try_as_str()
                                    .map_err(|err| SpecifiedEvaluationError {
                                        function_name: "argtop".to_string(),
                                        reason: err,
                                    })?
                                    .into_owned(),
                            );
                        }

                        strings.join(separator)
                    }
                })
            }
            (ConcreteAggregationMethod::Cardinality, Self::Frequencies(inner)) => {
                DynamicValue::from(inner.cardinality())
            }
            (ConcreteAggregationMethod::Count(count_type), Self::Count(inner)) => {
                DynamicValue::from(inner.get(count_type))
            }
            (ConcreteAggregationMethod::DistinctValues(separator), Self::Frequencies(inner)) => {
                DynamicValue::from(inner.join(separator))
            }
            (ConcreteAggregationMethod::First, Self::First(inner)) => {
                DynamicValue::from(inner.first())
            }
            (ConcreteAggregationMethod::Last, Self::Last(inner)) => {
                DynamicValue::from(inner.last())
            }
            (ConcreteAggregationMethod::LexFirst, Self::LexicographicExtent(inner)) => {
                DynamicValue::from(inner.first())
            }
            (ConcreteAggregationMethod::LexLast, Self::LexicographicExtent(inner)) => {
                DynamicValue::from(inner.last())
            }
            (ConcreteAggregationMethod::Min, Self::Extent(inner)) => {
                DynamicValue::from(inner.min())
            }
            (ConcreteAggregationMethod::Min, Self::ArgExtent(inner)) => {
                DynamicValue::from(inner.min())
            }
            (ConcreteAggregationMethod::ArgMin(expr_opt), Self::ArgExtent(inner)) => {
                if let Some((index, record)) = inner.argmin() {
                    match expr_opt {
                        None => DynamicValue::from(*index),
                        Some(expr) => return eval_expression(expr, Some(*index), record, context),
                    }
                } else {
                    DynamicValue::None
                }
            }
            (ConcreteAggregationMethod::Mean, Self::Welford(inner)) => {
                DynamicValue::from(inner.mean())
            }
            (ConcreteAggregationMethod::Median(median_type), Self::Numbers(inner)) => {
                DynamicValue::from(inner.median(median_type))
            }
            (ConcreteAggregationMethod::Quantile(p), Self::Numbers(inner)) => {
                DynamicValue::from(inner.quantile(*p))
            }
            (ConcreteAggregationMethod::Quartile(idx), Self::Numbers(inner)) => {
                DynamicValue::from(inner.quartiles().map(|q| q[*idx]))
            }
            (ConcreteAggregationMethod::Max, Self::Extent(inner)) => {
                DynamicValue::from(inner.max())
            }
            (ConcreteAggregationMethod::Max, Self::ArgExtent(inner)) => {
                DynamicValue::from(inner.max())
            }
            (ConcreteAggregationMethod::ArgMax(expr_opt), Self::ArgExtent(inner)) => {
                if let Some((index, record)) = inner.argmax() {
                    match expr_opt {
                        None => DynamicValue::from(*index),
                        Some(expr) => return eval_expression(expr, Some(*index), record, context),
                    }
                } else {
                    DynamicValue::None
                }
            }
            (ConcreteAggregationMethod::Mode, Self::Frequencies(inner)) => {
                DynamicValue::from(inner.mode())
            }
            (ConcreteAggregationMethod::Modes(separator), Self::Frequencies(inner)) => {
                DynamicValue::from(inner.modes().map(|m| m.join(separator)))
            }
            (
                ConcreteAggregationMethod::MostCommonCounts(k, separator),
                Self::Frequencies(inner),
            ) => DynamicValue::from(
                inner
                    .most_common_counts(*k)
                    .into_iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(separator),
            ),
            (
                ConcreteAggregationMethod::MostCommonValues(k, separator),
                Self::Frequencies(inner),
            ) => DynamicValue::from(inner.most_common(*k).join(separator)),
            (ConcreteAggregationMethod::Sum, Self::Sum(inner)) => DynamicValue::from(inner.get()),
            (ConcreteAggregationMethod::VarPop, Self::Welford(inner)) => {
                DynamicValue::from(inner.variance())
            }
            (ConcreteAggregationMethod::VarSample, Self::Welford(inner)) => {
                DynamicValue::from(inner.sample_variance())
            }
            (ConcreteAggregationMethod::StddevPop, Self::Welford(inner)) => {
                DynamicValue::from(inner.stdev())
            }
            (ConcreteAggregationMethod::StddevSample, Self::Welford(inner)) => {
                DynamicValue::from(inner.sample_stdev())
            }
            (ConcreteAggregationMethod::Top(_, separator), Self::ArgTop(inner)) => {
                DynamicValue::from(
                    inner
                        .top_values()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(separator),
                )
            }
            (ConcreteAggregationMethod::Types, Self::Types(inner)) => {
                DynamicValue::from(inner.sorted_types().join("|"))
            }
            (ConcreteAggregationMethod::Type, Self::Types(inner)) => {
                DynamicValue::from(inner.most_likely_type())
            }
            (ConcreteAggregationMethod::Values(separator), Self::Values(inner)) => {
                DynamicValue::from(inner.join(separator))
            }
            _ => unreachable!(),
        })
    }
}

// NOTE: at the beginning I was using a struct that would look like this:
// struct Aggregator {
//     count: Option<Count>,
//     sum: Option<Sum>,
// }

// But this has the downside of allocating a lot of memory for each Aggregator
// instances, and since we need to instantiate one Aggregator per group when
// aggregating per group, this would cost quite a lot of memory for no good
// reason. We can of course store a list of CSV rows per group but this would
// also cost O(n) memory (n being the size of target CSV file), whereas we
// actually only need O(1) memory per group, i.e. O(g) for most aggregation
// methods (e.g. sum, mean etc.).

// Note that we can wrap the inner aggregators in a Box to reduce the memory
// footprint. But this will still increase each time we add a new aggregation
// function family, which is far from ideal.

// The current solution relies on an enum of aggregation method `AggregationMethod`
// and an `Aggregator` struct which is basically wrapping only a vector of
// said enum, making it as light as possible. This is somewhat verbose however
// and we could rely on macros to help with this if needed.

// NOTE: this aggregator actively combines and matches different generic
// aggregation schemes and never repeats itself. For instance, mean will be
// inferred from aggregating sum and count. Also if the user asks for both
// sum and mean, the sum will only be aggregated once.

#[derive(Debug, Clone)]
struct CompositeAggregator {
    methods: Vec<Aggregator>,
}

impl CompositeAggregator {
    fn new() -> Self {
        Self {
            methods: Vec::new(),
        }
    }

    fn clear(&mut self) {
        for method in self.methods.iter_mut() {
            method.clear();
        }
    }

    fn merge(&mut self, other: Self) {
        for (self_method, other_method) in self.methods.iter_mut().zip(other.methods) {
            self_method.merge(other_method);
        }
    }

    fn add_method(&mut self, method: &ConcreteAggregationMethod) -> usize {
        macro_rules! upsert_aggregator {
            ($variant: ident) => {
                match self
                    .methods
                    .iter()
                    .position(|item| matches!(item, Aggregator::$variant(_)))
                {
                    Some(idx) => idx,
                    None => {
                        let idx = self.methods.len();
                        self.methods.push(Aggregator::$variant($variant::new()));
                        idx
                    }
                }
            };
        }

        match method {
            ConcreteAggregationMethod::All | ConcreteAggregationMethod::Any => {
                upsert_aggregator!(AllAny)
            }
            ConcreteAggregationMethod::Count(_) => {
                upsert_aggregator!(Count)
            }
            ConcreteAggregationMethod::Min | ConcreteAggregationMethod::Max => {
                // NOTE: if some ArgExtent already exists, we merge into it.
                match self
                    .methods
                    .iter()
                    .position(|item| matches!(item, Aggregator::ArgExtent(_)))
                {
                    None => upsert_aggregator!(Extent),
                    Some(idx) => idx,
                }
            }
            ConcreteAggregationMethod::ArgMin(_) | ConcreteAggregationMethod::ArgMax(_) => {
                // NOTE: if some Extent exist, we replace it
                match self
                    .methods
                    .iter()
                    .position(|item| matches!(item, Aggregator::Extent(_)))
                {
                    None => upsert_aggregator!(ArgExtent),
                    Some(idx) => {
                        self.methods[idx] = Aggregator::ArgExtent(ArgExtent::new());
                        idx
                    }
                }
            }
            ConcreteAggregationMethod::ArgTop(k, _, _) | ConcreteAggregationMethod::Top(k, _) => {
                match self.methods.iter().position(
                    |item| matches!(item, Aggregator::ArgTop(inner) if inner.capacity() == *k),
                ) {
                    None => {
                        let idx = self.methods.len();
                        self.methods.push(Aggregator::ArgTop(ArgTop::new(*k)));
                        idx
                    }
                    Some(idx) => idx,
                }
            }
            ConcreteAggregationMethod::First => {
                upsert_aggregator!(First)
            }
            ConcreteAggregationMethod::Last => {
                upsert_aggregator!(Last)
            }
            ConcreteAggregationMethod::LexFirst | ConcreteAggregationMethod::LexLast => {
                upsert_aggregator!(LexicographicExtent)
            }
            ConcreteAggregationMethod::Median(_)
            | ConcreteAggregationMethod::Quantile(_)
            | ConcreteAggregationMethod::Quartile(_) => {
                upsert_aggregator!(Numbers)
            }
            ConcreteAggregationMethod::Mode
            | ConcreteAggregationMethod::Modes(_)
            | ConcreteAggregationMethod::Cardinality
            | ConcreteAggregationMethod::DistinctValues(_)
            | ConcreteAggregationMethod::MostCommonCounts(_, _)
            | ConcreteAggregationMethod::MostCommonValues(_, _) => {
                upsert_aggregator!(Frequencies)
            }
            ConcreteAggregationMethod::Sum => {
                upsert_aggregator!(Sum)
            }
            ConcreteAggregationMethod::Mean
            | ConcreteAggregationMethod::VarPop
            | ConcreteAggregationMethod::VarSample
            | ConcreteAggregationMethod::StddevPop
            | ConcreteAggregationMethod::StddevSample => {
                upsert_aggregator!(Welford)
            }
            ConcreteAggregationMethod::Types | ConcreteAggregationMethod::Type => {
                upsert_aggregator!(Types)
            }
            ConcreteAggregationMethod::Values(_) => {
                upsert_aggregator!(Values)
            }
        }
    }

    fn process_value(
        &mut self,
        index: usize,
        value_opt: Option<DynamicValue>,
        record: &ByteRecord,
    ) -> Result<(), EvaluationError> {
        for method in self.methods.iter_mut() {
            match value_opt.as_ref() {
                Some(value) => match method {
                    Aggregator::AllAny(allany) => {
                        allany.add(value.is_truthy());
                    }
                    Aggregator::Count(count) => {
                        if !value.is_nullish() {
                            count.add_non_empty();
                        } else {
                            count.add_empty();
                        }
                    }
                    Aggregator::Extent(extent) => {
                        if !value.is_nullish() {
                            extent.add(value.try_as_number()?);
                        }
                    }
                    Aggregator::ArgExtent(extent) => {
                        if !value.is_nullish() {
                            extent.add(index, value.try_as_number()?, record);
                        }
                    }
                    Aggregator::ArgTop(top) => {
                        if !value.is_nullish() {
                            top.add(index, value.try_as_number()?, record);
                        }
                    }
                    Aggregator::First(first) => {
                        if !value.is_nullish() {
                            first.add(index, value);
                        }
                    }
                    Aggregator::Last(last) => {
                        if !value.is_nullish() {
                            last.add(index, value);
                        }
                    }
                    Aggregator::LexicographicExtent(extent) => {
                        if !value.is_nullish() {
                            extent.add(&value.try_as_str()?);
                        }
                    }
                    Aggregator::Frequencies(frequencies) => {
                        if !value.is_nullish() {
                            frequencies.add(value.try_as_str()?.into_owned());
                        }
                    }
                    Aggregator::Numbers(numbers) => {
                        if !value.is_nullish() {
                            numbers.add(value.try_as_number()?);
                        }
                    }
                    Aggregator::Sum(sum) => {
                        if !value.is_nullish() {
                            sum.add(value.try_as_number()?);
                        }
                    }
                    Aggregator::Welford(variance) => {
                        if !value.is_nullish() {
                            variance.add(value.try_as_f64()?);
                        }
                    }
                    Aggregator::Types(types) => {
                        if value.is_nullish() {
                            types.set_empty();
                        } else if let Ok(n) = value.try_as_number() {
                            match n {
                                DynamicNumber::Float(_) => types.set_float(),
                                DynamicNumber::Integer(_) => types.set_int(),
                            };
                        } else {
                            match value.try_as_str() {
                                Ok(s) if s.parse::<DateTime>().is_ok() => types.set_date(),
                                Ok(s) if s.starts_with("http://") || s.starts_with("https://") => {
                                    types.set_url()
                                }
                                _ => types.set_string(),
                            };
                        }
                    }
                    Aggregator::Values(values) => {
                        if !value.is_nullish() {
                            values.add(value.try_as_str()?.into_owned());
                        }
                    }
                },
                None => match method {
                    Aggregator::Count(count) => {
                        count.add_non_empty();
                    }
                    _ => unreachable!(),
                },
            }
        }

        Ok(())
    }

    fn finalize(&mut self, parallel: bool) {
        for method in self.methods.iter_mut() {
            method.finalize(parallel);
        }
    }

    fn get_final_value(
        &self,
        handle: usize,
        method: &ConcreteAggregationMethod,
        context: &EvaluationContext,
    ) -> Result<DynamicValue, SpecifiedEvaluationError> {
        self.methods[handle].get_final_value(method, context)
    }
}

fn validate_aggregation_function_arity(
    aggregation: &Aggregation,
) -> Result<(), ConcretizationError> {
    let arity = aggregation.args.len();

    let range = match aggregation.func_name.as_str() {
        "count" => 0..=1,
        "quantile" => 2..=2,
        "values" | "distinct_values" | "argmin" | "argmax" | "modes" => 1..=2,
        "most_common" | "most_common_counts" | "top" => 1..=3,
        "argtop" => 1..=4,
        _ => 1..=1,
    };

    if !range.contains(&arity) {
        return Err(ConcretizationError::InvalidArity(
            aggregation.func_name.clone(),
            InvalidArity::from_arity(Arity::Range(range), arity),
        ));
    }

    Ok(())
}

fn get_separator_from_argument(args: Vec<ConcreteExpr>, pos: usize) -> Option<String> {
    Some(match args.get(pos) {
        None => "|".to_string(),
        Some(arg) => match arg {
            ConcreteExpr::Value(separator) => separator.try_as_str().expect("").into_owned(),
            _ => return None,
        },
    })
}

#[derive(Debug, Clone)]
enum ConcreteAggregationMethod {
    All,
    Any,
    ArgMin(Option<ConcreteExpr>),
    ArgMax(Option<ConcreteExpr>),
    ArgTop(usize, Option<ConcreteExpr>, String),
    Cardinality,
    Count(CountType),
    DistinctValues(String),
    First,
    Last,
    LexFirst,
    LexLast,
    Min,
    Max,
    Mean,
    Median(MedianType),
    Mode,
    Modes(String),
    MostCommonValues(usize, String),
    MostCommonCounts(usize, String),
    Quartile(usize),
    Quantile(f64),
    Sum,
    Values(String),
    VarPop,
    VarSample,
    StddevPop,
    StddevSample,
    Top(usize, String),
    Type,
    Types,
}

impl ConcreteAggregationMethod {
    fn parse(name: &str, mut args: Vec<ConcreteExpr>) -> Result<Self, ConcretizationError> {
        Ok(match name {
            "all" => Self::All,
            "any" => Self::Any,
            "argmin" => Self::ArgMin(args.pop()),
            "argmax" => Self::ArgMax(args.pop()),
            "argtop" => Self::ArgTop(
                match args.first().unwrap() {
                    ConcreteExpr::Value(v) => match v.try_as_usize() {
                        Ok(k) => k,
                        Err(_) => return Err(ConcretizationError::NotStaticallyAnalyzable),
                    },
                    _ => return Err(ConcretizationError::NotStaticallyAnalyzable),
                },
                args.get(1).cloned(),
                match get_separator_from_argument(args, 2) {
                    None => return Err(ConcretizationError::NotStaticallyAnalyzable),
                    Some(separator) => separator,
                },
            ),
            "cardinality" => Self::Cardinality,
            "count" => Self::Count(CountType::NonEmpty),
            "count_empty" => Self::Count(CountType::Empty),
            "distinct_values" => Self::DistinctValues(match get_separator_from_argument(args, 0) {
                None => return Err(ConcretizationError::NotStaticallyAnalyzable),
                Some(separator) => separator,
            }),
            "first" => Self::First,
            "last" => Self::Last,
            "lex_first" => Self::LexFirst,
            "lex_last" => Self::LexLast,
            "min" => Self::Min,
            "max" => Self::Max,
            "avg" | "mean" => Self::Mean,
            "median" => Self::Median(MedianType::Interpolation),
            "median_high" => Self::Median(MedianType::High),
            "median_low" => Self::Median(MedianType::Low),
            "mode" => Self::Mode,
            "modes" => Self::Modes(match get_separator_from_argument(args, 0) {
                None => return Err(ConcretizationError::NotStaticallyAnalyzable),
                Some(separator) => separator,
            }),
            "most_common" => Self::MostCommonValues(
                match args.first().unwrap() {
                    ConcreteExpr::Value(v) => match v.try_as_usize() {
                        Ok(k) => k,
                        Err(_) => return Err(ConcretizationError::NotStaticallyAnalyzable),
                    },
                    _ => return Err(ConcretizationError::NotStaticallyAnalyzable),
                },
                match get_separator_from_argument(args, 1) {
                    None => return Err(ConcretizationError::NotStaticallyAnalyzable),
                    Some(separator) => separator,
                },
            ),
            "most_common_counts" => Self::MostCommonCounts(
                match args.first().unwrap() {
                    ConcreteExpr::Value(v) => match v.try_as_usize() {
                        Ok(k) => k,
                        Err(_) => return Err(ConcretizationError::NotStaticallyAnalyzable),
                    },
                    _ => return Err(ConcretizationError::NotStaticallyAnalyzable),
                },
                match get_separator_from_argument(args, 1) {
                    None => return Err(ConcretizationError::NotStaticallyAnalyzable),
                    Some(separator) => separator,
                },
            ),
            "quantile" => match args.first().unwrap() {
                ConcreteExpr::Value(v) => match v.try_as_f64() {
                    Ok(p) => Self::Quantile(p),
                    Err(_) => return Err(ConcretizationError::NotStaticallyAnalyzable),
                },
                _ => return Err(ConcretizationError::NotStaticallyAnalyzable),
            },
            "q1" => Self::Quartile(0),
            "q2" => Self::Quartile(1),
            "q3" => Self::Quartile(2),
            "values" => Self::Values(match get_separator_from_argument(args, 0) {
                None => return Err(ConcretizationError::NotStaticallyAnalyzable),
                Some(separator) => separator,
            }),
            "var" | "var_pop" => Self::VarPop,
            "var_sample" => Self::VarSample,
            "stddev" | "stddev_pop" => Self::StddevPop,
            "stddev_sample" => Self::StddevSample,
            "sum" => Self::Sum,
            "top" => Self::Top(
                match args.first().unwrap() {
                    ConcreteExpr::Value(v) => match v.try_as_usize() {
                        Ok(k) => k,
                        Err(_) => return Err(ConcretizationError::NotStaticallyAnalyzable),
                    },
                    _ => return Err(ConcretizationError::NotStaticallyAnalyzable),
                },
                match get_separator_from_argument(args, 1) {
                    None => return Err(ConcretizationError::NotStaticallyAnalyzable),
                    Some(separator) => separator,
                },
            ),
            "type" => Self::Type,
            "types" => Self::Types,
            _ => return Err(ConcretizationError::UnknownFunction(name.to_string())),
        })
    }
}

#[derive(Debug)]
struct ConcreteAggregation {
    agg_name: String,
    method: ConcreteAggregationMethod,
    expr: Option<ConcreteExpr>,
    expr_key: String,
    // args: Vec<ConcreteExpr>,
}

type ConcreteAggregations = Vec<ConcreteAggregation>;

fn concretize_aggregations(
    aggregations: Aggregations,
    headers: &ByteRecord,
) -> Result<ConcreteAggregations, ConcretizationError> {
    let mut concrete_aggregations = ConcreteAggregations::new();

    for mut aggregation in aggregations {
        validate_aggregation_function_arity(&aggregation)?;

        if ["most_common", "most_common_counts", "top", "argtop"]
            .contains(&aggregation.func_name.as_str())
        {
            aggregation.args.swap(0, 1);
        }

        let expr = aggregation
            .args
            .first()
            .map(|arg| concretize_expression(arg.clone(), headers))
            .transpose()?;

        let mut args: Vec<ConcreteExpr> = Vec::new();

        for arg in aggregation.args.into_iter().skip(1) {
            args.push(concretize_expression(arg, headers)?);
        }

        let method = ConcreteAggregationMethod::parse(&aggregation.func_name, args)?;

        let concrete_aggregation = ConcreteAggregation {
            agg_name: aggregation.agg_name,
            method,
            expr_key: aggregation.expr_key,
            expr,
            // args,
        };

        concrete_aggregations.push(concrete_aggregation);
    }

    Ok(concrete_aggregations)
}

fn prepare(code: &str, headers: &ByteRecord) -> Result<ConcreteAggregations, ConcretizationError> {
    let parsed_aggregations =
        parse_aggregations(code).map_err(|_| ConcretizationError::ParseError(code.to_string()))?;

    concretize_aggregations(parsed_aggregations, headers)
}

// NOTE: each execution unit is iterated upon linearly to aggregate values
// all while running a minimum number of operations (batched by 1. expression
// keys and 2. composite aggregation atom).
#[derive(Debug, Clone)]
struct PlannerExecutionUnit {
    expr_key: String,
    expr: Option<ConcreteExpr>,
    aggregator_blueprint: CompositeAggregator,
}

// NOTE: output unit are aligned with the list of concrete aggregations and
// offer a way to navigate the expression key indexation layer, then the
// composite aggregation layer.
#[derive(Debug, Clone)]
struct PlannerOutputUnit {
    expr_index: usize,
    aggregator_index: usize,
    agg_name: String,
    agg_method: ConcreteAggregationMethod,
}

#[derive(Debug, Clone)]
struct ConcreteAggregationPlanner {
    execution_plan: Vec<PlannerExecutionUnit>,
    output_plan: Vec<PlannerOutputUnit>,
}

impl From<ConcreteAggregations> for ConcreteAggregationPlanner {
    fn from(aggregations: ConcreteAggregations) -> Self {
        let mut execution_plan = Vec::<PlannerExecutionUnit>::new();
        let mut output_plan = Vec::<PlannerOutputUnit>::with_capacity(aggregations.len());

        for agg in aggregations {
            if let Some(expr_index) = execution_plan
                .iter()
                .position(|unit| unit.expr_key == agg.expr_key)
            {
                let aggregator_index = execution_plan[expr_index]
                    .aggregator_blueprint
                    .add_method(&agg.method);

                output_plan.push(PlannerOutputUnit {
                    expr_index,
                    aggregator_index,
                    agg_name: agg.agg_name,
                    agg_method: agg.method,
                });
            } else {
                let expr_index = execution_plan.len();
                let mut aggregator_blueprint = CompositeAggregator::new();
                let aggregator_index = aggregator_blueprint.add_method(&agg.method);

                execution_plan.push(PlannerExecutionUnit {
                    expr_key: agg.expr_key,
                    expr: agg.expr,
                    aggregator_blueprint,
                });

                output_plan.push(PlannerOutputUnit {
                    expr_index,
                    aggregator_index,
                    agg_name: agg.agg_name,
                    agg_method: agg.method,
                });
            }
        }

        Self {
            execution_plan,
            output_plan,
        }
    }
}

impl ConcreteAggregationPlanner {
    fn instantiate_aggregators(&self) -> Vec<CompositeAggregator> {
        self.execution_plan
            .iter()
            .map(|unit| unit.aggregator_blueprint.clone())
            .collect()
    }

    fn headers(&self) -> impl Iterator<Item = &[u8]> {
        self.output_plan.iter().map(|unit| unit.agg_name.as_bytes())
    }

    fn results<'a>(
        &'a self,
        aggregators: &'a [CompositeAggregator],
        context: &'a EvaluationContext,
    ) -> impl Iterator<Item = Result<DynamicValue, SpecifiedEvaluationError>> + 'a {
        self.output_plan.iter().map(move |unit| {
            aggregators[unit.expr_index].get_final_value(
                unit.aggregator_index,
                &unit.agg_method,
                context,
            )
        })
    }
}

// NOTE: parallelizing "horizontally" the planner's execution units does not
// seem to yield any performance increase. I guess the overhead is greater than
// the inner computation time.
fn run_with_record_on_aggregators(
    planner: &ConcreteAggregationPlanner,
    aggregators: &mut Vec<CompositeAggregator>,
    index: usize,
    record: &ByteRecord,
    context: &EvaluationContext,
) -> Result<(), SpecifiedEvaluationError> {
    for (unit, aggregator) in planner.execution_plan.iter().zip(aggregators) {
        let value = match &unit.expr {
            None => None,
            Some(expr) => Some(eval_expression(expr, Some(index), record, context)?),
        };

        aggregator
            .process_value(index, value, record)
            .map_err(|err| SpecifiedEvaluationError {
                reason: err,
                function_name: format!("<agg-expr: {}>", unit.expr_key),
            })?;
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct AggregationProgram {
    aggregators: Vec<CompositeAggregator>,
    planner: ConcreteAggregationPlanner,
    context: EvaluationContext,
}

impl AggregationProgram {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let concrete_aggregations = prepare(code, headers)?;
        let planner = ConcreteAggregationPlanner::from(concrete_aggregations);
        let aggregators = planner.instantiate_aggregators();

        Ok(Self {
            planner,
            aggregators,
            context: EvaluationContext::new(headers),
        })
    }

    pub fn clear(&mut self) {
        for aggregator in self.aggregators.iter_mut() {
            aggregator.clear()
        }
    }

    pub fn merge(&mut self, other: Self) {
        for (self_aggregator, other_aggregator) in
            self.aggregators.iter_mut().zip(other.aggregators)
        {
            self_aggregator.merge(other_aggregator);
        }
    }

    pub fn run_with_record(
        &mut self,
        index: usize,
        record: &ByteRecord,
    ) -> Result<(), SpecifiedEvaluationError> {
        run_with_record_on_aggregators(
            &self.planner,
            &mut self.aggregators,
            index,
            record,
            &self.context,
        )
    }

    pub fn headers(&self) -> impl Iterator<Item = &[u8]> {
        self.planner.headers()
    }

    pub fn finalize(&mut self, parallel: bool) -> Result<ByteRecord, SpecifiedEvaluationError> {
        for aggregator in self.aggregators.iter_mut() {
            aggregator.finalize(parallel);
        }

        let mut record = ByteRecord::new();

        for value in self.planner.results(&self.aggregators, &self.context) {
            record.push_field(&value?.serialize_as_bytes());
        }

        Ok(record)
    }
}

type GroupKey = Vec<Vec<u8>>;

#[derive(Debug, Clone)]
pub struct GroupAggregationProgram {
    planner: ConcreteAggregationPlanner,
    groups: SortedInsertHashmap<GroupKey, Vec<CompositeAggregator>>,
    context: EvaluationContext,
}

impl GroupAggregationProgram {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let concrete_aggregations = prepare(code, headers)?;
        let planner = ConcreteAggregationPlanner::from(concrete_aggregations);

        Ok(Self {
            planner,
            groups: SortedInsertHashmap::new(),
            context: EvaluationContext::new(headers),
        })
    }

    pub fn run_with_record(
        &mut self,
        group: GroupKey,
        index: usize,
        record: &ByteRecord,
    ) -> Result<(), SpecifiedEvaluationError> {
        let planner = &self.planner;

        let aggregators = self
            .groups
            .insert_with(group, || planner.instantiate_aggregators());

        run_with_record_on_aggregators(&self.planner, aggregators, index, record, &self.context)
    }

    pub fn headers(&self) -> impl Iterator<Item = &[u8]> {
        self.planner.headers()
    }

    pub fn into_byte_records(
        self,
        parallel: bool,
    ) -> impl Iterator<Item = Result<(GroupKey, ByteRecord), SpecifiedEvaluationError>> {
        let planner = self.planner;
        let context = self.context;

        self.groups
            .into_iter()
            .map(move |(group, mut aggregators)| {
                for aggregator in aggregators.iter_mut() {
                    aggregator.finalize(parallel);
                }

                let mut record = ByteRecord::new();

                for value in planner.results(&aggregators, &context) {
                    record.push_field(&value?.serialize_as_bytes());
                }

                Ok((group, record))
            })
    }
}

fn map_to_field<T: ToString>(opt: Option<T>) -> Vec<u8> {
    opt.map(|m| m.to_string().as_bytes().to_vec())
        .unwrap_or(b"".to_vec())
}

#[derive(Debug)]
pub struct Stats {
    nulls: bool,
    count: Count,
    extent: Extent,
    length_extent: GenericExtent<usize>,
    lexicograhic_extent: LexicographicExtent,
    welford: Welford,
    sum: Sum,
    types: Types,
    frequencies: Option<Frequencies>,
    numbers: Option<Numbers>,
}

impl Stats {
    pub fn new() -> Self {
        Self {
            nulls: false,
            count: Count::new(),
            extent: Extent::new(),
            length_extent: GenericExtent::new(),
            lexicograhic_extent: LexicographicExtent::new(),
            welford: Welford::new(),
            sum: Sum::new(),
            types: Types::new(),
            frequencies: None,
            numbers: None,
        }
    }

    pub fn include_nulls(&mut self) {
        self.nulls = true;
    }

    pub fn compute_frequencies(&mut self) {
        self.frequencies = Some(Frequencies::new());
    }

    pub fn compute_numbers(&mut self) {
        self.numbers = Some(Numbers::new());
    }

    pub fn headers(&self) -> ByteRecord {
        let mut headers = ByteRecord::new();

        headers.push_field(b"field");
        headers.push_field(b"count");
        headers.push_field(b"count_empty");
        headers.push_field(b"type");
        headers.push_field(b"types");
        headers.push_field(b"sum");
        headers.push_field(b"mean");

        if self.numbers.is_some() {
            headers.push_field(b"q1");
            headers.push_field(b"median");
            headers.push_field(b"q3");
        }

        headers.push_field(b"variance");
        headers.push_field(b"stddev");
        headers.push_field(b"min");
        headers.push_field(b"max");

        if self.frequencies.is_some() {
            headers.push_field(b"cardinality");
            headers.push_field(b"mode");
            headers.push_field(b"tied_for_mode");
        }

        headers.push_field(b"lex_first");
        headers.push_field(b"lex_last");
        headers.push_field(b"min_length");
        headers.push_field(b"max_length");

        headers
    }

    pub fn results(&self, name: &[u8]) -> ByteRecord {
        let mut record = ByteRecord::new();

        record.push_field(name);
        record.push_field(self.count.get_non_empty().to_string().as_bytes());
        record.push_field(self.count.get_empty().to_string().as_bytes());
        record.push_field(
            self.types
                .most_likely_type()
                .map(|t| t.as_bytes())
                .unwrap_or(b""),
        );
        record.push_field(self.types.sorted_types().join("|").as_bytes());
        record.push_field(&map_to_field(self.sum.get()));
        record.push_field(&map_to_field(self.welford.mean()));

        if let Some(numbers) = self.numbers.as_ref() {
            match numbers.quartiles() {
                Some(quartiles) => {
                    for quartile in quartiles {
                        record.push_field(quartile.to_string().as_bytes());
                    }
                }
                None => {
                    for _ in 0..3 {
                        record.push_field(b"");
                    }
                }
            }
        }

        record.push_field(&map_to_field(self.welford.variance()));
        record.push_field(&map_to_field(self.welford.stdev()));
        record.push_field(&map_to_field(self.extent.min()));
        record.push_field(&map_to_field(self.extent.max()));

        if let Some(frequencies) = self.frequencies.as_ref() {
            record.push_field(frequencies.cardinality().to_string().as_bytes());

            let modes = frequencies.modes();

            record.push_field(&map_to_field(modes.as_ref().map(|m| m[0].clone())));
            record.push_field(&map_to_field(modes.map(|m| m.len())));
        }

        record.push_field(&map_to_field(self.lexicograhic_extent.first()));
        record.push_field(&map_to_field(self.lexicograhic_extent.last()));
        record.push_field(&map_to_field(self.length_extent.min()));
        record.push_field(&map_to_field(self.length_extent.max()));

        record
    }

    pub fn process(&mut self, cell: &[u8]) {
        self.length_extent.add(cell.len());

        if cell.is_empty() {
            self.types.set_empty();
            self.count.add_empty();

            if self.nulls {
                self.welford.add(0.0);

                if let Some(numbers) = self.numbers.as_mut() {
                    numbers.add(DynamicNumber::Float(0.0));
                }
            }

            return;
        }

        self.count.add_non_empty();

        let cell = std::str::from_utf8(cell).expect("could not decode as utf-8");

        if let Ok(number) = cell.parse::<DynamicNumber>() {
            self.sum.add(number);
            self.welford.add(number.as_float());
            self.extent.add(number);

            match number {
                DynamicNumber::Float(_) => self.types.set_float(),
                DynamicNumber::Integer(_) => self.types.set_int(),
            }

            if let Some(numbers) = self.numbers.as_mut() {
                numbers.add(number);
            }
        } else if cell.parse::<DateTime>().is_ok() {
            self.types.set_date();
        } else if cell.starts_with("http://") || cell.starts_with("https://") {
            self.types.set_url();
        } else {
            self.types.set_string();
        }

        if let Some(frequencies) = self.frequencies.as_mut() {
            frequencies.add(cell.to_string());
        }

        self.lexicograhic_extent.add(cell);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl From<Vec<usize>> for Numbers {
        fn from(values: Vec<usize>) -> Self {
            let mut numbers = Self::new();

            for n in values {
                numbers.add(DynamicNumber::Integer(n as i64));
            }

            numbers
        }
    }

    #[test]
    fn test_median_aggregator() {
        let odd = vec![1, 3, 5];
        let even = vec![1, 2, 6, 7];

        let mut no_numbers = Numbers::new();
        let mut lone_numbers = Numbers::from(vec![8]);
        let mut odd_numbers = Numbers::from(odd);
        let mut even_numbers = Numbers::from(even);

        no_numbers.finalize(false);
        lone_numbers.finalize(false);
        odd_numbers.finalize(false);
        even_numbers.finalize(false);

        // Low
        assert_eq!(no_numbers.median(&MedianType::Low), None);

        assert_eq!(
            lone_numbers.median(&MedianType::Low),
            Some(DynamicNumber::Integer(8))
        );

        assert_eq!(
            odd_numbers.median(&MedianType::Low),
            Some(DynamicNumber::Integer(3))
        );

        assert_eq!(
            even_numbers.median(&MedianType::Low),
            Some(DynamicNumber::Integer(2))
        );

        // High
        assert_eq!(no_numbers.median(&MedianType::High), None);

        assert_eq!(
            lone_numbers.median(&MedianType::High),
            Some(DynamicNumber::Integer(8))
        );

        assert_eq!(
            odd_numbers.median(&MedianType::High),
            Some(DynamicNumber::Integer(3))
        );

        assert_eq!(
            even_numbers.median(&MedianType::High),
            Some(DynamicNumber::Integer(6))
        );

        // High
        assert_eq!(no_numbers.median(&MedianType::Interpolation), None);

        assert_eq!(
            lone_numbers.median(&MedianType::Interpolation),
            Some(DynamicNumber::Integer(8))
        );

        assert_eq!(
            odd_numbers.median(&MedianType::Interpolation),
            Some(DynamicNumber::Integer(3))
        );

        assert_eq!(
            even_numbers.median(&MedianType::Interpolation),
            Some(DynamicNumber::Float(4.0))
        );

        // Quartiles
        fn manual_quartiles(n: &Numbers) -> Option<Vec<DynamicNumber>> {
            Some(vec![
                n.quantile(0.25).unwrap(),
                n.quantile(0.5).unwrap(),
                n.quantile(0.75).unwrap(),
            ])
        }

        assert_eq!(
            even_numbers.quartiles(),
            Some(vec![
                DynamicNumber::Float(1.75),
                DynamicNumber::Float(4.0),
                DynamicNumber::Float(6.25)
            ])
        );
        assert_eq!(
            manual_quartiles(&even_numbers),
            Some(vec![
                DynamicNumber::Float(1.5),
                DynamicNumber::Float(4.0),
                DynamicNumber::Float(6.5)
            ])
        );

        assert_eq!(
            odd_numbers.quartiles(),
            Some(vec![
                DynamicNumber::Float(2.0),
                DynamicNumber::Float(3.0),
                DynamicNumber::Float(4.0)
            ])
        );
        assert_eq!(
            manual_quartiles(&odd_numbers),
            Some(vec![
                DynamicNumber::Integer(1),
                DynamicNumber::Integer(3),
                DynamicNumber::Integer(5)
            ])
        );
    }

    #[test]
    fn test_types_aggregator() {
        let mut types = Types::new();

        assert_eq!(types.sorted_types(), Vec::<&str>::new());
        assert_eq!(types.most_likely_type(), None);

        types.set_int();

        assert_eq!(types.sorted_types(), vec!["int"]);
        assert_eq!(types.most_likely_type(), Some("int"));

        types.set_float();

        assert_eq!(types.sorted_types(), vec!["int", "float"]);
        assert_eq!(types.most_likely_type(), Some("float"));

        types.set_string();

        assert_eq!(types.sorted_types(), vec!["int", "float", "string"]);
        assert_eq!(types.most_likely_type(), Some("string"));
    }

    // #[test]
    // fn test_planner() {
    //     let mut headers = ByteRecord::new();
    //     headers.push_field(b"A");
    //     headers.push_field(b"B");
    //     headers.push_field(b"C");

    //     let agg = parse_aggregations("mean(A), var(A), sum(B), last(A), first(C)").unwrap();
    //     let agg = concretize_aggregations(agg, &headers).unwrap();

    //     let planner = ConcreteAggregationPlanner::from(agg);

    //     dbg!(planner);
    // }
}
