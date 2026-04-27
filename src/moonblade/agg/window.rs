use std::collections::VecDeque;

use simd_csv::ByteRecord;

use super::aggregators::{Sum, Welford};
use super::program::{concretize_aggregations, is_agg_fn_name, AggregationProgram};
use crate::moonblade::error::{ConcretizationError, SpecifiedEvaluationError};
use crate::moonblade::interpreter::{concretize_expression, ConcreteExpr, EvaluationContext};
use crate::moonblade::parser::parse_aggregations;
use crate::moonblade::types::{DynamicNumber, DynamicValue, FunctionArguments, HeadersIndex};

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Copy)]
enum WelfordStat {
    Mean,
    Var,
    Stddev,
}

#[derive(Debug, Clone)]
struct RollingWelford {
    buffer: VecDeque<f64>,
    window_size: usize,
    welford: Welford,
}

impl RollingWelford {
    fn with_window_size(window_size: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(window_size),
            window_size,
            welford: Welford::new(),
        }
    }

    fn get(&self, stat: WelfordStat) -> Option<f64> {
        if self.buffer.len() < self.window_size {
            return None;
        }

        match stat {
            WelfordStat::Mean => self.welford.mean(),
            WelfordStat::Var => self.welford.variance(),
            WelfordStat::Stddev => self.welford.stdev(),
        }
    }

    fn add(&mut self, new_value: f64, stat: WelfordStat) -> Option<f64> {
        if self.buffer.len() == self.window_size {
            self.welford
                .roll(new_value, self.buffer.pop_front().unwrap());
            self.buffer.push_back(new_value);

            self.get(stat)
        } else {
            self.buffer.push_back(new_value);
            self.welford.add(new_value);

            if self.buffer.len() == self.window_size {
                self.get(stat)
            } else {
                None
            }
        }
    }

    fn clear(&mut self) {
        self.buffer.clear();
        self.welford.clear();
    }
}

#[derive(Debug, Clone)]
enum RankingKind {
    Arbitrary,
    Dense,
    CumulativeDistribution,
    PercentRank,
    NTile(usize),
}

impl RankingKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Arbitrary => "rank",
            Self::Dense => "dense_rank",
            Self::CumulativeDistribution => "cume_dist",
            Self::PercentRank => "percent_rank",
            Self::NTile(_) => "ntile",
        }
    }
}

#[derive(Debug, Clone)]
struct Ranking {
    kind: RankingKind,
    expr: ConcreteExpr,
    numbers: Vec<(DynamicNumber, usize)>,
    output: VecDeque<DynamicNumber>,
}

impl Ranking {
    fn new(kind: RankingKind, expr: ConcreteExpr) -> Self {
        Self {
            kind,
            expr,
            numbers: Vec::new(),
            output: VecDeque::new(),
        }
    }

    fn clear(&mut self) {
        self.numbers.clear();
        self.output.clear();
    }
}

#[derive(Debug, Clone)]
enum ConcreteWindowAggregation {
    Lead(ConcreteExpr, usize, ConcreteExpr),
    Lag(ConcreteExpr, usize, ConcreteExpr),
    FrontCoding(ConcreteExpr, Option<String>),
    RowNumber(usize),
    RowIndex(usize),
    CumulativeSum(ConcreteExpr, Sum),
    CumulativeMin(ConcreteExpr, Option<DynamicNumber>),
    CumulativeMax(ConcreteExpr, Option<DynamicNumber>),
    RollingSum(ConcreteExpr, RollingSum),
    RollingWelford(ConcreteExpr, WelfordStat, RollingWelford),
    Frac(ConcreteExpr, Sum, Option<usize>),
    Ranking(Ranking),
    TotalAggregation(AggregationProgram, DynamicValue),
}

fn eval_expression_to_number(
    expr: &ConcreteExpr,
    row_index: usize,
    col_index: Option<usize>,
    record: &ByteRecord,
    headers_index: &HeadersIndex,
) -> Result<DynamicNumber, SpecifiedEvaluationError> {
    let value =
        EvaluationContext::new_with_col_index(Some(row_index), col_index, record, headers_index)
            .evaluate(expr)?;

    value.try_as_number().map_err(|err| err.anonymous())
}

