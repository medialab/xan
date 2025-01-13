use std::sync::Arc;

use csv::ByteRecord;
use jiff::{civil::DateTime, Unit};

use super::aggregators::{
    AllAny, ApproxCardinality, ApproxQuantiles, ArgExtent, ArgTop, Count, CovarianceWelford, First,
    Frequencies, Last, LexicographicExtent, MedianType, Numbers, NumericExtent, Sum, Types, Values,
    Welford, ZonedExtent,
};
use crate::collections::ClusteredInsertHashmap;
use crate::moonblade::error::{
    ConcretizationError, EvaluationError, InvalidArity, SpecifiedEvaluationError,
};
use crate::moonblade::interpreter::{
    concretize_expression, eval_expression, ConcreteExpr, EvaluationContext,
};
use crate::moonblade::parser::{parse_aggregations, Aggregations};
use crate::moonblade::types::{Arity, DynamicNumber, DynamicValue};

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
                    Self::ApproxCardinality(inner) => {
                        inner.finalize();
                    }
                    Self::ApproxQuantiles(inner) => {
                        inner.finalize();
                    }
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
    ApproxCardinality,
    ApproxQuantiles,
    ArgExtent,
    ArgTop,
    Count,
    CovarianceWelford,
    NumericExtent,
    First,
    Last,
    Values,
    LexicographicExtent,
    Frequencies,
    Numbers,
    Sum,
    Types,
    Welford,
    ZonedExtent,
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
            (ConcreteAggregationMethod::ApproxCardinality, Self::ApproxCardinality(inner)) => {
                DynamicValue::from(inner.get())
            }
            (ConcreteAggregationMethod::ApproxQuantile(q), Self::ApproxQuantiles(inner)) => {
                DynamicValue::from(inner.get(*q))
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
                                    .map_err(|err| err.specify("argtop"))?
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
            (ConcreteAggregationMethod::Correlation, Self::CovarianceWelford(inner)) => {
                DynamicValue::from(inner.correlation())
            }
            (ConcreteAggregationMethod::Count, Self::Count(inner)) => {
                DynamicValue::from(inner.get_truthy())
            }
            (ConcreteAggregationMethod::CovariancePop, Self::CovarianceWelford(inner)) => {
                DynamicValue::from(inner.covariance())
            }
            (ConcreteAggregationMethod::CovarianceSample, Self::CovarianceWelford(inner)) => {
                DynamicValue::from(inner.sample_covariance())
            }
            (ConcreteAggregationMethod::Ratio, Self::Count(inner)) => {
                DynamicValue::from(inner.ratio())
            }
            (ConcreteAggregationMethod::Percentage, Self::Count(inner)) => {
                DynamicValue::from(inner.percentage())
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
            (ConcreteAggregationMethod::Earliest, Self::ZonedExtent(inner)) => {
                DynamicValue::from(inner.earliest())
            }
            (ConcreteAggregationMethod::Latest, Self::ZonedExtent(inner)) => {
                DynamicValue::from(inner.lastest())
            }
            (ConcreteAggregationMethod::CountTime(unit), Self::ZonedExtent(inner)) => {
                DynamicValue::from(inner.count(*unit))
            }
            (ConcreteAggregationMethod::Min, Self::NumericExtent(inner)) => {
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
            (ConcreteAggregationMethod::Max, Self::NumericExtent(inner)) => {
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
            ConcreteAggregationMethod::ApproxCardinality => {
                upsert_aggregator!(ApproxCardinality)
            }
            ConcreteAggregationMethod::ApproxQuantile(_) => {
                upsert_aggregator!(ApproxQuantiles)
            }
            ConcreteAggregationMethod::Count
            | ConcreteAggregationMethod::Ratio
            | ConcreteAggregationMethod::Percentage => {
                upsert_aggregator!(Count)
            }
            ConcreteAggregationMethod::CovariancePop
            | ConcreteAggregationMethod::CovarianceSample
            | ConcreteAggregationMethod::Correlation => {
                upsert_aggregator!(CovarianceWelford)
            }
            ConcreteAggregationMethod::Min | ConcreteAggregationMethod::Max => {
                // NOTE: if some ArgExtent already exists, we merge into it.
                match self
                    .methods
                    .iter()
                    .position(|item| matches!(item, Aggregator::ArgExtent(_)))
                {
                    None => upsert_aggregator!(NumericExtent),
                    Some(idx) => idx,
                }
            }
            ConcreteAggregationMethod::ArgMin(_) | ConcreteAggregationMethod::ArgMax(_) => {
                // NOTE: if some Extent exist, we replace it
                match self
                    .methods
                    .iter()
                    .position(|item| matches!(item, Aggregator::NumericExtent(_)))
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
            ConcreteAggregationMethod::Earliest
            | ConcreteAggregationMethod::Latest
            | ConcreteAggregationMethod::CountTime(_) => {
                upsert_aggregator!(ZonedExtent)
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
                    Aggregator::ApproxCardinality(approx_cardinality) => {
                        if !value.is_nullish() {
                            approx_cardinality.add(&value.try_as_str()?);
                        }
                    }
                    Aggregator::ApproxQuantiles(approx_quantiles) => {
                        if !value.is_nullish() {
                            approx_quantiles.add(value.try_as_f64()?);
                        }
                    }
                    Aggregator::Count(count) => {
                        count.add(value.is_truthy());
                    }
                    Aggregator::CovarianceWelford(_) => unreachable!(),
                    Aggregator::NumericExtent(extent) => {
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
                    Aggregator::ZonedExtent(extent) => {
                        if !value.is_nullish() {
                            extent.add(value.try_as_datetime()?.as_ref());
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
                        count.add_truthy();
                    }
                    _ => unreachable!(),
                },
            }
        }

        Ok(())
    }

    fn process_pair(
        &mut self,
        _index: usize,
        first: DynamicValue,
        second: DynamicValue,
    ) -> Result<(), EvaluationError> {
        for method in self.methods.iter_mut() {
            match method {
                Aggregator::CovarianceWelford(covariance_welford) => {
                    match (first.is_nullish(), second.is_nullish()) {
                        (true, false) | (false, true) => return Err(EvaluationError::Custom("unaligned series where given to covariance or correlation functions (both series must have the same number of data points)".to_string())),
                        (false, false) => {
                            covariance_welford.add(first.try_as_f64()?, second.try_as_f64()?);
                        }
                        _ => ()
                    }
                }
                _ => unreachable!(),
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
    name: &str,
    mut arity: usize,
) -> Result<(), ConcretizationError> {
    arity += 1;

    let range = match name {
        "count" => 0..=1,
        "quantile" | "approx_quantile" => 2..=2,
        "values" | "distinct_values" | "argmin" | "argmax" | "modes" => 1..=2,
        "most_common" | "most_common_counts" | "top" => 1..=3,
        "argtop" => 1..=4,
        _ => 1..=1,
    };

    if !range.contains(&arity) {
        return Err(ConcretizationError::InvalidArity(
            name.to_string(),
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
    ApproxCardinality,
    ApproxQuantile(f64),
    ArgMin(Option<ConcreteExpr>),
    ArgMax(Option<ConcreteExpr>),
    ArgTop(usize, Option<ConcreteExpr>, String),
    Cardinality,
    Correlation,
    Count,
    CountTime(Unit),
    CovariancePop,
    CovarianceSample,
    DistinctValues(String),
    Earliest,
    First,
    Latest,
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
    Percentage,
    Quartile(usize),
    Quantile(f64),
    Ratio,
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
        let arity = args.len();

        let method = match name {
            "all" => Self::All,
            "any" => Self::Any,
            "approx_cardinality" => Self::ApproxCardinality,
            "approx_quantile" => match args.first().unwrap() {
                ConcreteExpr::Value(v) => match v.try_as_f64() {
                    Ok(p) => Self::ApproxQuantile(p),
                    Err(_) => return Err(ConcretizationError::NotStaticallyAnalyzable),
                },
                _ => return Err(ConcretizationError::NotStaticallyAnalyzable),
            },
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
            "correlation" => Self::Correlation,
            "count" => Self::Count,
            "count_seconds" => Self::CountTime(Unit::Second),
            "count_hours" => Self::CountTime(Unit::Hour),
            "count_days" => Self::CountTime(Unit::Day),
            "count_years" => Self::CountTime(Unit::Year),
            "covariance" | "covariance_pop" => Self::CovariancePop,
            "covariance_sample" => Self::CovarianceSample,
            "distinct_values" => Self::DistinctValues(match get_separator_from_argument(args, 0) {
                None => return Err(ConcretizationError::NotStaticallyAnalyzable),
                Some(separator) => separator,
            }),
            "earliest" => Self::Earliest,
            "first" => Self::First,
            "latest" => Self::Latest,
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
            "percentage" => Self::Percentage,
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
            "ratio" => Self::Ratio,
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
        };

        validate_aggregation_function_arity(name, arity)?;

        Ok(method)
    }
}

#[derive(Debug)]
struct ConcreteAggregation {
    agg_name: String,
    method: ConcreteAggregationMethod,
    expr: Option<ConcreteExpr>,
    expr_key: String,
    pair_expr: Option<ConcreteExpr>,
    // args: Vec<ConcreteExpr>,
}

type ConcreteAggregations = Vec<ConcreteAggregation>;

fn concretize_aggregations(
    aggregations: Aggregations,
    headers: &ByteRecord,
) -> Result<ConcreteAggregations, ConcretizationError> {
    let mut concrete_aggregations = ConcreteAggregations::new();

    for mut aggregation in aggregations {
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

        let mut skip: usize = 1;

        let pair_expr = if aggregation.args.len() > 1
            && [
                "covariance",
                "covariance_pop",
                "covariance_sample",
                "correlation",
            ]
            .contains(&aggregation.func_name.as_str())
        {
            skip = 2;
            Some(concretize_expression(
                aggregation.args.get(1).unwrap().clone(),
                headers,
            )?)
        } else {
            None
        };

        let mut args: Vec<ConcreteExpr> = Vec::new();

        for arg in aggregation.args.into_iter().skip(skip) {
            args.push(concretize_expression(arg, headers)?);
        }

        let method = ConcreteAggregationMethod::parse(&aggregation.func_name, args)?;

        let concrete_aggregation = ConcreteAggregation {
            agg_name: aggregation.agg_name,
            method,
            expr_key: aggregation.expr_key,
            expr,
            pair_expr,
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
    pair_expr: Option<ConcreteExpr>,
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
                    pair_expr: agg.pair_expr,
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

        if let Some(pair_expr) = &unit.pair_expr {
            let second_value = eval_expression(pair_expr, Some(index), record, context)?;

            return aggregator
                .process_pair(index, value.unwrap(), second_value)
                .map_err(|err| err.specify(&format!("<agg-expr: {}>", unit.expr_key)));
        }

        if let Some(DynamicValue::List(list)) = value {
            for v in Arc::into_inner(list).unwrap() {
                aggregator
                    .process_value(index, Some(v), record)
                    .map_err(|err| err.specify(&format!("<agg-expr: {}>", unit.expr_key)))?;
            }
        } else {
            aggregator
                .process_value(index, value, record)
                .map_err(|err| err.specify(&format!("<agg-expr: {}>", unit.expr_key)))?;
        }
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
    groups: ClusteredInsertHashmap<GroupKey, Vec<CompositeAggregator>>,
    context: EvaluationContext,
}

impl GroupAggregationProgram {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let concrete_aggregations = prepare(code, headers)?;
        let planner = ConcreteAggregationPlanner::from(concrete_aggregations);

        Ok(Self {
            planner,
            groups: ClusteredInsertHashmap::new(),
            context: EvaluationContext::new(headers),
        })
    }

    pub fn merge(&mut self, other: Self) {
        for (key, other_aggregators) in other.groups.into_iter() {
            self.groups.insert_or_update_with(
                key,
                other_aggregators,
                |self_aggregators, other_aggregators| {
                    for (self_aggregator, other_aggregator) in
                        self_aggregators.iter_mut().zip(other_aggregators)
                    {
                        self_aggregator.merge(other_aggregator);
                    }
                },
            );
        }
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
