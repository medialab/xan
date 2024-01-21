use std::collections::hash_map::Entry;
use std::collections::HashMap;

use csv::ByteRecord;

use super::error::{CallError, ConcretizationError, EvaluationError, SpecifiedCallError};
use super::interpreter::{concretize_argument, eval_expr, ConcreteArgument};
use super::parser::{parse_aggregations, Aggregation, Aggregations};
use super::types::{DynamicNumber, DynamicValue, HeadersIndex, Variables};

// TODO: test when there is no data to be aggregated at all
// TODO: test with nulls and nans
// TODO: parallelize multiple aggregations
// TODO: validate agg arity
// TODO: we need some clear method to enable sorted group by optimization
// TODO: we need some merge method to enable parallelism
// TODO: factor count/sum/variance/mean into OnlineStats, or we could optimize if variance if needed

#[derive(Debug)]
struct Count {
    current: usize,
}

impl Count {
    fn new() -> Self {
        Self { current: 0 }
    }

    fn add(&mut self) {
        self.current += 1;
    }

    fn to_value(&self) -> DynamicValue {
        DynamicValue::from(self.current)
    }

    fn to_float(&self) -> f64 {
        self.current as f64
    }
}

#[derive(Debug)]
struct Sum {
    current: DynamicNumber,
}

impl Sum {
    fn new() -> Self {
        Self {
            current: DynamicNumber::Integer(0),
        }
    }

    // TODO: implement kahan-babushka summation from https://github.com/simple-statistics/simple-statistics/blob/main/src/sum.js
    fn add(&mut self, value: &DynamicNumber) {
        self.current = match self.current {
            DynamicNumber::Float(a) => match value {
                DynamicNumber::Float(b) => DynamicNumber::Float(a + b),
                DynamicNumber::Integer(b) => DynamicNumber::Float(a + (*b as f64)),
            },
            DynamicNumber::Integer(a) => match value {
                DynamicNumber::Float(b) => DynamicNumber::Float((a as f64) + b),
                DynamicNumber::Integer(b) => DynamicNumber::Integer(a + b),
            },
        }
    }

    fn to_value(&self) -> DynamicValue {
        DynamicValue::from(self.current.clone())
    }

    fn to_float(&self) -> f64 {
        match self.current {
            DynamicNumber::Float(v) => v,
            DynamicNumber::Integer(v) => v as f64,
        }
    }
}

#[derive(Debug)]
struct Extent {
    min: Option<DynamicNumber>,
    max: Option<DynamicNumber>,
    min_string: Option<String>,
    max_string: Option<String>,
    as_string: bool,
}

impl Extent {
    fn new() -> Self {
        Self {
            min: None,
            max: None,
            min_string: None,
            max_string: None,
            as_string: false,
        }
    }

    fn update_string(&mut self, string: String) {
        if let Some(current) = &self.min_string {
            if &string < current {
                self.min_string = Some(string.clone());
            }
        } else {
            self.min_string = Some(string.clone());
        }

        if let Some(current) = &self.max_string {
            if &string > current {
                self.max_string = Some(string);
            }
        } else {
            self.max_string = Some(string);
        }
    }

    fn update_number(&mut self, number: DynamicNumber) {
        if let Some(current) = &self.min {
            if &number < current {
                self.min = Some(number.clone());
            }
        } else {
            self.min = Some(number.clone());
        }

        if let Some(current) = &self.max {
            if &number > current {
                self.max = Some(number);
            }
        } else {
            self.max = Some(number);
        }
    }

    fn add(&mut self, value: &DynamicValue) {
        if self.as_string {
            match value.try_as_str() {
                Err(_) => return,
                Ok(string) => self.update_string(string.into_owned()),
            }
            return;
        }

        match value.try_as_number() {
            Err(_) => {
                // Switching to tracking strings
                self.as_string = true;

                self.min_string = self.min.as_ref().map(|min| min.to_string());
                self.max_string = self.max.as_ref().map(|max| max.to_string());

                return self.add(value);
            }
            Ok(number) => {
                self.update_number(number);
            }
        };
    }

    fn min_to_value(&self) -> DynamicValue {
        if self.as_string {
            return DynamicValue::from(self.min_string.clone());
        }

        DynamicValue::from(self.min.clone())
    }

    fn max_to_value(&self) -> DynamicValue {
        if self.as_string {
            return DynamicValue::from(self.max_string.clone());
        }

        DynamicValue::from(self.max.clone())
    }
}

