use std::collections::{BTreeMap, BTreeSet};
use std::hash::Hash;
use std::iter::once;
use std::sync::Arc;

use ahash::RandomState;
use indexmap::IndexMap;
use jiff::{civil::DateTime, Unit};
use simd_csv::ByteRecord;

use super::aggregators::{
    AllAny, ApproxCardinality, ApproxQuantiles, ArgExtent, ArgTop, Count, CovarianceWelford, First,
    Frequencies, Last, LexicographicExtent, MedianType, Numbers, NumericExtent, RMSWelford, Sum,
    Types, Values, Welford, ZonedExtent,
};
use crate::collections::ClusteredInsertHashmap;
use crate::moonblade::error::{ConcretizationError, EvaluationError, SpecifiedEvaluationError};
use crate::moonblade::interpreter::{
    concretize_expression, eval_expression_with_optional_last_value, ConcreteExpr,
};
use crate::moonblade::parser::{parse_aggregations, Aggregations};
use crate::moonblade::types::{DynamicNumber, DynamicValue, FunctionArguments, HeadersIndex};

// NOTE: we are boxing some ones to avoid going over size=64
#[derive(Debug, Clone)]
enum Aggregator {
    AllAny(AllAny),
    ApproxCardinality(Box<ApproxCardinality>),
    ApproxQuantiles(Box<ApproxQuantiles>),
    ArgExtent(Box<ArgExtent>),
    ArgTop(ArgTop),
    Count(Count),
    CovarianceWelford(CovarianceWelford),
    NumericExtent(NumericExtent),
    First(First),
    Last(Last),
    Values(Values),
    LexicographicExtent(LexicographicExtent),
    Frequencies(Box<Frequencies>),
    Numbers(Numbers),
    RMSWelford(RMSWelford),
    Sum(Sum),
    Types(Types),
    Welford(Welford),
    ZonedExtent(Box<ZonedExtent>),
}

impl Aggregator {
    fn clear(&mut self) {
        use Aggregator::*;

        match self {
            AllAny(inner) => inner.clear(),
            ApproxCardinality(inner) => inner.clear(),
            ApproxQuantiles(inner) => inner.clear(),
            ArgExtent(inner) => inner.clear(),
            ArgTop(inner) => inner.clear(),
            Count(inner) => inner.clear(),
            CovarianceWelford(inner) => inner.clear(),
            NumericExtent(inner) => inner.clear(),
            First(inner) => inner.clear(),
            Last(inner) => inner.clear(),
            Values(inner) => inner.clear(),
            LexicographicExtent(inner) => inner.clear(),
            Frequencies(inner) => inner.clear(),
            Numbers(inner) => inner.clear(),
            RMSWelford(inner) => inner.clear(),
            Sum(inner) => inner.clear(),
            Types(inner) => inner.clear(),
            Welford(inner) => inner.clear(),
            ZonedExtent(inner) => inner.clear(),
        }
    }

