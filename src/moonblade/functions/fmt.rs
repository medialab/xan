use bstr::ByteSlice;
use std::borrow::Cow;

use lazy_static::lazy_static;

use super::FunctionResult;
use crate::moonblade::error::EvaluationError;
use crate::moonblade::types::{BoundArguments, BoundStringLike, DynamicValue};

macro_rules! make_trim_fn {
    ($name: ident, $trim: ident, $trim_matches: ident) => {
        pub fn $name(args: BoundArguments) -> FunctionResult {
            let chars_opt = args.get(1);

            Ok(match chars_opt {
                None => {
                    let arg = args.get1();

                    match arg.as_string_like() {
                        Some(BoundStringLike::Bytes(bytes)) => bytes.$trim().into(),
                        Some(BoundStringLike::String(string)) => string.$trim().into(),
                        None => arg.try_as_str()?.$trim().into(),
                    }
                }
                Some(chars) => {
                    let pattern = chars.try_as_str()?;
                    DynamicValue::from(
                        args.get1_str()?
                            .$trim_matches(|c| pattern.chars().any(|p| p == c)),
                    )
                }
            })
        }
    };
}

make_trim_fn!(trim, trim, trim_matches);
make_trim_fn!(ltrim, trim_start, trim_start_matches);
make_trim_fn!(rtrim, trim_end, trim_end_matches);

pub fn pad(alignment: pad::Alignment, args: BoundArguments) -> FunctionResult {
    use pad::PadStr;

    let mut args_iter = args.into_iter();
    let first_arg = args_iter.next().unwrap();
    let string = first_arg.try_as_str()?;

    let width = args_iter.next().unwrap().try_as_usize()?;
    let padding_char = match args_iter.next() {
        None => ' ',
        Some(value) => {
            let padding_string = value.try_as_str()?;

            match padding_string.chars().count() {
                0 => {
                    return Err(EvaluationError::Custom(
                        "provided padding char is empty".to_string(),
                    ));
                }
                1 => padding_string.chars().next().unwrap(),
                2.. => {
                    return Err(EvaluationError::Custom(
                        "provided padding char is longer than a char".to_string(),
                    ));
                }
            }
        }
    };

    Ok(DynamicValue::from(string.pad(
        width,
        padding_char,
        alignment,
        false,
    )))
}

lazy_static! {
    static ref FMT_PATTERN: regex::Regex = regex::Regex::new(r"\{([A-Za-z_]*)\}").unwrap();
}

pub fn fmt(args: BoundArguments) -> FunctionResult {
    let pattern = args.get1().try_as_str()?;

    let (substitution_map, substitution_list) = if args.len() > 1 {
        let arg = args.get(1).unwrap();

        (arg.as_map(), arg.as_list())
    } else {
        (None, None)
    };

    if (substitution_map.is_some() || substitution_list.is_some()) && args.len() > 2 {
        Err("expecting only a single substitution map or list")?;
    }

    let mut formatted = String::with_capacity(pattern.len());
    let mut current_positional: usize = if substitution_list.is_some() { 0 } else { 1 };
    let mut last_match = 0;

    for capture in FMT_PATTERN.captures_iter(&pattern) {
        let m = capture.get(0).unwrap();
        let fallback = &capture[0];

        formatted.push_str(&pattern[last_match..m.start()]);

        match capture.get(1).unwrap().as_str() {
            "" => {
                if let Some(list) = substitution_list {
                    formatted.push_str(
                        &list
                            .get(current_positional)
                            .map(|arg| arg.try_as_str())
                            .transpose()?
                            .unwrap_or(Cow::Borrowed(fallback)),
                    );
                } else {
                    formatted.push_str(
                        &args
                            .get(current_positional)
                            .map(|arg| arg.try_as_str())
                            .transpose()?
                            .unwrap_or(Cow::Borrowed(fallback)),
                    );
                }

                current_positional += 1;
            }
            key => {
                if let Some(map) = &substitution_map {
                    if let Some(sub) = map.get(key) {
                        formatted.push_str(&sub.try_as_str()?);
                    } else {
                        formatted.push_str(fallback);
                    }
                } else {
                    formatted.push_str(fallback);
                }
            }
        };

        last_match = m.end();
    }

    formatted.push_str(&pattern[last_match..]);

    Ok(DynamicValue::from(formatted))
}

pub fn fmt_number(args: BoundArguments) -> FunctionResult {
    let mut args_iter = args.into_iter();
    let number = args_iter.next().unwrap().try_as_number()?;

    let thousands_sep = args_iter.next().unwrap();
    let comma = args_iter.next().unwrap();
    let significance = args_iter.next().unwrap();

    if !thousands_sep.is_none() || !comma.is_none() || !significance.is_none() {
        let mut formatter = numfmt::Formatter::new()
            .separator(',')
            .unwrap()
            .comma(comma.is_truthy());

        let separator = if comma.is_truthy() { '.' } else { ',' };

        if !significance.is_none() {
            formatter = formatter.precision(numfmt::Precision::Significance(
                significance.try_as_usize()? as u8,
            ));
        } else {
            formatter = formatter.precision(numfmt::Precision::Significance(5));
        }

        let formatted = crate::utils::format_number_with_formatter(&mut formatter, number);

        if !thousands_sep.is_none() {
            Ok(DynamicValue::from(
                formatted.replace(separator, &thousands_sep.try_as_str()?),
            ))
        } else {
            Ok(DynamicValue::from(formatted))
        }
    } else {
        Ok(DynamicValue::from(crate::utils::format_number(number)))
    }
}

pub fn printf(args: BoundArguments) -> FunctionResult {
    let l = args.len() - 1;

    let mut args_iter = args.into_iter();
    let fmt_arg = args_iter.next().unwrap();
    let fmt = fmt_arg.try_as_str()?;

    let mut fmt_args: Vec<Box<dyn sprintf::Printf>> = Vec::with_capacity(l);

    fn arg_to_printf(arg: &DynamicValue) -> Result<Box<dyn sprintf::Printf>, EvaluationError> {
        Ok(match arg {
            DynamicValue::Integer(i) => Box::new(*i),
            DynamicValue::Float(f) => Box::new(*f),
            _ => Box::new(arg.try_as_str()?.into_owned()),
        })
    }

    for arg in args_iter {
        if let Some(list) = arg.as_list() {
            for sub_arg in list.iter() {
                fmt_args.push(arg_to_printf(sub_arg)?);
            }
        } else {
            fmt_args.push(arg_to_printf(&arg.into_owned())?);
        }
    }

    let fmt_args_refs = fmt_args.iter().map(|b| b.as_ref()).collect::<Vec<_>>();

    match sprintf::vsprintf(&fmt, &fmt_args_refs) {
        Ok(string) => Ok(DynamicValue::from(string)),
        Err(error) => Err(EvaluationError::Custom(error.to_string())),
    }
}

pub fn to_fixed(mut args: BoundArguments) -> FunctionResult {
    let (arg1, arg2) = args.pop2();

    let n = arg1.try_as_f64()?;
    let p = arg2.try_as_usize()?.min(16);

    let formatted = format!("{n:.p$}");

    Ok(DynamicValue::from(formatted))
}

pub fn escape_regex(args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(regex::escape(args.get1_str()?.as_ref())))
}