enum MedianType {
    Interpolation,
    Low,
    High,
}

#[derive(Debug)]
struct Numbers {
    sorted: bool,
    numbers: Vec<DynamicNumber>,
}

impl Numbers {
    fn new() -> Self {
        Self {
            sorted: false,
            numbers: Vec::new(),
        }
    }

    fn add(&mut self, number: DynamicNumber) {
        self.numbers.push(number);
    }

    fn sort_if_needed(&mut self) {
        if self.sorted {
            return;
        }

        // TODO: can be done in parallel in the future if required, using rayon
        self.numbers.sort_by(|a, b| a.partial_cmp(&b).unwrap());
        self.sorted = true;
    }

    fn median_to_value(&mut self, median_type: MedianType) -> DynamicValue {
        self.sort_if_needed();

        let count = self.numbers.len();

        if count == 0 {
            return DynamicValue::None;
        }

        let median = match median_type {
            MedianType::Low => {
                let mut midpoint = count / 2;

                if count % 2 == 0 {
                    midpoint -= 1;
                }

                self.numbers[midpoint].clone()
            }
            MedianType::High => {
                let midpoint = count / 2;

                self.numbers[midpoint].clone()
            }
            MedianType::Interpolation => {
                let midpoint = count / 2;

                if count % 2 == 1 {
                    self.numbers[midpoint].clone()
                } else {
                    let down = &self.numbers[midpoint - 1];
                    let up = &self.numbers[midpoint];

                    (down.clone() + up.clone()) / DynamicNumber::Float(2.0)
                }
            }
        };

        DynamicValue::from(median)
    }
}

// NOTE: this is an implementation of Welford's online algorithm
#[derive(Debug)]
struct Variance {
    count: usize,
    m: f64,
    m2: f64,
}

impl Variance {
    fn new() -> Self {
        Self {
            count: 0,
            m: 0.0,
            m2: 0.0,
        }
    }

    fn add(&mut self, value: f64) {
        let (mut count, mut m, mut m2) = (self.count, self.m, self.m2);
        count += 1;
        let delta = value - m;
        m += delta / count as f64;
        let delta2 = value - m;
        m2 += delta * delta2;

        self.count = count;
        self.m = m;
        self.m2 = m2;
    }

    // fn mean(&self) -> Option<f64> {
    //     if self.count == 0 {
    //         return None;
    //     }

    //     Some(self.m)
    // }

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
}

#[derive(Debug)]
enum AggregationMethod {
    Count(Count),
    Extent(Extent),
    Numbers(Numbers),
    Sum(Sum),
    Variance(Variance),
}

impl AggregationMethod {
    fn is_count(&self) -> bool {
        match self {
            Self::Count(_) => true,
            _ => false,
        }
    }

    fn is_extent(&self) -> bool {
        match self {
            Self::Extent(_) => true,
            _ => false,
        }
    }

    fn is_numbers(&self) -> bool {
        match self {
            Self::Numbers(_) => true,
            _ => false,
        }
    }

    fn is_sum(&self) -> bool {
        match self {
            Self::Sum(_) => true,
            _ => false,
        }
    }

