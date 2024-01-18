use super::error::CallError;
use super::interpreter::ConcreteAggregation;
use super::types::{DynamicNumber, DynamicValue};

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

    fn finalize(self) -> DynamicValue {
        DynamicValue::from(self.current)
    }
}

// TODO: ensure we only keep one aggregator per aggregation key, which also
// means each unique expression must only be evaluated once
// TODO: move to Aggregator enum with common interface? no because mean is a compound aggregator?
struct Aggregator {
    sum: Option<Sum>,
}

impl Aggregator {
    pub fn from_concrete_aggregation(aggregation: &ConcreteAggregation) -> Self {
        let mut sum = None;

        if aggregation.method == "sum" || aggregation.method == "mean" {
            sum = Some(Sum::new());
        }

        Self { sum }
    }

    pub fn add(&mut self, value: DynamicValue) -> Result<(), CallError> {
        if let Some(ref mut sum) = self.sum {
            sum.add(value.try_as_number()?);
        }

        Ok(())
    }

    pub fn finalize(self, aggregation: &ConcreteAggregation) -> DynamicValue {
        if aggregation.method == "sum" {
            return self.sum.unwrap().finalize();
        }

        unreachable!();
    }
}
