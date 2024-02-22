use std::collections::HashMap;

use csv::ByteRecord;
use rayon::prelude::*;

use super::error::{ConcretizationError, EvaluationError, SpecifiedEvaluationError};
use super::interpreter::{concretize_expression, eval_expression, ConcreteExpr, EvaluationContext};
use super::parser::{parse_aggregations, Aggregation, Aggregations};
use super::types::{DynamicNumber, DynamicValue};

#[derive(Debug, Clone)]
struct Count {
    current: usize,
}

impl Count {
    fn new() -> Self {
        Self { current: 0 }
    }

    fn clear(&mut self) {
        self.current = 0;
    }

    fn add(&mut self) {
        self.current += 1;
    }

    fn get(&self) -> usize {
        self.current
    }

    fn merge(&mut self, other: Self) {
        self.current += other.current;
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
    current: DynamicNumber,
}

impl Sum {
    fn new() -> Self {
        Self {
            current: DynamicNumber::Integer(0),
        }
    }

    fn clear(&mut self) {
        self.current = DynamicNumber::Integer(0);
    }

    // TODO: implement kahan-babushka summation from https://github.com/simple-statistics/simple-statistics/blob/main/src/sum.js
    fn add(&mut self, value: &DynamicNumber) {
        match &mut self.current {
            DynamicNumber::Float(a) => match value {
                DynamicNumber::Float(b) => *a += b,
                DynamicNumber::Integer(b) => *a += *b as f64,
            },
            DynamicNumber::Integer(a) => match value {
                DynamicNumber::Float(b) => self.current = DynamicNumber::Float((*a as f64) + b),
                DynamicNumber::Integer(b) => *a += b,
            },
        };
    }

    fn get(&self) -> DynamicNumber {
        self.current
    }

    fn merge(&mut self, other: Self) {
        self.add(&other.current);
    }
}

#[derive(Debug, Clone)]
struct Extent {
    extent: Option<(DynamicNumber, DynamicNumber)>,
}

impl Extent {
    fn new() -> Self {
        Self { extent: None }
    }

    fn clear(&mut self) {
        self.extent = None;
    }