impl ConcreteWindowAggregation {
    fn extent(&self) -> (usize, usize) {
        match self {
            Self::Lead(_, n, _) => (0, *n),
            Self::Lag(_, n, _) => (*n, 0),
            _ => (0, 0),
        }
    }

    fn requires_total_buffer(&self) -> bool {
        matches!(
            self,
            Self::Frac(_, _, _) | Self::Ranking(_) | Self::TotalAggregation(_, _)
        )
    }

    fn aggregate_total(
        &mut self,
        row_index: usize,
        col_index: Option<usize>,
        record: &ByteRecord,
        headers_index: &HeadersIndex,
    ) -> Result<(), SpecifiedEvaluationError> {
        match self {
            Self::Frac(expr, sum, _) => {
                let value = EvaluationContext::new_with_col_index(
                    Some(row_index),
                    col_index,
                    record,
                    headers_index,
                )
                .evaluate(expr)?;

                if !value.is_nullish() {
                    sum.add(value.try_as_number().map_err(|err| err.specify("frac"))?);
                }
            }
            Self::Ranking(Ranking {
                expr,
                numbers,
                kind,
                ..
            }) => {
                let value = EvaluationContext::new_with_col_index(
                    Some(row_index),
                    col_index,
                    record,
                    headers_index,
                )
                .evaluate(expr)?;
                let number = value
                    .try_as_number()
                    .map_err(|err| err.specify(kind.as_str()))?;

                numbers.push((number, numbers.len()));
            }
            Self::TotalAggregation(program, _) => {
                match col_index {
                    Some(i) => program.run_with_col_index(row_index, i, record)?,
                    None => program.run_with_record(row_index, record)?,
                };
            }
            _ => (),
        };

        Ok(())
    }

    fn finalize_total(&mut self) -> Result<(), SpecifiedEvaluationError> {
        match self {
            Self::Ranking(Ranking {
                numbers,
                output,
                kind,
                ..
            }) => {
                numbers.sort();
                output.resize(numbers.len(), DynamicNumber::Integer(0));

                let n = numbers.len();

                match kind {
                    RankingKind::Dense => {
                        let mut rank: usize = 0;
                        let mut last_number: Option<DynamicNumber> = None;

                        for (n, i) in numbers.iter() {
                            match last_number {
                                None => {
                                    last_number = Some(*n);
                                    rank += 1;
                                }
                                Some(l) if l != *n => {
                                    last_number = Some(*n);
                                    rank += 1;
                                }
                                _ => {}
                            };

                            output[*i] = DynamicNumber::Integer(rank as i64);
                        }
                    }
                    RankingKind::Arbitrary => {
                        let mut rank: usize = 1;

                        for (_, i) in numbers.iter() {
                            output[*i] = DynamicNumber::Integer(rank as i64);
                            rank += 1;
                        }
                    }
                    RankingKind::CumulativeDistribution => {
                        let mut i: usize = 0;

                        while i < n {
                            let mut j = i + 1;

                            while j < n && numbers[i].0 == numbers[j].0 {
                                j += 1;
                            }

                            let c = j as f64 / n as f64;

                            for k in i..j {
                                output[numbers[k].1] = DynamicNumber::Float(c);
                            }

                            i = j;
                        }
                    }
                    RankingKind::PercentRank => {
                        let mut rank: usize = 1;
                        let mut i: usize = 0;

                        // Avoiding division by zero
                        if n > 1 {
                            while i < n {
                                let mut j = i + 1;

                                while j < n && numbers[i].0 == numbers[j].0 {
                                    j += 1;
                                }

                                let p = (rank - 1) as f64 / (n - 1) as f64;

                                for k in i..j {
                                    output[numbers[k].1] = DynamicNumber::Float(p);
                                }

                                rank += j - i;
                                i = j;
                            }
                        }
                    }
                    RankingKind::NTile(k) => {
                        let k = *k;

                        let q = n / k;
                        let r = n % k;

                        let mut i: usize = 0;

                        'main: for tile in 1..k + 1 {
                            let size = q + (if tile <= r { 1 } else { 0 });

                            for _ in 0..size {
                                if i >= n {
                                    break 'main;
                                }

                                output[numbers[i].1] = DynamicNumber::Integer(tile as i64);
                                i += 1;
                            }
                        }
                    }
                }
            }
            Self::TotalAggregation(program, value) => {
                *value = program.finalize_iter(false).next().unwrap()?
            }
            _ => (),
        }

