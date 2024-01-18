use csv::ByteRecord;

use super::error::{CallError, EvaluationError, PrepareError, SpecifiedCallError};
use super::interpreter::{concretize_argument, eval_expr, ConcreteArgument};
use super::parser::{parse_aggregations, Aggregations};
use super::types::{DynamicNumber, DynamicValue, Variables};

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

    fn into_value(self) -> DynamicValue {
        DynamicValue::from(self.current)
    }

    fn into_float(self) -> f64 {
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
    fn add(&mut self, value: DynamicNumber) {
        self.current += value
    }

    fn into_value(self) -> DynamicValue {
        DynamicValue::from(self.current)
    }

    fn into_float(self) -> f64 {
        match self.current {
            DynamicNumber::Float(v) => v,
            DynamicNumber::Integer(v) => v as f64,
        }
    }
}

// TODO: ensure we only keep one aggregator per aggregation key, which also
// means each unique expression must only be evaluated once
// TODO: move to Aggregator enum with common interface? no because mean is a compound aggregator?
// TODO: aggregations must be grouped by expression key and then have dependencies (mean, sum, count for instance)
#[derive(Debug)]
struct Aggregator {
    count: Option<Count>,
    sum: Option<Sum>,
}

impl Aggregator {
    pub fn from_method(method: &str) -> Self {
        let mut count = None;
        let mut sum = None;

        if method == "count" || method == "mean" {
            count = Some(Count::new());
        }

        if method == "sum" || method == "mean" {
            sum = Some(Sum::new());
        }

        Self { count, sum }
    }

    pub fn add(&mut self, value: DynamicValue) -> Result<(), CallError> {
        if let Some(ref mut count) = self.count {
            count.add();
        }

        if let Some(ref mut sum) = self.sum {
            sum.add(value.try_as_number()?);
        }

        Ok(())
    }

    pub fn finalize(self, method: &str) -> DynamicValue {
        match method {
            "count" => self.count.unwrap().into_value(),
            "mean" => {
                let count = self.count.unwrap().into_float();
                let sum = self.sum.unwrap().into_float();

                DynamicValue::from(sum / count)
            }
            "sum" => self.sum.unwrap().into_value(),
            _ => unreachable!(),
        }
    }
}

// TODO: validate agg arity
#[derive(Debug)]
pub struct ConcreteAggregation {
    name: String,
    pub method: String,
    expr: Option<ConcreteArgument>,
    // args: Vec<ConcreteArgument>,
}

pub type ConcreteAggregations = Vec<ConcreteAggregation>;

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
            expr,
            // args,
        };

        concrete_aggregations.push(concrete_aggregation);
    }

    Ok(concrete_aggregations)
}

#[derive(Debug)]
pub struct AggregationProgram<'a> {
    aggregations: ConcreteAggregations,
    aggregators: Vec<Aggregator>,
    variables: Variables<'a>,
}

impl<'a> AggregationProgram<'a> {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, PrepareError> {
        let parsed_aggregations =
            parse_aggregations(code).map_err(|_| PrepareError::ParseError(code.to_string()))?;
        let concrete_aggregations = concretize_aggregations(parsed_aggregations, headers)?;
        let aggregators = concrete_aggregations
            .iter()
            .map(|agg| Aggregator::from_method(&agg.method))
            .collect();

        Ok(AggregationProgram {
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

            aggregator.add(value).map_err(|err| {
                EvaluationError::Call(SpecifiedCallError {
                    reason: err,
                    function_name: aggregation.method.to_string(),
                })
            })?;
        }

        Ok(())
    }

    pub fn headers(&self) -> ByteRecord {
        let mut record = ByteRecord::new();

        for aggregation in self.aggregations.iter() {
            record.push_field(aggregation.name.as_bytes());
        }

        record
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
                    .finalize(&aggregation.method)
                    .serialize_as_bytes(b"|"),
            );
        }

        record
    }
}