    fn merge(&mut self, other: Self) {
        use Aggregator::*;

        match (self, other) {
            (AllAny(inner), AllAny(other_inner)) => inner.merge(other_inner),
            (ApproxCardinality(inner), ApproxCardinality(other_inner)) => inner.merge(*other_inner),
            (ApproxQuantiles(inner), ApproxQuantiles(other_inner)) => inner.merge(*other_inner),
            (ArgExtent(inner), ArgExtent(other_inner)) => inner.merge(*other_inner),
            (ArgTop(inner), ArgTop(other_inner)) => inner.merge(other_inner),
            (Count(inner), Count(other_inner)) => inner.merge(other_inner),
            (CovarianceWelford(inner), CovarianceWelford(other_inner)) => inner.merge(other_inner),
            (NumericExtent(inner), NumericExtent(other_inner)) => inner.merge(other_inner),
            (First(inner), First(other_inner)) => inner.merge(other_inner),
            (Last(inner), Last(other_inner)) => inner.merge(other_inner),
            (Values(inner), Values(other_inner)) => inner.merge(other_inner),
            (LexicographicExtent(inner), LexicographicExtent(other_inner)) => {
                inner.merge(other_inner)
            }
            (Frequencies(inner), Frequencies(other_inner)) => inner.merge(*other_inner),
            (Numbers(inner), Numbers(other_inner)) => inner.merge(other_inner),
            (RMSWelford(inner), RMSWelford(other_inner)) => inner.merge(other_inner),
            (Sum(inner), Sum(other_inner)) => inner.merge(other_inner),
            (Types(inner), Types(other_inner)) => inner.merge(other_inner),
            (Welford(inner), Welford(other_inner)) => inner.merge(other_inner),
            (ZonedExtent(inner), ZonedExtent(other_inner)) => inner.merge(*other_inner),
            _ => unreachable!(),
        }
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

    fn get_final_value(
        &self,
        method: &ConcreteAggregationMethod,
        headers_index: &HeadersIndex,
    ) -> Result<DynamicValue, SpecifiedEvaluationError> {
        debug_assert!(std::mem::size_of::<Self>() <= 64);

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

                        for (index, record, last_value) in inner.top_records() {
                            let value = eval_expression_with_optional_last_value(
                                expr,
                                Some(index),
                                &record,
                                headers_index,
                                last_value,
                            )?;

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
            (ConcreteAggregationMethod::Ratio(decimals), Self::Count(inner)) => match decimals {
                None => DynamicValue::from(inner.ratio()),
                Some(p) => DynamicValue::from(format!("{:.p$}", inner.ratio(), p = p)),
            },
            (ConcreteAggregationMethod::Percentage(decimals), Self::Count(inner)) => {
                DynamicValue::from(inner.percentage(*decimals))
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
                if let Some((index, record, last_value)) = inner.argmin() {
                    match expr_opt {
                        None => DynamicValue::from(*index),
                        Some(expr) => {
                            return eval_expression_with_optional_last_value(
                                expr,
                                Some(*index),
                                record,
                                headers_index,
                                last_value.clone(),
                            )
                        }
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
                if let Some((index, record, last_value)) = inner.argmax() {
                    match expr_opt {
                        None => DynamicValue::from(*index),
                        Some(expr) => {
                            return eval_expression_with_optional_last_value(
                                expr,
                                Some(*index),
                                record,
                                headers_index,
                                last_value.clone(),
                            )
                        }
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
            (ConcreteAggregationMethod::Rms, Self::RMSWelford(inner)) => {
                DynamicValue::from(inner.rms())
            }
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

        macro_rules! upsert_boxed_aggregator {
            ($variant: ident) => {
                match self
                    .methods
                    .iter()
                    .position(|item| matches!(item, Aggregator::$variant(_)))
                {
                    Some(idx) => idx,
                    None => {
                        let idx = self.methods.len();
                        self.methods
                            .push(Aggregator::$variant(Box::new($variant::new())));
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
                upsert_boxed_aggregator!(ApproxCardinality)
            }
            ConcreteAggregationMethod::ApproxQuantile(_) => {
                upsert_boxed_aggregator!(ApproxQuantiles)
            }
            ConcreteAggregationMethod::Count
            | ConcreteAggregationMethod::Ratio(_)
            | ConcreteAggregationMethod::Percentage(_) => {
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
                    None => upsert_boxed_aggregator!(ArgExtent),
                    Some(idx) => {
                        self.methods[idx] = Aggregator::ArgExtent(Box::new(ArgExtent::new()));
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
                upsert_boxed_aggregator!(ZonedExtent)
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
                upsert_boxed_aggregator!(Frequencies)
            }
            ConcreteAggregationMethod::Rms => {
                upsert_aggregator!(RMSWelford)
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
                            extent.add(index, value.try_as_number()?, record, &None);
                        }
                    }
                    Aggregator::ArgTop(top) => {
                        if !value.is_nullish() {
                            top.add(index, value.try_as_number()?, record, &None);
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
                    Aggregator::RMSWelford(inner) => {
                        if !value.is_nullish() {
                            inner.add(value.try_as_f64()?);
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
        headers_index: &HeadersIndex,
    ) -> Result<DynamicValue, SpecifiedEvaluationError> {
        self.methods[handle].get_final_value(method, headers_index)
    }
}

fn cast_as_static_value<T>(
    arg: &ConcreteExpr,
    cast_fn: fn(&DynamicValue) -> Result<T, EvaluationError>,
) -> Result<T, ConcretizationError> {
    if let ConcreteExpr::Value(v) = arg {
        cast_fn(v).map_err(|_| ConcretizationError::NotStaticallyAnalyzable)
    } else {
        Err(ConcretizationError::NotStaticallyAnalyzable)
    }
}

fn cast_as_separator(arg_opt: Option<&ConcreteExpr>) -> Result<String, ConcretizationError> {
    match arg_opt {
        None => Ok("|".to_string()),
        Some(arg) => {
            if let ConcreteExpr::Value(v) = arg {
                let sep = v
                    .try_as_str()
                    .map_err(|_| ConcretizationError::NotStaticallyAnalyzable)?;

                Ok(sep.into_owned())
            } else {
                Err(ConcretizationError::NotStaticallyAnalyzable)
            }
        }
    }
}

type ArgumentParser = fn(&[ConcreteExpr]) -> Result<ConcreteAggregationMethod, ConcretizationError>;

fn get_function_arguments_parser(name: &str) -> Option<(FunctionArguments, ArgumentParser)> {
    use ConcreteAggregationMethod::*;

    Some(match name {
        "all" => (FunctionArguments::unary(), |_| Ok(All)),
        "any" => (FunctionArguments::unary(), |_| Ok(Any)),
        "approx_cardinality" => (FunctionArguments::unary(), |_| Ok(ApproxCardinality)),
        "approx_quantile" => (FunctionArguments::binary(), |args| {
            Ok(ApproxQuantile(cast_as_static_value(
                args.first().unwrap(),
                DynamicValue::try_as_f64,
            )?))
        }),
        "argmin" => (FunctionArguments::with_range(1..=2), |args| {
            Ok(ArgMin(args.last().cloned()))
        }),
        "argmax" => (FunctionArguments::with_range(1..=2), |args| {
            Ok(ArgMax(args.last().cloned()))
        }),
        "argtop" => (FunctionArguments::with_range(1..=4), |args| {
            Ok(ArgTop(
                cast_as_static_value(args.first().unwrap(), DynamicValue::try_as_usize)?,
                args.get(1).cloned(),
                cast_as_separator(args.get(2))?,
            ))
        }),
        "cardinality" => (FunctionArguments::unary(), |_| Ok(Cardinality)),
        "correlation" => (FunctionArguments::binary(), |_| Ok(Correlation)),
        "count" => (FunctionArguments::with_range(0..=1), |_| Ok(Count)),
        "count_seconds" => (FunctionArguments::unary(), |_| Ok(CountTime(Unit::Second))),
        "count_hours" => (FunctionArguments::unary(), |_| Ok(CountTime(Unit::Hour))),
        "count_days" => (FunctionArguments::unary(), |_| Ok(CountTime(Unit::Day))),
        "count_years" => (FunctionArguments::unary(), |_| Ok(CountTime(Unit::Year))),
        "covariance" | "covariance_pop" => (FunctionArguments::binary(), |_| Ok(CovariancePop)),
        "covariance_sample" => (FunctionArguments::binary(), |_| Ok(CovarianceSample)),
        "distinct_values" => (FunctionArguments::with_range(1..=2), |args| {
            Ok(DistinctValues(cast_as_separator(args.first())?))
        }),
        "earliest" => (FunctionArguments::unary(), |_| Ok(Earliest)),
        "first" => (FunctionArguments::unary(), |_| Ok(First)),
        "latest" => (FunctionArguments::unary(), |_| Ok(Latest)),
        "last" => (FunctionArguments::unary(), |_| Ok(Last)),
        "lex_first" => (FunctionArguments::unary(), |_| Ok(LexFirst)),
        "lex_last" => (FunctionArguments::unary(), |_| Ok(LexLast)),
        "min" => (FunctionArguments::unary(), |_| Ok(Min)),
        "max" => (FunctionArguments::unary(), |_| Ok(Max)),
        "avg" | "mean" => (FunctionArguments::unary(), |_| Ok(Mean)),
        "median" => (FunctionArguments::unary(), |_| {
            Ok(Median(MedianType::Interpolation))
        }),
        "median_high" => (FunctionArguments::unary(), |_| Ok(Median(MedianType::High))),
        "median_low" => (FunctionArguments::unary(), |_| Ok(Median(MedianType::Low))),
        "mode" => (FunctionArguments::unary(), |_| Ok(Mode)),
        "modes" => (FunctionArguments::with_range(1..=2), |args| {
            Ok(Modes(cast_as_separator(args.first())?))
        }),
        "most_common" => (FunctionArguments::with_range(1..=3), |args| {
            Ok(MostCommonValues(
                cast_as_static_value(args.first().unwrap(), DynamicValue::try_as_usize)?,
                cast_as_separator(args.get(1))?,
            ))
        }),
        "most_common_counts" => (FunctionArguments::with_range(1..=3), |args| {
            Ok(MostCommonCounts(
                cast_as_static_value(args.first().unwrap(), DynamicValue::try_as_usize)?,
                cast_as_separator(args.get(1))?,
            ))
        }),
        "percentage" => (FunctionArguments::with_range(1..=2), |args| {
            let decimals = if !args.is_empty() {
                let value =
                    cast_as_static_value(args.first().unwrap(), DynamicValue::try_as_usize)?;
                Some(value.min(16))
            } else {
                None
            };

            Ok(Percentage(decimals))
        }),
        "quantile" => (FunctionArguments::binary(), |args| {
            Ok(Quantile(cast_as_static_value(
                args.first().unwrap(),
                DynamicValue::try_as_f64,
            )?))
        }),
        "q1" => (FunctionArguments::unary(), |_| Ok(Quartile(0))),
        "q2" => (FunctionArguments::unary(), |_| Ok(Quartile(1))),
        "q3" => (FunctionArguments::unary(), |_| Ok(Quartile(2))),
        "values" => (FunctionArguments::with_range(1..=2), |args| {
            Ok(Values(cast_as_separator(args.first())?))
        }),
        "var" | "var_pop" => (FunctionArguments::unary(), |_| Ok(VarPop)),
        "var_sample" => (FunctionArguments::unary(), |_| Ok(VarSample)),
        "ratio" => (FunctionArguments::with_range(1..=2), |args| {
            let decimals = if !args.is_empty() {
                let value =
                    cast_as_static_value(args.first().unwrap(), DynamicValue::try_as_usize)?;
                Some(value.min(16))
            } else {
                None
            };

            Ok(Ratio(decimals))
        }),
        "rms" => (FunctionArguments::unary(), |_| Ok(Rms)),
        "stddev" | "stddev_pop" => (FunctionArguments::unary(), |_| Ok(StddevPop)),
        "stddev_sample" => (FunctionArguments::unary(), |_| Ok(StddevSample)),
        "sum" => (FunctionArguments::unary(), |_| Ok(Sum)),
        "top" => (FunctionArguments::with_range(1..=3), |args| {
            Ok(Top(
                cast_as_static_value(args.first().unwrap(), DynamicValue::try_as_usize)?,
                cast_as_separator(args.get(1))?,
            ))
        }),
        "type" => (FunctionArguments::unary(), |_| Ok(Type)),
        "types" => (FunctionArguments::unary(), |_| Ok(Types)),
        _ => return None,
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
    Percentage(Option<usize>),
    Quartile(usize),
    Quantile(f64),
    Ratio(Option<usize>),
    Rms,
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
    fn parse(
        name: &str,
        args_count: usize,
        args: &[ConcreteExpr],
    ) -> Result<Self, ConcretizationError> {
        match get_function_arguments_parser(name) {
            None => Err(ConcretizationError::UnknownFunction(name.to_string())),
            Some((function_arguments, parser)) => {
                function_arguments
                    .validate_arity(args_count)
                    .map_err(|invalid_arity| {
                        ConcretizationError::InvalidArity(name.to_string(), invalid_arity)
                    })?;

                parser(args)
            }
        }
    }
}

#[derive(Debug)]
struct ConcreteAggregation {
    agg_name: String,
    method: ConcreteAggregationMethod,
    expr: Option<ConcreteExpr>,
    pair_expr: Option<ConcreteExpr>,
}

impl ConcreteAggregation {
    fn key(&self) -> (&Option<ConcreteExpr>, &Option<ConcreteExpr>) {
        (&self.expr, &self.pair_expr)
    }
}

type ConcreteAggregations = Vec<ConcreteAggregation>;

fn concretize_aggregations(
    aggregations: Aggregations,
    headers: &ByteRecord,
) -> Result<ConcreteAggregations, ConcretizationError> {
    let mut concrete_aggregations = ConcreteAggregations::new();

    for mut aggregation in aggregations {
        let args_count = aggregation.args.len();

        if ["most_common", "most_common_counts", "top", "argtop"]
            .contains(&aggregation.func_name.as_str())
        {
            aggregation.args.swap(0, 1);
        }

        let expr = aggregation
            .args
            .first()
            .map(|arg| concretize_expression(arg.clone(), headers, None))
            .transpose()?;

        let pair_expr = if aggregation.args.len() > 1
            && [
                "covariance",
                "covariance_pop",
                "covariance_sample",
                "correlation",
            ]
            .contains(&aggregation.func_name.as_str())
        {
            Some(concretize_expression(
                aggregation.args.pop().unwrap().clone(),
                headers,
                None,
            )?)
        } else {
            None
        };

        let mut args: Vec<ConcreteExpr> = Vec::new();

        for arg in aggregation.args.into_iter().skip(1) {
            args.push(concretize_expression(arg, headers, None)?);
        }

        let method = ConcreteAggregationMethod::parse(&aggregation.func_name, args_count, &args)?;

        let concrete_aggregation = ConcreteAggregation {
            agg_name: aggregation.agg_name,
            method,
            expr,
            pair_expr,
        };

        concrete_aggregations.push(concrete_aggregation);
    }

    Ok(concrete_aggregations)
}

fn prepare(code: &str, headers: &ByteRecord) -> Result<ConcreteAggregations, ConcretizationError> {
    let parsed_aggregations = parse_aggregations(code).map_err(ConcretizationError::ParseError)?;

    concretize_aggregations(parsed_aggregations, headers)
}

// NOTE: each execution unit is iterated upon linearly to aggregate values
// all while running a minimum number of operations (batched by 1. expression
// keys and 2. composite aggregation atom).
#[derive(Debug, Clone)]
struct PlannerExecutionUnit {
    expr: Option<ConcreteExpr>,
    pair_expr: Option<ConcreteExpr>,
    aggregator_blueprint: CompositeAggregator,
}

impl PlannerExecutionUnit {
    fn key(&self) -> (&Option<ConcreteExpr>, &Option<ConcreteExpr>) {
        (&self.expr, &self.pair_expr)
    }

    fn used_column_indices(&self, scratch: &mut Vec<usize>) {
        if let Some(expr) = &self.expr {
            expr.used_column_indices(scratch);
        }

        if let Some(expr) = &self.pair_expr {
            expr.used_column_indices(scratch);
        }
    }
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
                .position(|unit| unit.key() == agg.key())
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

    fn instantiate_single_aggregator(&self) -> CompositeAggregator {
        debug_assert!(self.execution_plan.len() == 1);

        self.execution_plan[0].aggregator_blueprint.clone()
    }

    fn headers(&self) -> impl Iterator<Item = &[u8]> {
        self.output_plan.iter().map(|unit| unit.agg_name.as_bytes())
    }

    fn results<'a>(
        &'a self,
        aggregators: &'a [CompositeAggregator],
        headers_index: &'a HeadersIndex,
    ) -> impl Iterator<Item = Result<DynamicValue, SpecifiedEvaluationError>> + 'a {
        self.output_plan.iter().map(move |unit| {
            aggregators[unit.expr_index].get_final_value(
                unit.aggregator_index,
                &unit.agg_method,
                headers_index,
            )
        })
    }
}

// NOTE: parallelizing "horizontally" the planner's execution units does not
// seem to yield any performance increase. I guess the overhead is greater than
// the inner computation time.
fn run_with_record_on_aggregators<'a>(
    planner: &ConcreteAggregationPlanner,
    aggregators: impl Iterator<Item = &'a mut CompositeAggregator>,
    index: usize,
    record: &ByteRecord,
    headers_index: &HeadersIndex,
    last_value: Option<DynamicValue>,
) -> Result<(), SpecifiedEvaluationError> {
    for (unit, aggregator) in planner.execution_plan.iter().zip(aggregators) {
        let value = match &unit.expr {
            None => None,
            Some(expr) => Some(eval_expression_with_optional_last_value(
                expr,
                Some(index),
                record,
                headers_index,
                last_value.clone(),
            )?),
        };

        if let Some(pair_expr) = &unit.pair_expr {
            let second_value = eval_expression_with_optional_last_value(
                pair_expr,
                Some(index),
                record,
                headers_index,
                last_value,
            )?;

            return aggregator
                .process_pair(index, value.unwrap(), second_value)
                .map_err(|err| err.specify("<agg-expr>"));
        }

        if let Some(DynamicValue::List(list)) = value {
            for v in Arc::into_inner(list).unwrap() {
                aggregator
                    .process_value(index, Some(v), record)
                    .map_err(|err| err.specify("<agg-expr>"))?;
            }
        } else {
            aggregator
                .process_value(index, value, record)
                .map_err(|err| err.specify("<agg-expr>"))?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct AggregationProgram {
    aggregators: Vec<CompositeAggregator>,
    len: usize,
    planner: ConcreteAggregationPlanner,
    headers_index: HeadersIndex,
    last_value: DynamicValue,
}

impl AggregationProgram {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let concrete_aggregations = prepare(code, headers)?;
        let len = concrete_aggregations.len();
        let planner = ConcreteAggregationPlanner::from(concrete_aggregations);
        let aggregators = planner.instantiate_aggregators();

        Ok(Self {
            planner,
            aggregators,
            len,
            headers_index: HeadersIndex::from_headers(headers),
            last_value: DynamicValue::empty_bytes(),
        })
    }

    pub fn has_single_expr(&self) -> bool {
        self.len == 1
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
            self.aggregators.iter_mut(),
            index,
            record,
            &self.headers_index,
            None,
        )
    }

    pub fn run_with_cell(
        &mut self,
        index: usize,
        record: &ByteRecord,
        cell: &[u8],
    ) -> Result<(), SpecifiedEvaluationError> {
        self.last_value.set_bytes(cell);

        run_with_record_on_aggregators(
            &self.planner,
            self.aggregators.iter_mut(),
            index,
            record,
            &self.headers_index,
            Some(self.last_value.clone()),
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

        for value in self.planner.results(&self.aggregators, &self.headers_index) {
            record.push_field(&value?.serialize_as_bytes());
        }

        Ok(record)
    }
}

#[derive(Debug, Clone)]
pub struct GroupAggregationProgram<K> {
    planner: ConcreteAggregationPlanner,
    groups: ClusteredInsertHashmap<K, Vec<CompositeAggregator>>,
    headers_index: HeadersIndex,
    len: usize,
    dummy_record: ByteRecord,
    last_value: DynamicValue,
}

impl<K: Eq + Hash> GroupAggregationProgram<K> {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let concrete_aggregations = prepare(code, headers)?;
        let len = concrete_aggregations.len();
        let planner = ConcreteAggregationPlanner::from(concrete_aggregations);

        Ok(Self {
            planner,
            groups: ClusteredInsertHashmap::new(),
            headers_index: HeadersIndex::from_headers(headers),
            len,
            dummy_record: ByteRecord::new(),
            last_value: DynamicValue::empty_bytes(),
        })
    }

    pub fn parse_without_headers(code: &str) -> Result<Self, ConcretizationError> {
        Self::parse(code, &ByteRecord::new())
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn has_single_expr(&self) -> bool {
        self.len == 1
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
        group: K,
        index: usize,
        record: &ByteRecord,
    ) -> Result<(), SpecifiedEvaluationError> {
        let planner = &self.planner;

        let aggregators = self
            .groups
            .insert_with(group, || planner.instantiate_aggregators());

        run_with_record_on_aggregators(
            &self.planner,
            aggregators.iter_mut(),
            index,
            record,
            &self.headers_index,
            None,
        )
    }

    pub fn run_with_cells<'a>(
        &mut self,
        group: K,
        index: usize,
        record: &ByteRecord,
        cells: impl Iterator<Item = &'a [u8]>,
    ) -> Result<(), SpecifiedEvaluationError> {
        let planner = &self.planner;

        let aggregators = self
            .groups
            .insert_with(group, || planner.instantiate_aggregators());

        for cell in cells {
            self.last_value.set_bytes(cell);

            run_with_record_on_aggregators(
                &self.planner,
                aggregators.iter_mut(),
                index,
                record,
                &self.headers_index,
                Some(self.last_value.clone()),
            )?;
        }

        Ok(())
    }

    pub fn run_with<T: Into<DynamicValue>>(
        &mut self,
        group: K,
        index: usize,
        value: T,
    ) -> Result<(), SpecifiedEvaluationError> {
        let planner = &self.planner;

        let aggregators = self
            .groups
            .insert_with(group, || planner.instantiate_aggregators());

        run_with_record_on_aggregators(
            &self.planner,
            aggregators.iter_mut(),
            index,
            &self.dummy_record,
            &self.headers_index,
            Some(value.into()),
        )
    }

    pub fn headers(&self) -> impl Iterator<Item = &[u8]> {
        self.planner.headers()
    }

    pub fn iter(self) -> impl Iterator<Item = Result<(K, DynamicValue), SpecifiedEvaluationError>> {
        assert!(self.has_single_expr());

        let planner = self.planner;
        let headers_index = self.headers_index;

        self.groups
            .into_iter()
            .map(move |(group, mut aggregators)| {
                for aggregator in aggregators.iter_mut() {
                    aggregator.finalize(false);
                }

                planner
                    .results(&aggregators, &headers_index)
                    .next()
                    .unwrap()
                    .map(|value| (group, value))
            })
    }

    pub fn into_byte_records(
        self,
        parallel: bool,
    ) -> impl Iterator<Item = Result<(K, ByteRecord), SpecifiedEvaluationError>> {
        let planner = self.planner;
        let headers_index = self.headers_index;

        self.groups
            .into_iter()
            .map(move |(group, mut aggregators)| {
                for aggregator in aggregators.iter_mut() {
                    aggregator.finalize(parallel);
                }

                let mut record = ByteRecord::new();

                for value in planner.results(&aggregators, &headers_index) {
                    record.push_field(&value?.serialize_as_bytes());
                }

                Ok((group, record))
            })
    }
}

#[derive(Debug, Clone, Default)]
struct PivotedColumnNamesIndex {
    names: ByteRecord,
    seen: BTreeSet<Vec<u8>>,
}

impl PivotedColumnNamesIndex {
    fn add(&mut self, name: &[u8]) {
        if !self.seen.contains(name) {
            self.seen.insert(name.to_vec());
            self.names.push_field(name);
        }
    }
}

type GroupKey = Vec<Vec<u8>>;

#[derive(Debug, Clone)]
pub struct PivotAggregationProgram {
    planner: ConcreteAggregationPlanner,
    groups: IndexMap<GroupKey, BTreeMap<Vec<u8>, CompositeAggregator>, RandomState>,
    headers_index: HeadersIndex,
    pivoted_column_names_index: PivotedColumnNamesIndex,
}

impl PivotAggregationProgram {
    pub fn parse(code: &str, headers: &ByteRecord) -> Result<Self, ConcretizationError> {
        let concrete_aggregations = prepare(code, headers)?;

        if concrete_aggregations.len() != 1 {
            return Err(ConcretizationError::Custom(format!(
                "expected a single aggregation clause, but got {}",
                concrete_aggregations.len()
            )));
        }

        let planner = ConcreteAggregationPlanner::from(concrete_aggregations);

        Ok(Self {
            planner,
            groups: IndexMap::with_hasher(RandomState::new()),
            headers_index: HeadersIndex::from_headers(headers),
            pivoted_column_names_index: PivotedColumnNamesIndex::default(),
        })
    }

    pub fn used_column_indices(&self) -> Vec<usize> {
        let mut indices = Vec::new();

        for unit in self.planner.execution_plan.iter() {
            unit.used_column_indices(&mut indices);
        }

        indices.sort();
        indices.dedup();

        indices
    }

    pub fn run_with_record(
        &mut self,
        group: GroupKey,
        pivot: Vec<u8>,
        index: usize,
        record: &ByteRecord,
    ) -> Result<(), SpecifiedEvaluationError> {
        self.pivoted_column_names_index.add(&pivot);

        let planner = &self.planner;

        let pivot_map = self.groups.entry(group).or_default();

        let aggregator = pivot_map
            .entry(pivot)
            .or_insert_with(|| planner.instantiate_single_aggregator());

        run_with_record_on_aggregators(
            planner,
            once(aggregator),
            index,
            record,
            &self.headers_index,
            None,
        )
    }

    pub fn pivoted_column_names(&self) -> &ByteRecord {
        &self.pivoted_column_names_index.names
    }

    pub fn flush<F, E>(self, mut callback: F) -> Result<(), E>
    where
        F: FnMut(&ByteRecord) -> Result<(), E>,
        E: From<SpecifiedEvaluationError>,
    {
        let mut record = ByteRecord::new();
        let names = self.pivoted_column_names_index.names;

        for (group, mut pivot_map) in self.groups.into_iter() {
            record.clear();

            for cell in group {
                record.push_field(&cell);
            }

            for name in names.iter() {
                if let Some(aggregator) = pivot_map.get_mut(name) {
                    aggregator.finalize(false);

                    for value in self
                        .planner
                        .results(std::slice::from_ref(aggregator), &self.headers_index)
                    {
                        record.push_field(&value?.serialize_as_bytes());
                    }
                } else {
                    record.push_field(b"");
                }
            }

            callback(&record)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct GroupAlongColumnsAggregationProgram {
    planner: ConcreteAggregationPlanner,
    cols: usize,
    groups: ClusteredInsertHashmap<GroupKey, Vec<CompositeAggregator>>,
    headers_index: HeadersIndex,
    last_value: DynamicValue,
}

impl GroupAlongColumnsAggregationProgram {
    pub fn parse(
        code: &str,
        headers: &ByteRecord,
        cols: usize,
    ) -> Result<Self, ConcretizationError> {
        let concrete_aggregations = prepare(code, headers)?;

        if concrete_aggregations.len() != 1 {
            return Err(ConcretizationError::Custom(format!(
                "expected a single aggregation clause, but got {}",
                concrete_aggregations.len()
            )));
        }

        let planner = ConcreteAggregationPlanner::from(concrete_aggregations);

        Ok(Self {
            planner,
            cols,
            groups: ClusteredInsertHashmap::new(),
            headers_index: HeadersIndex::from_headers(headers),
            last_value: DynamicValue::empty_bytes(),
        })
    }

    pub fn run_with_cells<'a>(
        &mut self,
        group: GroupKey,
        index: usize,
        record: &ByteRecord,
        cells: impl Iterator<Item = &'a [u8]>,
    ) -> Result<(), SpecifiedEvaluationError> {
        let planner = &self.planner;

        let aggregators_per_cell = self.groups.insert_with(group, || {
            (0..self.cols)
                .map(|_| planner.instantiate_single_aggregator())
                .collect()
        });

        for (cell, aggregator) in cells.zip(aggregators_per_cell) {
            self.last_value.set_bytes(cell);

            run_with_record_on_aggregators(
                &self.planner,
                once(aggregator),
                index,
                record,
                &self.headers_index,
                Some(self.last_value.clone()),
            )?;
        }

        Ok(())
    }

    pub fn into_byte_records(
        self,
        parallel: bool,
    ) -> impl Iterator<Item = Result<(GroupKey, ByteRecord), SpecifiedEvaluationError>> {
        let planner = self.planner;
        let headers_index = self.headers_index;

        self.groups
            .into_iter()
            .map(move |(group, mut aggregator_per_cell)| {
                for aggregator in aggregator_per_cell.iter_mut() {
                    aggregator.finalize(parallel);
                }

                let mut record = ByteRecord::new();

                for aggregator in aggregator_per_cell.iter() {
                    for value in planner.results(std::slice::from_ref(aggregator), &headers_index) {
                        record.push_field(&value?.serialize_as_bytes());
                    }
                }

                Ok((group, record))
            })
    }
}