        Ok(())
    }

    fn run(
        &mut self,
        index: usize,
        col_index: Option<usize>,
        record: &ByteRecord,
        headers_index: &HeadersIndex,
        past_buffer: Option<&PastBuffer>,
        future_buffer: Option<&FutureBuffer>,
    ) -> Result<DynamicValue, SpecifiedEvaluationError> {
        match self {
            Self::Lag(expr, n, default) => {
                let past_buffer = past_buffer.unwrap();

                match past_buffer.get(*n - 1) {
                    None => Ok(EvaluationContext::new_with_col_index(
                        Some(index),
                        col_index,
                        record,
                        headers_index,
                    )
                    .evaluate(default)?),
                    Some((past_row_index, past_col_index, past_record)) => {
                        let value = EvaluationContext::new_with_col_index(
                            Some(*past_row_index),
                            *past_col_index,
                            past_record,
                            headers_index,
                        )
                        .evaluate(expr)?;

                        Ok(value)
                    }
                }
            }
            Self::Lead(expr, n, default) => {
                let (future_row_index, future_col_index, future_record, is_padding) =
                    future_buffer.unwrap().get(*n).unwrap();

                let expr = if *is_padding { default } else { expr };

                let value = EvaluationContext::new_with_col_index(
                    Some(*future_row_index),
                    *future_col_index,
                    future_record,
                    headers_index,
                )
                .evaluate(expr)?;

                Ok(value)
            }
            Self::FrontCoding(expr, last_string_opt) => {
                let value = EvaluationContext::new_with_col_index(
                    Some(index),
                    col_index,
                    record,
                    headers_index,
                )
                .evaluate(expr)?;
                let string = value
                    .try_as_str()
                    .map_err(|err| err.anonymous())?
                    .into_owned();

                fn first_mismatch_index(a: &str, b: &str) -> usize {
                    for (i, (ca, cb)) in a.chars().zip(b.chars()).enumerate() {
                        if i == b.len() || ca != cb {
                            return i;
                        }
                    }

                    a.len()
                }

                // TODO: try_into_string
                match last_string_opt {
                    None => {
                        let result = DynamicValue::from("0 ".to_string() + &string);

                        *last_string_opt = Some(string);

                        Ok(result)
                    }
                    Some(last_string) => {
                        let i = first_mismatch_index(&string, last_string);

                        let result = DynamicValue::from(format!("{} ", i) + &string[i..]);

                        *last_string_opt = Some(string);

                        Ok(result)
                    }
                }
            }
            Self::RowNumber(counter) => {
                *counter += 1;
                Ok(DynamicValue::from(*counter))
            }
            Self::RowIndex(counter) => {
                let idx = *counter;
                *counter += 1;
                Ok(DynamicValue::from(idx))
            }
            Self::CumulativeSum(expr, sum) => {
                let number =
                    eval_expression_to_number(expr, index, col_index, record, headers_index)?;

                sum.add(number);

                Ok(DynamicValue::from(sum.get()))
            }
            Self::CumulativeMin(expr, min) => {
                let number =
                    eval_expression_to_number(expr, index, col_index, record, headers_index)?;

                match min {
                    None => *min = Some(number),
                    Some(current) => {
                        if number < *current {
                            *current = number;
                        }
                    }
                };

                Ok(DynamicValue::from(*min))
            }
            Self::CumulativeMax(expr, max) => {
                let number =
                    eval_expression_to_number(expr, index, col_index, record, headers_index)?;

                match max {
                    None => *max = Some(number),
                    Some(current) => {
                        if number > *current {
                            *current = number;
                        }
                    }
                };

                Ok(DynamicValue::from(*max))
            }
            Self::RollingSum(expr, sum) => {
                let number =
                    eval_expression_to_number(expr, index, col_index, record, headers_index)?;

                Ok(DynamicValue::from(sum.add(number)))
            }
            Self::RollingWelford(expr, stat, welford) => {
                let value = EvaluationContext::new_with_col_index(
                    Some(index),
                    col_index,
                    record,
                    headers_index,
                )
                .evaluate(expr)?;
                let float = value.try_as_f64().map_err(|err| err.anonymous())?;

                Ok(DynamicValue::from(welford.add(float, *stat)))
            }
            Self::Frac(expr, sum, decimals) => {
                // NOTE: we are evaluation the expression twice, because it seems less costly
                // than allocating a cache for every record.
                let value = EvaluationContext::new_with_col_index(
                    Some(index),
                    col_index,
                    record,
                    headers_index,
                )
                .evaluate(expr)?;

                if value.is_nullish() {
                    return Ok(DynamicValue::None);
                }

                let number = value.try_as_number().map_err(|err| err.specify("frac"))?;
                let frac = sum.get().map(|s| number / s);

                Ok(match decimals {
                    None => DynamicValue::from(frac),
                    Some(d) => DynamicValue::from(frac.map(|f| format!("{:.p$}", f, p = d))),
                })
            }
            Self::Ranking(Ranking { output, .. }) => {
                Ok(DynamicValue::from(output.pop_front().unwrap()))
            }
            Self::TotalAggregation(_, value) => Ok(value.clone()),
        }
    }

    fn clear(&mut self) {
        match self {
            Self::RowNumber(counter) | Self::RowIndex(counter) => {
                *counter = 0;
            }
            Self::CumulativeSum(_, sum) => {
                sum.clear();
            }
            Self::CumulativeMin(_, min) => {
                *min = None;
            }
            Self::CumulativeMax(_, max) => {
                *max = None;
            }
            Self::RollingSum(_, sum) => {
                sum.clear();
            }
            Self::RollingWelford(_, _, welford) => {
                welford.clear();
            }
            Self::Lag(_, _, _) | Self::Lead(_, _, _) => (),
            Self::FrontCoding(_, string) => {
                *string = None;
            }
            Self::Frac(_, sum, _) => {
                sum.clear();
            }
            Self::Ranking(ranking) => {
                ranking.clear();
            }
            Self::TotalAggregation(program, value) => {
                program.clear();
                *value = DynamicValue::None;
            }
        };
    }
}

