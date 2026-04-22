use std::borrow::Cow;

use bstr::ByteSlice;

use crate::moonblade::types::{BoundArguments, BoundStringLike, DynamicValue};

use super::FunctionResult;

pub fn split(args: BoundArguments) -> FunctionResult {
    let to_split = args.get(0).unwrap().try_as_str()?;
    let pattern_arg = args.get(1).unwrap();
    let count = args.get(2);

    let splitted: Vec<DynamicValue> = if let Some(pattern) = pattern_arg.as_regex() {
        if let Some(c) = count {
            pattern
                .splitn(&to_split, c.try_as_usize()? + 1)
                .map(DynamicValue::from)
                .collect()
        } else {
            pattern.split(&to_split).map(DynamicValue::from).collect()
        }
    } else {
        let pattern = pattern_arg.try_as_str()?;

        if let Some(c) = count {
            to_split
                .splitn(c.try_as_usize()? + 1, pattern.as_ref())
                .map(DynamicValue::from)
                .collect()
        } else {
            to_split.split(&*pattern).map(DynamicValue::from).collect()
        }
    };

    Ok(DynamicValue::from(splitted))
}

pub fn lower(args: BoundArguments) -> FunctionResult {
    let arg = args.get1();

    Ok(match arg.as_string_like() {
        Some(BoundStringLike::Bytes(bytes)) => DynamicValue::from_owned_bytes(bytes.to_lowercase()),
        Some(BoundStringLike::String(string)) => string.to_lowercase().into(),
        None => arg.try_as_str()?.to_lowercase().into(),
    })
}

pub fn upper(args: BoundArguments) -> FunctionResult {
    let arg = args.get1();

    Ok(match arg.as_string_like() {
        Some(BoundStringLike::Bytes(bytes)) => DynamicValue::from_owned_bytes(bytes.to_uppercase()),
        Some(BoundStringLike::String(string)) => string.to_uppercase().into(),
        None => arg.try_as_str()?.to_uppercase().into(),
    })
}

pub fn count(args: BoundArguments) -> FunctionResult {
    let (arg1, arg2) = args.get2();

    let string = arg1.try_as_str()?;

    match arg2.as_regex() {
        Some(regex) => Ok(DynamicValue::from(regex.find_iter(&string).count())),
        None => {
            let pattern = arg2.try_as_str()?;

            Ok(DynamicValue::from(string.matches(pattern.as_ref()).count()))
        }
    }
}

pub fn startswith(args: BoundArguments) -> FunctionResult {
    let (string, pattern) = args.get2_str()?;

    Ok(DynamicValue::from(string.starts_with(pattern.as_ref())))
}

pub fn endswith(args: BoundArguments) -> FunctionResult {
    let (string, pattern) = args.get2_str()?;

    Ok(DynamicValue::from(string.ends_with(pattern.as_ref())))
}

pub fn join(args: BoundArguments) -> FunctionResult {
    let (arg1, arg2) = args.get2();

    let list = arg1.try_as_list()?;
    let joiner = arg2.try_as_str()?;

    let mut string_list: Vec<Cow<str>> = Vec::with_capacity(list.len());

    for value in list.iter() {
        string_list.push(value.try_as_str()?);
    }

    Ok(DynamicValue::from(string_list.join(&joiner)))
}

pub fn regex_match(args: BoundArguments) -> FunctionResult {
    let haystack = args.get(0).unwrap().try_as_str()?;
    let pattern = args.get(1).unwrap().try_as_regex()?;
    let group = args
        .get(2)
        .map(|v| v.try_as_usize())
        .transpose()?
        .unwrap_or(0);

    if let Some(caps) = pattern.captures(haystack.as_ref()) {
        Ok(DynamicValue::from(caps.get(group).map(|g| g.as_str())))
    } else {
        Ok(DynamicValue::None)
    }
}

pub fn replace(args: BoundArguments) -> FunctionResult {
    let (arg1, arg2, arg3) = args.get3();

    let string = arg1.try_as_str()?;
    let replacement = arg3.try_as_str()?;

    let replaced = match arg2.as_regex() {
        Some(regex) => regex.replace_all(&string, replacement).into_owned(),
        None => {
            let pattern = arg2.try_as_str()?;

            string.replace(&*pattern, &replacement)
        }
    };

    Ok(DynamicValue::from(replaced))
}