    fn is_variance(&self) -> bool {
        match self {
            Self::Variance(_) => true,
            _ => false,
        }
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

#[derive(Debug)]
struct Aggregator {
    methods: Vec<AggregationMethod>,
}

impl Aggregator {
    fn new() -> Self {
        Self {
            methods: Vec::new(),
        }
    }

    fn with_method(method: &str) -> Self {
        let mut aggregator = Self::new();
        aggregator.add_method(method);
        aggregator
    }

    fn has_count(&self) -> bool {
        self.methods.iter().any(|method| method.is_count())
    }

    fn has_extent(&self) -> bool {
        self.methods.iter().any(|method| method.is_extent())
    }

    fn has_numbers(&self) -> bool {
        self.methods.iter().any(|method| method.is_numbers())
    }

    fn has_sum(&self) -> bool {
        self.methods.iter().any(|method| method.is_sum())
    }

    fn has_variance(&self) -> bool {
        self.methods.iter().any(|method| method.is_variance())
    }

    fn get_count(&self) -> Option<&Count> {
        for method in self.methods.iter() {
            match method {
                AggregationMethod::Count(count) => return Some(count),
                _ => continue,
            }
        }

        None
    }

    fn get_extent(&self) -> Option<&Extent> {
        for method in self.methods.iter() {
            match method {
                AggregationMethod::Extent(extent) => return Some(extent),
                _ => continue,
            }
        }

        None
    }

    fn get_numbers_mut(&mut self) -> Option<&mut Numbers> {
        for method in self.methods.iter_mut() {
            match method {
                AggregationMethod::Numbers(numbers) => return Some(numbers),
                _ => continue,
            }
        }

        None
    }

    fn get_sum(&self) -> Option<&Sum> {
        for method in self.methods.iter() {
            match method {
                AggregationMethod::Sum(sum) => return Some(sum),
                _ => continue,
            }
        }

        None
    }

    fn get_variance(&self) -> Option<&Variance> {
        for method in self.methods.iter() {
            match method {
                AggregationMethod::Variance(variance) => return Some(variance),
                _ => continue,
            }
        }

        None
    }

    fn add_method(&mut self, method: &str) {
        match method {
            "count" => {
                if self.has_count() {
                    return;
                }

                self.methods.push(AggregationMethod::Count(Count::new()));
            }
            "min" | "max" => {
                if self.has_extent() {
                    return;
                }

                self.methods.push(AggregationMethod::Extent(Extent::new()));
            }
            "avg" | "mean" => {
                if !self.has_count() {
                    self.methods.push(AggregationMethod::Count(Count::new()));
                }
                if !self.has_sum() {
                    self.methods.push(AggregationMethod::Sum(Sum::new()));
                }
            }
            "median" | "median_low" | "median_high" => {
                if !self.has_numbers() {
                    self.methods
                        .push(AggregationMethod::Numbers(Numbers::new()));
                }
            }
            "sum" => {
                if self.has_sum() {
                    return;
                }

                self.methods.push(AggregationMethod::Sum(Sum::new()));
            }
            "var" | "var_sample" | "var_pop" | "stddev" | "stddev_sample" | "stddev_pop" => {
                if self.has_variance() {
                    return;
                }

                self.methods
                    .push(AggregationMethod::Variance(Variance::new()));
            }
            _ => unreachable!(),
        }
    }

    fn process_value(&mut self, value: DynamicValue) -> Result<(), CallError> {
        for method in self.methods.iter_mut() {
            match method {
                AggregationMethod::Count(count) => {
                    count.add();
                }
                AggregationMethod::Extent(extent) => {
                    extent.add(&value);
                }
                AggregationMethod::Numbers(numbers) => {
                    numbers.add(value.try_as_number()?);
                }
                AggregationMethod::Sum(sum) => {
                    sum.add(&value.try_as_number()?);
                }
                AggregationMethod::Variance(variance) => {
                    variance.add(value.try_as_f64()?);
                }
            }
        }

        Ok(())
    }

    fn finalize_method(&mut self, method: &str) -> DynamicValue {
        match method {
            "count" => self.get_count().unwrap().to_value(),
            "min" => self.get_extent().unwrap().min_to_value(),
            "avg" | "mean" => {
                let count = self.get_count().unwrap().to_float();

                if count == 0.0 {
                    return DynamicValue::None;
                }

                let sum = self.get_sum().unwrap().to_float();

                DynamicValue::from(sum / count)
            }
            "median" => self
                .get_numbers_mut()
                .unwrap()
                .median_to_value(MedianType::Interpolation),
            "median_high" => self
                .get_numbers_mut()
                .unwrap()
                .median_to_value(MedianType::High),
            "median_low" => self
                .get_numbers_mut()
                .unwrap()
                .median_to_value(MedianType::Low),
            "max" => self.get_extent().unwrap().max_to_value(),
            "sum" => self.get_sum().unwrap().to_value(),
            "var" | "var_pop" => DynamicValue::from(self.get_variance().unwrap().variance()),
            "var_sample" => DynamicValue::from(self.get_variance().unwrap().sample_variance()),
            "stddev" | "stddev_pop" => DynamicValue::from(self.get_variance().unwrap().stdev()),
            "stddev_sample" => DynamicValue::from(self.get_variance().unwrap().sample_stdev()),
            _ => unreachable!(),
        }
    }
}

// NOTE: the rationale of the `KeyedAggregator` is to make sure to group
// aggregations per expression. This means 'sum(A) as sum, mean(A)` will never
// need to run the expression twice and can share an `Aggregator` allocation.
// TODO: deal with count() having no expr.
#[derive(Debug)]
struct KeyedAggregatorEntry {
    key: String,
    expr: Option<ConcreteArgument>,
    aggregator: Aggregator,
}

#[derive(Debug)]
struct KeyedAggregator {
    mapping: Vec<KeyedAggregatorEntry>,
}

impl KeyedAggregator {
    fn new() -> Self {
        Self {
            mapping: Vec::new(),
        }
    }

    fn get_mut(&mut self, key: &str) -> Option<&mut KeyedAggregatorEntry> {
        self.mapping.iter_mut().find(|entry| entry.key == key)
    }

    fn add(&mut self, aggregation: &ConcreteAggregation) {
        match self.get_mut(&aggregation.key) {
            None => {
                self.mapping.push(KeyedAggregatorEntry {
                    key: aggregation.key.clone(),
                    expr: aggregation.expr.clone(),
                    aggregator: Aggregator::with_method(&aggregation.method),
                });
            }
            Some(entry) => entry.aggregator.add_method(&aggregation.method),
        }
    }

    fn run_with_record(
        &mut self,
        record: &ByteRecord,
        headers_index: &HeadersIndex,
        variables: &Variables,
    ) -> Result<(), EvaluationError> {
        for entry in self.mapping.iter_mut() {
            // NOTE: count tolerates having no expression to evaluate, for instance.
            let value = match &entry.expr {
                None => DynamicValue::None,
                Some(expr) => eval_expr(expr, record, headers_index, variables)?,
            };

            entry.aggregator.process_value(value).map_err(|err| {
                EvaluationError::Call(SpecifiedCallError {
                    reason: err,
                    function_name: "<agg-expr>".to_string(),
                })
            })?;
        }

        Ok(())
    }

    fn finalize(&mut self, key: &str, method: &str) -> Option<DynamicValue> {
        self.get_mut(key)
            .map(|entry| entry.aggregator.finalize_method(method))
    }
}

impl From<&ConcreteAggregations> for KeyedAggregator {
    fn from(aggregations: &ConcreteAggregations) -> Self {
        let mut aggregator = Self::new();

        for agg in aggregations {
            aggregator.add(agg);
        }

        aggregator
    }
}

fn validate_aggregation_function_arity(
    aggregation: &Aggregation,
) -> Result<(), ConcretizationError> {
    let arity = aggregation.args.len();

    match aggregation.name.as_str() {
        "count" => {
            if !(0..=1).contains(&arity) {
                Err(ConcretizationError::from_range_arity(
                    aggregation.method.clone(),
                    0..=1,
                    arity,
                ))
            } else {
                Ok(())
            }
        }
        _ => {
            if arity != 1 {
                Err(ConcretizationError::from_invalid_arity(
                    aggregation.method.clone(),
                    1,
                    arity,
                ))
            } else {
                Ok(())
            }
        }
    }
}

#[derive(Debug)]
struct ConcreteAggregation {
    name: String,
    method: String,
    expr: Option<ConcreteArgument>,
    key: String,
    // args: Vec<ConcreteArgument>,
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
            .get(0)
            .map(|arg| concretize_argument(arg.clone(), headers))
            .transpose()?;

        let mut args: Vec<ConcreteArgument> = Vec::new();

        for arg in aggregation.args.into_iter().skip(1) {
            args.push(concretize_argument(arg, headers)?);
        }

        let concrete_aggregation = ConcreteAggregation {
            name: aggregation.name,
            method: aggregation.method,
            key: aggregation.key,
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

#[derive(Debug)]
pub struct AggregationProgram<'a> {
    aggregations: ConcreteAggregations,
    aggregator: KeyedAggregator,
    headers_index: HeadersIndex,
    variables: Variables<'a>,
}

impl<'a> AggregationProgram<'a> {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let concrete_aggregations = prepare(code, headers)?;

        let aggregator = KeyedAggregator::from(&concrete_aggregations);

        Ok(Self {
            aggregations: concrete_aggregations,
            aggregator,
            headers_index: HeadersIndex::from_headers(headers),
            variables: Variables::new(),
        })
    }

    pub fn run_with_record(&mut self, record: &ByteRecord) -> Result<(), EvaluationError> {
        self.aggregator
            .run_with_record(record, &self.headers_index, &self.variables)
    }

    pub fn headers(&self) -> ByteRecord {
        let mut record = ByteRecord::new();

        for aggregation in self.aggregations.iter() {
            record.push_field(aggregation.name.as_bytes());
        }

        record
    }

    pub fn finalize(mut self) -> ByteRecord {
        let mut record = ByteRecord::new();

        for aggregation in self.aggregations.into_iter() {
            let value = self
                .aggregator
                .finalize(&aggregation.key, &aggregation.method)
                .unwrap();

            record.push_field(&value.serialize_as_bytes(b"|"));
        }

        record
    }
}

#[derive(Debug)]
pub struct GroupAggregationProgram<'a> {
    aggregations: ConcreteAggregations,
    groups: HashMap<Vec<u8>, KeyedAggregator>,
    headers_index: HeadersIndex,
    variables: Variables<'a>,
}

impl<'a> GroupAggregationProgram<'a> {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let concrete_aggregations = prepare(code, headers)?;

