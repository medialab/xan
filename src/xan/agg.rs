use std::collections::HashMap;

use csv::ByteRecord;

use super::error::{CallError, EvaluationError, PrepareError, SpecifiedCallError};
use super::interpreter::{concretize_argument, eval_expr, ConcreteArgument};
use super::parser::{parse_aggregations, Aggregations};
use super::types::{DynamicNumber, DynamicValue, Variables};

// TODO: test when there is no data to be aggregated at all
// TODO: test with nulls and nans
// TODO: parallelize multiple aggregations
// TODO: ensure we only keep one aggregator per aggregation key, which also
// means each unique expression must only be evaluated once
// TODO: move to Aggregator enum with common interface? no because mean is a compound aggregator?
// TODO: aggregations must be grouped by expression key and then have dependencies (mean, sum, count for instance)
// TODO: validate agg arity
// TODO: we need some clear method to enable sorted group by optimization

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

#[derive(Debug)]
enum AggregationMethod {
    Count(Count),
    Extent(Extent),
    Sum(Sum),
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

    fn is_sum(&self) -> bool {
        match self {
            Self::Sum(_) => true,
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

    fn has_sum(&self) -> bool {
        self.methods.iter().any(|method| method.is_sum())
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

    fn get_sum(&self) -> Option<&Sum> {
        for method in self.methods.iter() {
            match method {
                AggregationMethod::Sum(sum) => return Some(sum),
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
            "mean" => {
                if !self.has_count() {
                    self.methods.push(AggregationMethod::Count(Count::new()));
                }
                if !self.has_sum() {
                    self.methods.push(AggregationMethod::Sum(Sum::new()));
                }
            }
            "sum" => {
                if self.has_sum() {
                    return;
                }

                self.methods.push(AggregationMethod::Sum(Sum::new()));
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
                AggregationMethod::Sum(sum) => {
                    sum.add(&value.try_as_number()?);
                }
            }
        }

        Ok(())
    }

    fn finalize_method(&self, method: &str) -> DynamicValue {
        match method {
            "count" => self.get_count().unwrap().to_value(),
            "min" => self.get_extent().unwrap().min_to_value(),
            "mean" => {
                let count = self.get_count().unwrap().to_float();
                let sum = self.get_sum().unwrap().to_float();

                DynamicValue::from(sum / count)
            }
            "max" => self.get_extent().unwrap().max_to_value(),
            "sum" => self.get_sum().unwrap().to_value(),
            _ => unreachable!(),
        }
    }
}

// #[derive(Debug)]
// struct KeyedAggregator {
//     mapping: BTreeMap<String, Aggregator>,
// }

// impl KeyedAggregator {
//     fn new() -> Self {
//         Self {
//             mapping: BTreeMap::new(),
//         }
//     }

//     fn add(&mut self, aggregation: &ConcreteAggregation) {
//         self.mapping
//             .entry(aggregation.key.clone())
//             .and_modify(|aggregator| {
//                 aggregator.add_method(&aggregation.method);
//             })
//             .or_insert_with(|| Aggregator::with_method(&aggregation.method));
//     }
// }

#[derive(Debug)]
struct ConcreteAggregation {
    name: String,
    method: String,
    expr: Option<ConcreteArgument>,
    // key: String,
    // args: Vec<ConcreteArgument>,
}

type ConcreteAggregations = Vec<ConcreteAggregation>;

fn concretize_aggregations(
    aggregations: Aggregations,
    headers: &ByteRecord,
) -> Result<ConcreteAggregations, PrepareError> {
    let mut concrete_aggregations = ConcreteAggregations::new();

    for aggregation in aggregations {
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
            // key: aggregation.key,
            expr,
            // args,
        };

        concrete_aggregations.push(concrete_aggregation);
    }

    Ok(concrete_aggregations)
}

fn prepare(code: &str, headers: &ByteRecord) -> Result<ConcreteAggregations, PrepareError> {
    let parsed_aggregations =
        parse_aggregations(code).map_err(|_| PrepareError::ParseError(code.to_string()))?;

    concretize_aggregations(parsed_aggregations, headers)
}

fn headers_from_concrete_aggregations(aggregations: &ConcreteAggregations) -> ByteRecord {
    let mut record = ByteRecord::new();

    for aggregation in aggregations.iter() {
        record.push_field(aggregation.name.as_bytes());
    }

    record
}

#[derive(Debug)]
pub struct AggregationProgram<'a> {
    aggregations: ConcreteAggregations,
    aggregators: Vec<Aggregator>,
    variables: Variables<'a>,
}

impl<'a> AggregationProgram<'a> {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, PrepareError> {
        let concrete_aggregations = prepare(code, headers)?;

        let aggregators = concrete_aggregations
            .iter()
            .map(|agg| Aggregator::with_method(&agg.method))
            .collect();

        Ok(Self {
            aggregations: concrete_aggregations,
            aggregators,
            variables: Variables::new(),
        })
    }

    pub fn run_with_record(&mut self, record: &ByteRecord) -> Result<(), EvaluationError> {
        for (aggregation, aggregator) in self.aggregations.iter().zip(self.aggregators.iter_mut()) {
            // NOTE: count tolerates having no expression to evaluate, for instance.
            let value = match &aggregation.expr {
                None => DynamicValue::None,
                Some(expr) => eval_expr(expr, record, &self.variables)?,
            };

            aggregator.process_value(value).map_err(|err| {
                EvaluationError::Call(SpecifiedCallError {
                    reason: err,
                    function_name: aggregation.method.to_string(),
                })
            })?;
        }

        Ok(())
    }

    pub fn headers(&self) -> ByteRecord {
        headers_from_concrete_aggregations(&self.aggregations)
    }

    pub fn finalize(self) -> ByteRecord {
        let mut record = ByteRecord::new();

        for (aggregation, aggregator) in self
            .aggregations
            .into_iter()
            .zip(self.aggregators.into_iter())
        {
            record.push_field(
                &aggregator
                    .finalize_method(&aggregation.method)
                    .serialize_as_bytes(b"|"),
            );
        }

        record
    }
}

#[derive(Debug)]
pub struct GroupAggregationProgram<'a> {
    aggregations: ConcreteAggregations,
    groups: HashMap<Vec<u8>, Vec<Aggregator>>,
    variables: Variables<'a>,
}