fn get_function(name: &str) -> Option<FunctionArguments> {
    Some(match name {
        "row_number" | "row_index" => FunctionArguments::nullary(),
        "frac" => FunctionArguments::with_range(1..=2),
        "lag" | "lead" => FunctionArguments::with_range(1..=3),
        "cumsum" | "cummin" | "cummax" | "dense_rank" | "rank" | "cume_dist" | "percent_rank"
        | "front_coding" => FunctionArguments::unary(),
        "rolling_sum" | "rolling_mean" | "rolling_avg" | "rolling_var" | "rolling_stddev"
        | "ntile" => FunctionArguments::binary(),
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
    headers_index: &HeadersIndex,
) -> Result<ConcreteWindowAggregations, ConcretizationError> {
    let aggs = parse_aggregations(input).map_err(ConcretizationError::ParseError)?;

    let mut concrete_aggs = Vec::with_capacity(aggs.len());

    for mut agg in aggs {
        let func_name = &agg.func_name;

        if is_agg_fn_name(func_name) {
            let agg_name = agg.agg_name.clone();

            let concrete_aggregations = concretize_aggregations(vec![agg], headers_index)?;
            let sub_program = AggregationProgram::from_concrete_aggregations(
                concrete_aggregations,
                headers_index.clone(),
            );

            concrete_aggs.push((
                agg_name,
                ConcreteWindowAggregation::TotalAggregation(sub_program, DynamicValue::None),
            ));

            continue;
        }

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
            "row_index" => {
                concrete_aggs.push((agg.agg_name, ConcreteWindowAggregation::RowIndex(0)));
            }
            "lead" | "lag" => {
                let (n, default) = if agg.args.len() == 1 {
                    (1, ConcreteExpr::Value(DynamicValue::None))
                } else if agg.args.len() == 2 {
                    (
                        cast_as_usize(&concretize_expression(
                            agg.args.pop().unwrap(),
                            headers_index,
                            None,
                        )?)?,
                        ConcreteExpr::Value(DynamicValue::None),
                    )
                } else {
                    let default =
                        concretize_expression(agg.args.pop().unwrap(), headers_index, None)?;

                    (
                        cast_as_usize(&concretize_expression(
                            agg.args.pop().unwrap(),
                            headers_index,
                            None,
                        )?)?,
                        default,
                    )
                };

                let expr = concretize_expression(agg.args.pop().unwrap(), headers_index, None)?;

                let concrete_agg = if func_name == "lead" {
                    ConcreteWindowAggregation::Lead(expr, n, default)
                } else {
                    ConcreteWindowAggregation::Lag(expr, n, default)
                };

                concrete_aggs.push((agg.agg_name, concrete_agg));
            }
            "cumsum" | "cummin" | "cummax" | "front_coding" => {
                let expr = concretize_expression(agg.args.pop().unwrap(), headers_index, None)?;

                concrete_aggs.push((
                    agg.agg_name,
                    match func_name.as_str() {
                        "cumsum" => ConcreteWindowAggregation::CumulativeSum(expr, Sum::new()),
                        "cummin" => ConcreteWindowAggregation::CumulativeMin(expr, None),
                        "cummax" => ConcreteWindowAggregation::CumulativeMax(expr, None),
                        "front_coding" => ConcreteWindowAggregation::FrontCoding(expr, None),
                        _ => unreachable!(),
                    },
                ))
            }
            "rolling_sum" | "rolling_mean" | "rolling_avg" | "rolling_var" | "rolling_stddev" => {
                let expr = concretize_expression(agg.args.pop().unwrap(), headers_index, None)?;
                let window_size = cast_as_usize(&concretize_expression(
                    agg.args.pop().unwrap(),
                    headers_index,
                    None,
                )?)?;

                concrete_aggs.push((
                    agg.agg_name,
                    match func_name.as_str() {
                        "rolling_sum" => ConcreteWindowAggregation::RollingSum(
                            expr,
                            RollingSum::with_window_size(window_size),
                        ),
                        "rolling_mean" | "rolling_avg" => {
                            ConcreteWindowAggregation::RollingWelford(
                                expr,
                                WelfordStat::Mean,
                                RollingWelford::with_window_size(window_size),
                            )
                        }
                        "rolling_var" => ConcreteWindowAggregation::RollingWelford(
                            expr,
                            WelfordStat::Var,
                            RollingWelford::with_window_size(window_size),
                        ),
                        "rolling_stddev" => ConcreteWindowAggregation::RollingWelford(
                            expr,
                            WelfordStat::Stddev,
                            RollingWelford::with_window_size(window_size),
                        ),
                        _ => unreachable!(),
                    },
                ));
            }
            "frac" => {
                let decimals = if agg.args.len() == 2 {
                    Some(cast_as_usize(&concretize_expression(
                        agg.args.pop().unwrap(),
                        headers_index,
                        None,
                    )?)?)
                } else {
                    None
                };

                let expr = concretize_expression(agg.args.pop().unwrap(), headers_index, None)?;

                concrete_aggs.push((
                    agg.agg_name,
                    ConcreteWindowAggregation::Frac(expr, Sum::new(), decimals),
                ));
            }
            "dense_rank" | "rank" | "cume_dist" | "percent_rank" => {
                let expr = concretize_expression(agg.args.pop().unwrap(), headers_index, None)?;

                let kind = match func_name.as_str() {
                    "dense_rank" => RankingKind::Dense,
                    "rank" => RankingKind::Arbitrary,
                    "cume_dist" => RankingKind::CumulativeDistribution,
                    "percent_rank" => RankingKind::PercentRank,
                    _ => unreachable!(),
                };

                concrete_aggs.push((
                    agg.agg_name,
                    ConcreteWindowAggregation::Ranking(Ranking::new(kind, expr)),
                ));
            }
            "ntile" => {
                let expr = concretize_expression(agg.args.pop().unwrap(), headers_index, None)?;
                let k = cast_as_usize(&concretize_expression(
                    agg.args.pop().unwrap(),
                    headers_index,
                    None,
                )?)?;

                concrete_aggs.push((
                    agg.agg_name,
                    ConcreteWindowAggregation::Ranking(Ranking::new(RankingKind::NTile(k), expr)),
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

type PastBuffer = VecDeque<(usize, Option<usize>, ByteRecord)>;
type FutureBuffer = VecDeque<(usize, Option<usize>, ByteRecord, bool)>;
type TotalBuffer = Vec<(usize, Option<usize>, ByteRecord)>;

#[derive(Debug, Clone)]
pub struct WindowAggregationProgram {
    aggs: ConcreteWindowAggregations,
    headers_index: HeadersIndex,
    past_buffer: Option<(usize, PastBuffer)>,
    future_buffer: Option<(usize, FutureBuffer)>,
    total_buffer: Option<TotalBuffer>,
    output_buffer: Vec<DynamicValue>,
    mask: Vec<Option<usize>>,
    rest: Vec<usize>,
}

impl WindowAggregationProgram {
    pub fn parse(
        code: &str,
        headers: &ByteRecord,
        headless: bool,
    ) -> Result<Self, ConcretizationError> {
        let headers_index = HeadersIndex::new(headers, headless);
        let aggs = concretize_window_aggregations(code, &headers_index)?;

        let mask = vec![None; headers.len()];
        let rest = (0..aggs.len()).collect();

        let (max_past, max_future) = find_buffer_extent(&aggs);

        let past_buffer = (max_past > 0).then(|| (max_past, VecDeque::with_capacity(max_past)));

        let future_buffer =
            (max_future > 0).then(|| (max_future + 1, VecDeque::with_capacity(max_future + 1)));

        let total_buffer = aggs
            .iter()
            .any(|(_, agg)| agg.requires_total_buffer())
            .then(Vec::new);

        Ok(Self {
            aggs,
            output_buffer: Vec::with_capacity(headers.len()),
            headers_index,
            past_buffer,
            future_buffer,
            total_buffer,
            mask,
            rest,
        })
    }

    fn has_something_to_overwrite(&self) -> bool {
        self.rest.len() < self.aggs.len()
    }

    pub fn overwrite(&mut self) {
        self.rest.clear();

        for (expr_i, (name, _)) in self.aggs.iter().enumerate() {
            let pos = self.headers_index.first_by_name(name);

            if let Some(i) = pos {
                self.mask[i] = Some(expr_i);
            } else {
                self.rest.push(expr_i);
            }
        }
    }

    pub fn headers(&self) -> impl Iterator<Item = &[u8]> {
        self.rest.iter().map(|i| self.aggs[*i].0.as_bytes())
    }

    fn run_with_record_impl(
        &mut self,
        row_index: usize,
        col_index: Option<usize>,
        record: &ByteRecord,
        is_padding: bool,
    ) -> Result<Option<ByteRecord>, SpecifiedEvaluationError> {
        if let Some(total_buffer) = &mut self.total_buffer {
            total_buffer.push((row_index, col_index, record.clone()));

            return Ok(None);
        }

        if let Some((future_capacity, future_buffer)) = &mut self.future_buffer {
            if future_buffer.len() < *future_capacity {
                future_buffer.push_back((row_index, col_index, record.clone(), is_padding));

                return Ok(None);
            }
        }

        self.output_buffer.clear();

        let past_buffer_ref = self.past_buffer.as_ref().map(|(_, b)| b);
        let future_buffer_ref = self.future_buffer.as_ref().map(|(_, b)| b);

        for (_, agg) in self.aggs.iter_mut() {
            let (working_row_index, working_col_index, working_record) =
                if let Some((_, future_buffer)) = &self.future_buffer {
                    let (i, c, r, _) = future_buffer.front().unwrap();

                    (*i, *c, r)
                } else {
                    (row_index, col_index, record)
                };

            self.output_buffer.push(agg.run(
                working_row_index,
                working_col_index,
                working_record,
                &self.headers_index,
                past_buffer_ref,
                future_buffer_ref,
            )?);
        }

        let record_to_emit = if let Some((_, future_buffer)) = &mut self.future_buffer {
            let r = future_buffer.pop_front();
            future_buffer.push_back((row_index, col_index, record.clone(), is_padding));
            &r.unwrap().2
        } else {
            record
        };

        if let Some((past_capacity, past_buffer)) = &mut self.past_buffer {
            if past_buffer.len() >= *past_capacity {
                past_buffer.pop_back();
            }

            past_buffer.push_front((row_index, col_index, record_to_emit.clone()));
        }

        if self.has_something_to_overwrite() {
            let mut output_record = ByteRecord::new();

            for (expr_i_opt, cell) in self.mask.iter().copied().zip(record_to_emit.iter()) {
                if let Some(expr_i) = expr_i_opt {
                    self.output_buffer[expr_i].push_field_to_record(&mut output_record);
                } else {
                    output_record.push_field(cell);
                }
            }

            for expr_i in self.rest.iter().copied() {
                self.output_buffer[expr_i].push_field_to_record(&mut output_record);
            }

            Ok(Some(output_record))
        } else {
            let mut output_record = record_to_emit.clone();

            for value in self.output_buffer.iter() {
                value.push_field_to_record(&mut output_record);
            }

            Ok(Some(output_record))
        }
    }

    #[inline(always)]
    pub fn run_with_record(
        &mut self,
        row_index: usize,
        col_index: Option<usize>,
        record: &ByteRecord,
    ) -> Result<Option<ByteRecord>, SpecifiedEvaluationError> {
        self.run_with_record_impl(row_index, col_index, record, false)
    }

    fn flush_impl<F, E>(
        &mut self,
        mut from_row_index: usize,
        col_index: Option<usize>,
        mut callback: F,
    ) -> Result<(), E>
    where
        F: FnMut(ByteRecord) -> Result<(), E>,
        E: From<SpecifiedEvaluationError>,
    {
        if let Some(total_buffer) = self.total_buffer.take() {
            for (row_index, col_index, record) in total_buffer.iter() {
                for (_, agg) in self.aggs.iter_mut() {
                    agg.aggregate_total(*row_index, *col_index, record, &self.headers_index)?;
                }
            }

            for (_, agg) in self.aggs.iter_mut() {
                agg.finalize_total()?;
            }

            for (row_index, col_index, record) in total_buffer.iter() {
                if let Some(output_record) = self.run_with_record(*row_index, *col_index, record)? {
                    callback(output_record)?;
                }
            }
        }

        if let Some((_, future_buffer)) = &self.future_buffer {
            let padding = (0..self.headers_index.len())
                .map(|_| b"")
                .collect::<ByteRecord>();

            for _ in 0..future_buffer.len() {
                from_row_index += 1;
                callback(
                    self.run_with_record_impl(from_row_index, col_index, &padding, true)?
                        .unwrap(),
                )?;
            }
        }

        Ok(())
    }

    #[inline(always)]
    pub fn flush<F, E>(mut self, from_row_index: usize, callback: F) -> Result<(), E>
    where
        F: FnMut(ByteRecord) -> Result<(), E>,
        E: From<SpecifiedEvaluationError>,
    {
        self.flush_impl(from_row_index, None, callback)
    }

    fn clear(&mut self) {
        if self.aggs.iter().any(|(_, agg)| agg.requires_total_buffer()) {
            self.total_buffer = Some(Vec::new());
        }

        if let Some((_, past_buffer)) = &mut self.past_buffer {
            past_buffer.clear();
        }

        if let Some((_, future_buffer)) = &mut self.future_buffer {
            future_buffer.clear();
        }

        for (_, agg) in &mut self.aggs {
            agg.clear();
        }
    }

    fn flush_and_clear_impl<F, E>(
        &mut self,
        from_row_index: usize,
        col_index: Option<usize>,
        callback: F,
    ) -> Result<(), E>
    where
        F: FnMut(ByteRecord) -> Result<(), E>,
        E: From<SpecifiedEvaluationError>,
    {
        self.flush_impl(from_row_index, col_index, callback)?;
        self.clear();

        Ok(())
    }

    #[inline(always)]
    pub fn flush_and_clear<F, E>(&mut self, from_row_index: usize, callback: F) -> Result<(), E>
    where
        F: FnMut(ByteRecord) -> Result<(), E>,
        E: From<SpecifiedEvaluationError>,
    {
        self.flush_and_clear_impl(from_row_index, None, callback)?;
        self.clear();

        Ok(())
    }
}

// TODO: this is hardly optimal, allocation-wise...
pub struct WindowAggregationArray {
    copies: Vec<WindowAggregationProgram>,
    selection: Vec<usize>,
    output_buffer: Vec<ByteRecord>,
}

impl WindowAggregationArray {
    pub fn from_program(program: &WindowAggregationProgram, selection: &[usize]) -> Self {
        Self {
            copies: (0..selection.len()).map(|_| program.clone()).collect(),
            selection: selection.to_vec(),
            output_buffer: Vec::with_capacity(selection.len()),
        }
    }

    pub fn run_with_record(
        &mut self,
        row_index: usize,
        record: &ByteRecord,
    ) -> Result<Option<(ByteRecord, &Vec<ByteRecord>)>, SpecifiedEvaluationError> {
        let mut input_record: Option<ByteRecord> = None;
        self.output_buffer.clear();

        for (col_index, program) in self.selection.iter().copied().zip(self.copies.iter_mut()) {
            if let Some(mut output_record) =
                program.run_with_record_impl(row_index, Some(col_index), record, false)?
            {
                self.output_buffer
                    .push(output_record.iter().skip(record.len()).collect());

                if input_record.is_none() {
                    output_record.truncate(record.len());
                    input_record = Some(output_record);
                }
            }
        }

        Ok(input_record.map(|r| (r, &self.output_buffer)))
    }

    fn flush_impl<F, E>(
        &mut self,
        alignment: usize,
        from_row_index: usize,
        mut callback: F,
    ) -> Result<(), E>
    where
        F: FnMut(ByteRecord, Vec<ByteRecord>) -> Result<(), E>,
        E: From<SpecifiedEvaluationError>,
    {
        let mut flush_buffer: Vec<(ByteRecord, Vec<ByteRecord>)> = vec![];

        for (col_index, program) in self.selection.iter().copied().zip(self.copies.iter_mut()) {
            let mut index: usize = 0;

            program.flush_impl(from_row_index, Some(col_index), |record| {
                if flush_buffer.get(index).is_none() {
                    flush_buffer.push((record.iter().take(alignment).collect(), vec![]));
                }

                flush_buffer
                    .get_mut(index)
                    .unwrap()
                    .1
                    .push(record.iter().skip(alignment).collect());

                index += 1;
                Ok(())
            })?;
        }

        for (record, records) in flush_buffer {
            debug_assert_eq!(records.len(), self.copies.len());
            callback(record, records)?;
        }

        Ok(())
    }

    #[inline(always)]
    pub fn flush<F, E>(
        mut self,
        alignment: usize,
        from_row_index: usize,
        callback: F,
    ) -> Result<(), E>
    where
        F: FnMut(ByteRecord, Vec<ByteRecord>) -> Result<(), E>,
        E: From<SpecifiedEvaluationError>,
    {
        self.flush_impl(from_row_index, alignment, callback)
    }

    #[inline(always)]
    fn clear(&mut self) {
        for program in self.copies.iter_mut() {
            program.clear();
        }
    }

    #[inline(always)]
    pub fn flush_and_clear<F, E>(
        &mut self,
        alignment: usize,
        from_row_index: usize,
        callback: F,
    ) -> Result<(), E>
    where
        F: FnMut(ByteRecord, Vec<ByteRecord>) -> Result<(), E>,
        E: From<SpecifiedEvaluationError>,
    {
        self.flush_impl(alignment, from_row_index, callback)?;
        self.clear();

        Ok(())
    }
}
