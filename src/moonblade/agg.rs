use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::rc::Rc;

use csv::ByteRecord;

use super::error::{CallError, ConcretizationError, EvaluationError, SpecifiedCallError};
use super::interpreter::{concretize_expression, eval_expr, ConcreteArgument};
use super::parser::{parse_aggregations, Aggregation, Aggregations};
use super::types::{DynamicNumber, DynamicValue, HeadersIndex, Variables};

#[derive(Debug)]
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
}

#[derive(Debug)]
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
}

#[derive(Debug)]
struct FirstLast {
    first: Option<Rc<DynamicValue>>,
    last: Option<Rc<DynamicValue>>,
}

impl FirstLast {
    fn new() -> Self {
        Self {
            first: None,
            last: None,
        }
    }

    fn clear(&mut self) {
        self.first = None;
        self.last = None;
    }

    fn add(&mut self, next_value: &Rc<DynamicValue>) {
        if self.first.is_none() {
            self.first = Some(next_value.clone());
        }

        self.last = Some(next_value.clone());
    }

    fn first(&self) -> Option<DynamicValue> {
        self.first.as_ref().map(|p| p.as_ref().clone())
    }

    fn last(&self) -> Option<DynamicValue> {
        self.last.as_ref().map(|p| p.as_ref().clone())
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
}

#[derive(Debug)]
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
}

#[derive(Debug)]
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
}

#[derive(Debug)]
enum MedianType {
    Interpolation,
    Low,
    High,
}

#[derive(Debug)]
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

    // TODO: par_finalize
    fn finalize(&mut self) {
        self.numbers.sort_by(|a, b| a.partial_cmp(b).unwrap());
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
}

#[derive(Debug)]
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

    fn add(&mut self, value: String) {
        self.counter
            .entry(value)
            .and_modify(|count| *count += 1)
            .or_insert(1);
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
}

// NOTE: this is an implementation of Welford's online algorithm
#[derive(Debug)]
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
}

macro_rules! build_aggregation_method_enum {
    ($($variant: ident,)+) => {
        #[derive(Debug)]
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

            fn finalize(&mut self) {
                match self {
                    Self::Numbers(inner) => {
                        inner.finalize();
                    }
                    _ => (),
                }
            }
        }
    };
}

build_aggregation_method_enum!(
    AllAny,
    Count,
    Extent,
    FirstLast,
    LexicographicExtent,
    Frequencies,
    Numbers,
    Sum,
    Welford,
);

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

macro_rules! build_variant_methods {
    ($variant: ident, $has_name: ident, $gettr_name: ident) => {
        fn $has_name(&self) -> bool {
            self.methods.iter().any(|m| match m {
                Aggregator::$variant(_) => true,
                _ => false,
            })
        }

        fn $gettr_name(&self) -> &$variant {
            for method in self.methods.iter() {
                match method {
                    Aggregator::$variant(m) => return m,
                    _ => continue,
                }
            }

            unreachable!()
        }
    };
}