    fn add(&mut self, value: DynamicNumber) {
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

    fn min(&self) -> Option<DynamicNumber> {
        self.extent.map(|e| e.0)
    }

    fn max(&self) -> Option<DynamicNumber> {
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

type ArgExtentEntry = (DynamicNumber, (usize, csv::ByteRecord));

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

    fn add(&mut self, index: usize, value: DynamicNumber, arg: &csv::ByteRecord) {
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

    fn merge(&mut self, other: Self) {
        self.numbers.extend(other.numbers);
    }
}

#[derive(Debug, Clone)]
struct Frequencies {
    counter: HashMap<String, usize>,
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

    fn add_count(&mut self, value: String, count: usize) {
        self.counter
            .entry(value)
            .and_modify(|current| *current += count)
            .or_insert(count);
    }

    fn add(&mut self, value: String) {
        self.add_count(value, 1);
    }

    fn mode(&self) -> Option<String> {
        let mut max: Option<(usize, &String)> = None;

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
struct Welford {
    count: usize,
    mean: f64,
    m2: f64,
}

impl Welford {
    fn new() -> Self {
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

    fn add(&mut self, value: f64) {
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

    fn mean(&self) -> Option<f64> {
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

#[derive(Debug, Clone)]
struct Types {
    bitset: u8,
}

impl Types {
    fn new() -> Self {
        Types { bitset: 0 }
    }

    fn set(&mut self, pos: u8) {
        self.bitset |= 1 << pos;
    }

    fn set_empty(&mut self) {
        self.set(TYPE_EMPTY)
    }

    fn set_string(&mut self) {
        self.set(TYPE_STRING)
    }

    fn set_float(&mut self) {
        self.set(TYPE_FLOAT)
    }

    fn set_int(&mut self) {
        self.set(TYPE_INT)
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

    fn most_likely_type(&self) -> Option<&str> {
        Some(if self.has_string() {
            "string"
        } else if self.has_float() {
            "float"
        } else if self.has_int() {
            "int"
        } else if self.has_empty() {
            "empty"
        } else {
            return None;
        })
    }

    fn sorted_types(&self) -> Vec<&str> {
        let mut result: Vec<&str> = Vec::new();

        if self.has_int() {
            result.push("int");
        }
        if self.has_float() {
            result.push("float");
        }
        if self.has_string() {
            result.push("string");
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
    ) -> Result<DynamicValue, EvaluationError> {
        Ok(match (self, method) {
            (Self::AllAny(inner), ConcreteAggregationMethod::All) => {
                DynamicValue::from(inner.all())
            }
            (Self::AllAny(inner), ConcreteAggregationMethod::Any) => {
                DynamicValue::from(inner.any())
            }
            (Self::Frequencies(inner), ConcreteAggregationMethod::Cardinality) => {
                DynamicValue::from(inner.cardinality())
            }
            (Self::Count(inner), ConcreteAggregationMethod::Count) => {
                DynamicValue::from(inner.get())
            }
            (Self::Frequencies(inner), ConcreteAggregationMethod::DistinctValues(separator)) => {
                DynamicValue::from(inner.join(separator))
            }
            (Self::First(inner), ConcreteAggregationMethod::First) => {
                DynamicValue::from(inner.first())
            }
            (Self::Last(inner), ConcreteAggregationMethod::Last) => {
                DynamicValue::from(inner.last())
            }
            (Self::LexicographicExtent(inner), ConcreteAggregationMethod::LexFirst) => {
                DynamicValue::from(inner.first())
            }
            (Self::LexicographicExtent(inner), ConcreteAggregationMethod::LexLast) => {
                DynamicValue::from(inner.last())
            }
            (Self::Extent(inner), ConcreteAggregationMethod::Min) => {
                DynamicValue::from(inner.min())
            }
            (Self::ArgExtent(inner), ConcreteAggregationMethod::Min) => {
                DynamicValue::from(inner.min())
            }
            (Self::Welford(inner), ConcreteAggregationMethod::Mean) => {
                DynamicValue::from(inner.mean())
            }
            (Self::Numbers(inner), ConcreteAggregationMethod::Median(median_type)) => {
                DynamicValue::from(inner.median(median_type))
            }
            (Self::Extent(inner), ConcreteAggregationMethod::Max) => {
                DynamicValue::from(inner.max())
            }
            (Self::ArgExtent(inner), ConcreteAggregationMethod::Max) => {
                DynamicValue::from(inner.max())
            }
            (Self::Frequencies(inner), ConcreteAggregationMethod::Mode) => {
                DynamicValue::from(inner.mode())
            }
            (Self::Sum(inner), ConcreteAggregationMethod::Sum) => DynamicValue::from(inner.get()),
            (Self::Welford(inner), ConcreteAggregationMethod::VarPop) => {
                DynamicValue::from(inner.variance())
            }
            (Self::Welford(inner), ConcreteAggregationMethod::VarSample) => {
                DynamicValue::from(inner.sample_variance())
            }
            (Self::Welford(inner), ConcreteAggregationMethod::StddevPop) => {
                DynamicValue::from(inner.stdev())
            }
            (Self::Welford(inner), ConcreteAggregationMethod::StddevSample) => {
                DynamicValue::from(inner.sample_stdev())
            }
            (Self::Types(inner), ConcreteAggregationMethod::Types) => {
                DynamicValue::from(inner.sorted_types().join("|"))
            }
            (Self::Types(inner), ConcreteAggregationMethod::Type) => {
                DynamicValue::from(inner.most_likely_type())
            }
            (Self::Values(inner), ConcreteAggregationMethod::Values(separator)) => {
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
            ConcreteAggregationMethod::Count => {
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
            ConcreteAggregationMethod::First => {
                upsert_aggregator!(First)
            }
            ConcreteAggregationMethod::Last => {
                upsert_aggregator!(Last)
            }
            ConcreteAggregationMethod::LexFirst | ConcreteAggregationMethod::LexLast => {
                upsert_aggregator!(LexicographicExtent)
            }
            ConcreteAggregationMethod::Median(_) => {
                upsert_aggregator!(Numbers)
            }
            ConcreteAggregationMethod::Mode
            | ConcreteAggregationMethod::Cardinality
            | ConcreteAggregationMethod::DistinctValues(_) => {
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
                            count.add();
                        }
                    }
                    Aggregator::Extent(extent) => {
                        extent.add(value.try_as_number()?);
                    }
                    Aggregator::ArgExtent(extent) => {
                        extent.add(index, value.try_as_number()?, record);
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
                        extent.add(&value.try_as_str()?);
                    }
                    Aggregator::Frequencies(frequencies) => {
                        frequencies.add(value.try_as_str()?.into_owned());
                    }
                    Aggregator::Numbers(numbers) => {
                        numbers.add(value.try_as_number()?);
                    }
                    Aggregator::Sum(sum) => {
                        sum.add(&value.try_as_number()?);
                    }
                    Aggregator::Welford(variance) => {
                        variance.add(value.try_as_f64()?);
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
                            types.set_string();
                        }
                    }
                    Aggregator::Values(values) => {
                        values.add(value.try_as_str()?.into_owned());
                    }
                },
                None => match method {
                    Aggregator::Count(count) => {
                        count.add();
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
    ) -> Result<DynamicValue, EvaluationError> {
        self.methods[handle].get_final_value(method, context)
    }
}

fn validate_aggregation_function_arity(
    aggregation: &Aggregation,
) -> Result<(), ConcretizationError> {
    let arity = aggregation.args.len();

    match aggregation.func_name.as_str() {
        "count" => {
            if !(0..=1).contains(&arity) {
                Err(ConcretizationError::from_invalid_range_arity(
                    aggregation.func_name.clone(),
                    0..=1,
                    arity,
                ))
            } else {
                Ok(())
            }
        }
        "values" | "distinct_values" | "argmin" | "argmax" => {
            if !(1..=2).contains(&arity) {
                Err(ConcretizationError::from_invalid_range_arity(
                    aggregation.func_name.clone(),
                    1..=2,
                    arity,
                ))
            } else {
                Ok(())
            }
        }
        _ => {
            if arity != 1 {
                Err(ConcretizationError::from_invalid_arity(
                    aggregation.func_name.clone(),
                    1,
                    arity,
                ))
            } else {
                Ok(())
            }
        }
    }
}

fn get_separator_from_optional_first_argument(args: Vec<ConcreteExpr>) -> Option<String> {
    Some(match args.first() {
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
    ArgMin(ConcreteExpr),
    ArgMax(ConcreteExpr),
    Cardinality,
    Count,
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
    Sum,
    Values(String),
    VarPop,
    VarSample,
    StddevPop,
    StddevSample,
    Type,
    Types,
}

impl ConcreteAggregationMethod {
    fn parse(name: &str, mut args: Vec<ConcreteExpr>) -> Result<Self, ConcretizationError> {
        Ok(match name {
            "all" => Self::All,
            "any" => Self::Any,
            "argmin" => Self::ArgMin(args.pop().unwrap()),
            "argmax" => Self::ArgMax(args.pop().unwrap()),
            "cardinality" => Self::Cardinality,
            "count" => Self::Count,
            "distinct_values" => {
                Self::DistinctValues(match get_separator_from_optional_first_argument(args) {
                    None => return Err(ConcretizationError::NotStaticallyAnalyzable),
                    Some(separator) => separator,
                })
            }
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
            "values" => Self::Values(match get_separator_from_optional_first_argument(args) {
                None => return Err(ConcretizationError::NotStaticallyAnalyzable),
                Some(separator) => separator,
            }),
            "var" | "var_pop" => Self::VarPop,
            "var_sample" => Self::VarSample,
            "stddev" | "stddev_pop" => Self::StddevPop,
            "stddev_sample" => Self::StddevSample,
            "sum" => Self::Sum,
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

    for aggregation in aggregations {
        validate_aggregation_function_arity(&aggregation)?;

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
    ) -> impl Iterator<Item = Result<DynamicValue, EvaluationError>> + 'a {
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

    pub fn finalize(&mut self, parallel: bool) -> Result<ByteRecord, EvaluationError> {
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
    groups: HashMap<GroupKey, Vec<CompositeAggregator>>,
    context: EvaluationContext,
}

impl GroupAggregationProgram {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let concrete_aggregations = prepare(code, headers)?;
        let planner = ConcreteAggregationPlanner::from(concrete_aggregations);

        Ok(Self {
            planner,
            groups: HashMap::new(),
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
            .entry(group)
            .or_insert_with(|| planner.instantiate_aggregators());

        run_with_record_on_aggregators(&self.planner, aggregators, index, record, &self.context)
    }

    pub fn headers(&self) -> impl Iterator<Item = &[u8]> {
        self.planner.headers()
    }

    pub fn into_byte_records(
        self,
        parallel: bool,
    ) -> impl Iterator<Item = Result<(GroupKey, ByteRecord), EvaluationError>> {
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