        Ok(Self {
            aggregations: concrete_aggregations,
            groups: HashMap::new(),
            headers_index: HeadersIndex::from_headers(headers),
            variables: Variables::new(),
        })
    }

    pub fn run_with_record(
        &mut self,
        group: Vec<u8>,
        record: &ByteRecord,
    ) -> Result<(), EvaluationError> {
        match self.groups.entry(group) {
            Entry::Vacant(entry) => {
                let mut aggregator = KeyedAggregator::from(&self.aggregations);
                aggregator.run_with_record(record, &self.headers_index, &self.variables)?;
                entry.insert(aggregator);
            }
            Entry::Occupied(mut entry) => {
                entry
                    .get_mut()
                    .run_with_record(record, &self.headers_index, &self.variables)?;
            }
        }

        Ok(())
    }

    pub fn headers(&self) -> ByteRecord {
        let mut record = ByteRecord::new();
        record.push_field(b"group");

        for aggregation in self.aggregations.iter() {
            record.push_field(aggregation.name.as_bytes());
        }

        record
    }

    pub fn finalize<F, E>(mut self, mut callback: F) -> Result<(), E>
    where
        F: FnMut(&ByteRecord) -> Result<(), E>,
    {
        let mut record = ByteRecord::new();

        for (group, mut aggregator) in self.groups.into_iter() {
            record.clear();
            record.push_field(&group);

            for aggregation in self.aggregations.iter_mut() {
                let value = aggregator
                    .finalize(&aggregation.key, &aggregation.method)
                    .unwrap();

                record.push_field(&value.serialize_as_bytes(b"|"));
            }

            callback(&record)?;
        }

        Ok(())
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
    fn test_median_types() {
        let odd = vec![1, 3, 5];
        let even = vec![1, 2, 6, 7];

        let mut no_numbers = Numbers::new();
        let mut lone_numbers = Numbers::from(vec![8]);
        let mut odd_numbers = Numbers::from(odd);
        let mut even_numbers = Numbers::from(even);

        // Low
        assert_eq!(
            no_numbers.median_to_value(MedianType::Low),
            DynamicValue::None
        );

        assert_eq!(
            lone_numbers.median_to_value(MedianType::Low),
            DynamicValue::from(8)
        );

        assert_eq!(
            odd_numbers.median_to_value(MedianType::Low),
            DynamicValue::from(3)
        );

        assert_eq!(
            even_numbers.median_to_value(MedianType::Low),
            DynamicValue::from(2)
        );

        // High
        assert_eq!(
            no_numbers.median_to_value(MedianType::High),
            DynamicValue::None
        );

        assert_eq!(
            lone_numbers.median_to_value(MedianType::High),
            DynamicValue::from(8)
        );

        assert_eq!(
            odd_numbers.median_to_value(MedianType::High),
            DynamicValue::from(3)
        );

        assert_eq!(
            even_numbers.median_to_value(MedianType::High),
            DynamicValue::from(6)
        );

        // High
        assert_eq!(
            no_numbers.median_to_value(MedianType::Interpolation),
            DynamicValue::None
        );

        assert_eq!(
            lone_numbers.median_to_value(MedianType::Interpolation),
            DynamicValue::from(8)
        );

        assert_eq!(
            odd_numbers.median_to_value(MedianType::Interpolation),
            DynamicValue::from(3)
        );

        assert_eq!(
            even_numbers.median_to_value(MedianType::Interpolation),
            DynamicValue::from(4.0)
        );
    }
}