#[derive(Debug)]
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

    fn with_method(method: &ConcreteAggregationMethod) -> Self {
        let mut aggregator = Self::new();
        aggregator.add_method(method);
        aggregator
    }

    build_variant_methods!(AllAny, has_allany, get_allany);
    build_variant_methods!(Count, has_count, get_count);
    build_variant_methods!(Extent, has_extent, get_extent);
    build_variant_methods!(FirstLast, has_firstlast, get_firstlast);
    build_variant_methods!(
        LexicographicExtent,
        has_lexicographic_extent,
        get_lexicographic_extent
    );
    build_variant_methods!(Frequencies, has_frequencies, get_frequencies);
    build_variant_methods!(Numbers, has_numbers, get_numbers);
    build_variant_methods!(Sum, has_sum, get_sum);
    build_variant_methods!(Welford, has_welford, get_welford);

    fn add_method(&mut self, method: &ConcreteAggregationMethod) {
        match method {
            ConcreteAggregationMethod::All | ConcreteAggregationMethod::Any => {
                if self.has_allany() {
                    return;
                }

                self.methods.push(Aggregator::AllAny(AllAny::new()));
            }
            ConcreteAggregationMethod::Count => {
                if self.has_count() {
                    return;
                }

                self.methods.push(Aggregator::Count(Count::new()));
            }
            ConcreteAggregationMethod::Min | ConcreteAggregationMethod::Max => {
                if self.has_extent() {
                    return;
                }

                self.methods.push(Aggregator::Extent(Extent::new()));
            }
            ConcreteAggregationMethod::First | ConcreteAggregationMethod::Last => {
                if self.has_firstlast() {
                    return;
                }

                self.methods.push(Aggregator::FirstLast(FirstLast::new()));
            }
            ConcreteAggregationMethod::LexFirst | ConcreteAggregationMethod::LexLast => {
                if self.has_lexicographic_extent() {
                    return;
                }

                self.methods
                    .push(Aggregator::LexicographicExtent(LexicographicExtent::new()));
            }
            ConcreteAggregationMethod::Median(_) => {
                if !self.has_numbers() {
                    self.methods.push(Aggregator::Numbers(Numbers::new()));
                }
            }
            ConcreteAggregationMethod::Mode | ConcreteAggregationMethod::Cardinality => {
                if !self.has_frequencies() {
                    self.methods
                        .push(Aggregator::Frequencies(Frequencies::new()));
                }
            }
            ConcreteAggregationMethod::Sum => {
                if self.has_sum() {
                    return;
                }

                self.methods.push(Aggregator::Sum(Sum::new()));
            }
            ConcreteAggregationMethod::Mean
            | ConcreteAggregationMethod::VarPop
            | ConcreteAggregationMethod::VarSample
            | ConcreteAggregationMethod::StddevPop
            | ConcreteAggregationMethod::StddevSample => {
                if self.has_welford() {
                    return;
                }

                self.methods.push(Aggregator::Welford(Welford::new()));
            }
        }
    }

    fn process_value(&mut self, value_opt: Option<DynamicValue>) -> Result<(), CallError> {
        let value_opt = value_opt.map(Rc::new);

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
                    Aggregator::FirstLast(firstlast) => {
                        if !value.is_nullish() {
                            firstlast.add(value);
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

    fn finalize(&mut self) {
        for method in self.methods.iter_mut() {
            method.finalize();
        }
    }

    fn get_method_value(&self, method: &ConcreteAggregationMethod) -> DynamicValue {
        match method {
            ConcreteAggregationMethod::All => DynamicValue::from(self.get_allany().all()),
            ConcreteAggregationMethod::Any => DynamicValue::from(self.get_allany().any()),
            ConcreteAggregationMethod::Cardinality => {
                DynamicValue::from(self.get_frequencies().cardinality())
            }
            ConcreteAggregationMethod::Count => DynamicValue::from(self.get_count().get()),
            ConcreteAggregationMethod::First => DynamicValue::from(self.get_firstlast().first()),
            ConcreteAggregationMethod::Last => DynamicValue::from(self.get_firstlast().last()),
            ConcreteAggregationMethod::LexFirst => {
                DynamicValue::from(self.get_lexicographic_extent().first())
            }
            ConcreteAggregationMethod::LexLast => {
                DynamicValue::from(self.get_lexicographic_extent().last())
            }
            ConcreteAggregationMethod::Min => DynamicValue::from(self.get_extent().min()),
            ConcreteAggregationMethod::Mean => DynamicValue::from(self.get_welford().mean()),
            ConcreteAggregationMethod::Median(median_type) => {
                DynamicValue::from(self.get_numbers().median(median_type))
            }
            ConcreteAggregationMethod::Max => DynamicValue::from(self.get_extent().max()),
            ConcreteAggregationMethod::Mode => DynamicValue::from(self.get_frequencies().mode()),
            ConcreteAggregationMethod::Sum => DynamicValue::from(self.get_sum().get()),
            ConcreteAggregationMethod::VarPop => DynamicValue::from(self.get_welford().variance()),
            ConcreteAggregationMethod::VarSample => {
                DynamicValue::from(self.get_welford().sample_variance())
            }
            ConcreteAggregationMethod::StddevPop => DynamicValue::from(self.get_welford().stdev()),
            ConcreteAggregationMethod::StddevSample => {
                DynamicValue::from(self.get_welford().sample_stdev())
            }
        }
    }
}

// NOTE: the rationale of the `KeyedAggregator` is to make sure to group
// aggregations per expression. This means 'sum(A) as sum, mean(A)` will never
// need to run the expression twice and can share an `CompositeAggregator` allocation.
// TODO: deal with count() having no expr.
#[derive(Debug)]
struct KeyedAggregatorEntry {
    key: String,
    expr: Option<ConcreteArgument>,
    aggregator: CompositeAggregator,
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

    fn clear_inner_aggregators(&mut self) {
        for entry in self.mapping.iter_mut() {
            entry.aggregator.clear();
        }
    }

    fn get(&self, key: &str) -> Option<&KeyedAggregatorEntry> {
        self.mapping.iter().find(|entry| entry.key == key)
    }

    fn get_mut(&mut self, key: &str) -> Option<&mut KeyedAggregatorEntry> {
        self.mapping.iter_mut().find(|entry| entry.key == key)
    }

    fn add(&mut self, aggregation: &ConcreteAggregation) {
        match self.get_mut(&aggregation.expr_key) {
            None => {
                self.mapping.push(KeyedAggregatorEntry {
                    key: aggregation.expr_key.clone(),
                    expr: aggregation.expr.clone(),
                    aggregator: CompositeAggregator::with_method(&aggregation.method),
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
                None => None,
                Some(expr) => Some(eval_expr(expr, record, headers_index, variables)?),
            };

            entry.aggregator.process_value(value).map_err(|err| {
                EvaluationError::Call(SpecifiedCallError {
                    reason: err,
                    function_name: format!("<agg-expr: {}>", entry.key),
                })
            })?;
        }

        Ok(())
    }

    fn finalize(&mut self) {
        for entry in self.mapping.iter_mut() {
            entry.aggregator.finalize();
        }
    }

    fn get_value(&self, key: &str, method: &ConcreteAggregationMethod) -> Option<DynamicValue> {
        self.get(key)
            .map(|entry| entry.aggregator.get_method_value(method))
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

#[derive(Debug)]
enum ConcreteAggregationMethod {
    All,
    Any,
    Cardinality,
    Count,
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
    VarPop,
    VarSample,
    StddevPop,
    StddevSample,
}

impl ConcreteAggregationMethod {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "all" => Self::All,
            "any" => Self::Any,
            "cardinality" => Self::Cardinality,
            "count" => Self::Count,
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
            "var" | "var_pop" => Self::VarPop,
            "var_sample" => Self::VarSample,
            "stddev" | "stddev_pop" => Self::StddevPop,
            "stddev_sample" => Self::StddevSample,
            "sum" => Self::Sum,
            _ => return None,
        })
    }
}

#[derive(Debug)]
struct ConcreteAggregation {
    agg_name: String,
    method: ConcreteAggregationMethod,
    expr: Option<ConcreteArgument>,
    expr_key: String,
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
            .map(|arg| concretize_expression(arg.clone(), headers))
            .transpose()?;

        let mut args: Vec<ConcreteArgument> = Vec::new();

        for arg in aggregation.args.into_iter().skip(1) {
            args.push(concretize_expression(arg, headers)?);
        }

        if let Some(method) = ConcreteAggregationMethod::parse(&aggregation.func_name) {
            let concrete_aggregation = ConcreteAggregation {
                agg_name: aggregation.agg_name,
                method,
                expr_key: aggregation.expr_key,
                expr,
                // args,
            };

            concrete_aggregations.push(concrete_aggregation);
        } else {
            return Err(ConcretizationError::UnknownFunction(aggregation.func_name));
        }
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

    pub fn clear(&mut self) {
        self.aggregator.clear_inner_aggregators();
    }

    pub fn run_with_record(&mut self, record: &ByteRecord) -> Result<(), EvaluationError> {
        self.aggregator
            .run_with_record(record, &self.headers_index, &self.variables)
    }

    pub fn headers(&self) -> impl Iterator<Item = &[u8]> {
        self.aggregations.iter().map(|agg| agg.agg_name.as_bytes())
    }

    pub fn finalize(&mut self) -> ByteRecord {
        let mut record = ByteRecord::new();

        self.aggregator.finalize();

        for aggregation in self.aggregations.iter() {
            let value = self
                .aggregator
                .get_value(&aggregation.expr_key, &aggregation.method)
                .unwrap();

            record.push_field(&value.serialize_as_bytes(b"|"));
        }

        record
    }

    pub fn finalize_with_group(&mut self, group: &[u8]) -> ByteRecord {
        let mut record = ByteRecord::new();
        record.push_field(group);

        self.aggregator.finalize();

        for aggregation in self.aggregations.iter() {
            let value = self
                .aggregator
                .get_value(&aggregation.expr_key, &aggregation.method)
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

    pub fn headers_with_prepended_group_column(&self, group_column_name: &str) -> ByteRecord {
        let mut record = ByteRecord::new();

        record.push_field(group_column_name.as_bytes());

        for aggregation in self.aggregations.iter() {
            record.push_field(aggregation.agg_name.as_bytes());
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

            aggregator.finalize();

            for aggregation in self.aggregations.iter_mut() {
                let value = aggregator
                    .get_value(&aggregation.expr_key, &aggregation.method)
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

        no_numbers.finalize();
        lone_numbers.finalize();
        odd_numbers.finalize();
        even_numbers.finalize();

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
}
