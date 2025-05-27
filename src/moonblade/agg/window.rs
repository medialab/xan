use std::collections::VecDeque;

use csv::ByteRecord;

use super::aggregators::Sum;
use crate::moonblade::error::{ConcretizationError, SpecifiedEvaluationError};
use crate::moonblade::interpreter::{concretize_expression, eval_expression, ConcreteExpr};
use crate::moonblade::parser::parse_aggregations;
use crate::moonblade::types::{DynamicNumber, DynamicValue, FunctionArguments, HeadersIndex};

#[derive(Debug)]
struct RollingSum {
    buffer: VecDeque<DynamicNumber>,
    window_size: usize,
    sum: Sum,
}

impl RollingSum {
    fn with_window_size(window_size: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(window_size),
            window_size,
            sum: Sum::new(),
        }
    }

    fn add(&mut self, number: DynamicNumber) -> Option<DynamicNumber> {
        if self.buffer.len() == self.window_size {
            self.sum.add(-self.buffer.pop_front().unwrap());
            self.sum.add(number);
            self.buffer.push_back(number);

            self.sum.get()
        } else {
            self.buffer.push_back(number);
            self.sum.add(number);

            if self.buffer.len() == self.window_size {
                self.sum.get()
            } else {
                None
            }
        }
    }

    fn clear(&mut self) {
        self.buffer.clear();
        self.sum.clear();
    }
}

#[derive(Debug)]
enum ConcreteWindowAggregation {
    Lead(ConcreteExpr, usize),
    Lag(ConcreteExpr, usize),
    RowNumber(usize),
    CumulativeSum(ConcreteExpr, Sum),
    RollingSum(ConcreteExpr, RollingSum),
}

impl ConcreteWindowAggregation {
    fn extent(&self) -> (usize, usize) {
        match self {
            Self::Lead(_, n) => (0, *n),
            Self::Lag(_, n) => (*n, 0),
            _ => (0, 0),
        }
    }

    fn run(
        &mut self,
        index: usize,
        record: &ByteRecord,
        headers_index: &HeadersIndex,
        past_buffer: Option<&Buffer>,
        future_buffer: Option<&Buffer>,
    ) -> Result<DynamicValue, SpecifiedEvaluationError> {
        match self {
            Self::Lag(expr, n) => {
                let past_buffer = past_buffer.unwrap();

                match past_buffer.get(*n - 1) {
                    None => Ok(DynamicValue::None),
                    Some((past_index, past_record)) => {
                        let value =
                            eval_expression(expr, Some(*past_index), past_record, headers_index)?;

                        Ok(value)
                    }
                }
            }
            Self::Lead(expr, n) => match future_buffer.unwrap().get(*n) {
                None => Ok(DynamicValue::None),
                Some((future_index, future_record)) => {
                    let value =
                        eval_expression(expr, Some(*future_index), future_record, headers_index)?;

                    Ok(value)
                }
            },
            Self::RowNumber(counter) => {
                *counter += 1;
                Ok(DynamicValue::from(*counter))
            }
            Self::CumulativeSum(expr, sum) => {
                let value = eval_expression(expr, Some(index), record, headers_index)?;

                sum.add(value.try_as_number().map_err(|err| err.anonymous())?);

                Ok(DynamicValue::from(sum.get()))
            }
            Self::RollingSum(expr, sum) => {
                let value = eval_expression(expr, Some(index), record, headers_index)?;
                let number = value.try_as_number().map_err(|err| err.anonymous())?;

                Ok(DynamicValue::from(sum.add(number)))
            }
        }
    }

    fn clear(&mut self) {
        match self {
            Self::RowNumber(counter) => {
                *counter = 0;
            }
            Self::CumulativeSum(_, sum) => {
                sum.clear();
            }
            Self::RollingSum(_, sum) => {
                sum.clear();
            }
            Self::Lag(_, _) | Self::Lead(_, _) => (),
        };
    }
}

fn get_function(name: &str) -> Option<FunctionArguments> {
    Some(match name {
        "row_number" => FunctionArguments::nullary(),
        "lag" | "lead" => FunctionArguments::with_range(1..=2),
        "cumsum" => FunctionArguments::unary(),
        "rolling_sum" => FunctionArguments::binary(),
        _ => return None,
    })
}

fn cast_as_usize(arg: &ConcreteExpr) -> Result<usize, ConcretizationError> {
    match arg {
        ConcreteExpr::Value(v) => v
            .try_as_usize()
            .map_err(|_| ConcretizationError::NotStaticallyAnalyzable),
        _ => Err(ConcretizationError::NotStaticallyAnalyzable),
    }
}

type ConcreteWindowAggregations = Vec<(String, ConcreteWindowAggregation)>;

fn concretize_window_aggregations(
    input: &str,
    headers: &ByteRecord,
) -> Result<ConcreteWindowAggregations, ConcretizationError> {
    let aggs = parse_aggregations(input).map_err(ConcretizationError::ParseError)?;

    let mut concrete_aggs = Vec::with_capacity(aggs.len());

    for mut agg in aggs {
        let func_name = &agg.func_name;

        let arguments_spec = get_function(func_name)
            .ok_or_else(|| ConcretizationError::UnknownFunction(func_name.to_string()))?;

        arguments_spec
            .validate_arity(agg.args.len())
            .map_err(|invalid_arity| {
                ConcretizationError::InvalidArity(func_name.to_string(), invalid_arity)
            })?;

        match func_name.as_str() {
            "row_number" => {
                concrete_aggs.push((agg.agg_name, ConcreteWindowAggregation::RowNumber(0)));
            }
            "lead" | "lag" => {
                let n = if agg.args.len() == 1 {
                    1
                } else {
                    cast_as_usize(&concretize_expression(
                        agg.args.pop().unwrap(),
                        headers,
                        None,
                    )?)?
                };

                let expr = concretize_expression(agg.args.pop().unwrap(), headers, None)?;

                let concrete_agg = if func_name == "lead" {
                    ConcreteWindowAggregation::Lead(expr, n)
                } else {
                    ConcreteWindowAggregation::Lag(expr, n)
                };

                concrete_aggs.push((agg.agg_name, concrete_agg));
            }
            "cumsum" => {
                let expr = concretize_expression(agg.args.pop().unwrap(), headers, None)?;

                concrete_aggs.push((
                    agg.agg_name,
                    ConcreteWindowAggregation::CumulativeSum(expr, Sum::new()),
                ))
            }
            "rolling_sum" => {
                let expr = concretize_expression(agg.args.pop().unwrap(), headers, None)?;
                let window_size = cast_as_usize(&concretize_expression(
                    agg.args.pop().unwrap(),
                    headers,
                    None,
                )?)?;

                concrete_aggs.push((
                    agg.agg_name,
                    ConcreteWindowAggregation::RollingSum(
                        expr,
                        RollingSum::with_window_size(window_size),
                    ),
                ));
            }
            _ => unreachable!(),
        };
    }

    Ok(concrete_aggs)
}

fn find_buffer_extent(aggs: &ConcreteWindowAggregations) -> (usize, usize) {
    let mut extent = (0, 0);

    for (_, agg) in aggs {
        let current_extent = agg.extent();

        if current_extent.0 > extent.0 {
            extent.0 = current_extent.0;
        }

        if current_extent.1 > extent.1 {
            extent.1 = current_extent.1;
        }
    }

    extent
}

type Buffer = VecDeque<(usize, ByteRecord)>;

#[derive(Debug)]
pub struct WindowAggregationProgram {
    aggs: ConcreteWindowAggregations,
    headers_index: HeadersIndex,
    past_buffer: Option<(usize, Buffer)>,
    future_buffer: Option<(usize, Buffer)>,
    output_buffer: Vec<DynamicValue>,
}

impl WindowAggregationProgram {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let aggs = concretize_window_aggregations(code, headers)?;

        let (max_past, max_future) = find_buffer_extent(&aggs);

        let past_buffer = (max_past > 0).then(|| (max_past, VecDeque::with_capacity(max_past)));
        let future_buffer =
            (max_future > 0).then(|| (max_future + 1, VecDeque::with_capacity(max_future + 1)));

        Ok(Self {
            aggs,
            output_buffer: Vec::with_capacity(headers.len()),
            headers_index: HeadersIndex::from_headers(headers),
            past_buffer,
            future_buffer,
        })
    }

    pub fn headers(&self) -> impl Iterator<Item = &[u8]> {
        self.aggs.iter().map(|(name, _)| name.as_bytes())
    }

    pub fn run_with_record(
        &mut self,
        index: usize,
        record: &ByteRecord,
    ) -> Result<Option<ByteRecord>, SpecifiedEvaluationError> {
        if let Some((future_capacity, future_buffer)) = &mut self.future_buffer {
            if future_buffer.len() < *future_capacity {
                future_buffer.push_back((index, record.clone()));

                return Ok(None);
            }
        }

        self.output_buffer.clear();

        let past_buffer_ref = self.past_buffer.as_ref().map(|(_, b)| b);
        let future_buffer_ref = self.future_buffer.as_ref().map(|(_, b)| b);

        for (_, agg) in self.aggs.iter_mut() {
            let (working_index, working_record) =
                if let Some((_, future_buffer)) = &self.future_buffer {
                    let (i, r) = future_buffer.front().unwrap();

                    (*i, r)
                } else {
                    (index, record)
                };

            self.output_buffer.push(agg.run(
                working_index,
                working_record,
                &self.headers_index,
                past_buffer_ref,
                future_buffer_ref,
            )?);
        }

        let record_to_emit = if let Some((_, future_buffer)) = &mut self.future_buffer {
            let r = future_buffer.pop_front();
            future_buffer.push_back((index, record.clone()));
            &r.unwrap().1
        } else {
            record
        };

        if let Some((past_capacity, past_buffer)) = &mut self.past_buffer {
            if past_buffer.len() >= *past_capacity {
                past_buffer.pop_back();
            }

            past_buffer.push_front((index, record_to_emit.clone()));
        }

        let mut output_record = record_to_emit.clone();

        for value in self.output_buffer.iter() {
            output_record.push_field(&value.serialize_as_bytes());
        }

        Ok(Some(output_record))
    }

    pub fn flush(&mut self) -> Result<Vec<ByteRecord>, SpecifiedEvaluationError> {
        if let Some((_, future_buffer)) = self.future_buffer.as_mut() {
            let padding = (0..self.output_buffer.len())
                .map(|_| b"")
                .collect::<csv::ByteRecord>();

            let mut output = Vec::new();

            for _ in 0..future_buffer.len() {
                output.push(self.run_with_record(0, &padding)?.unwrap());
            }

            return Ok(output);
        }

        Ok(vec![])
    }

    pub fn flush_and_clear(&mut self) -> Result<Vec<ByteRecord>, SpecifiedEvaluationError> {
        let records = self.flush()?;

        if let Some((_, past_buffer)) = &mut self.past_buffer {
            past_buffer.clear();
        }

        if let Some((_, future_buffer)) = &mut self.future_buffer {
            future_buffer.clear();
        }

        for (_, agg) in &mut self.aggs {
            agg.clear();
        }

        Ok(records)
    }
}
