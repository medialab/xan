use std::cmp::max;
use std::sync::Arc;

use crate::moonblade::error::EvaluationError;
use crate::moonblade::types::{BoundArgument, BoundArguments, BoundContainer, DynamicValue};

use super::FunctionResult;

pub fn len(mut args: BoundArguments) -> FunctionResult {
    let arg = args.pop1();

    let l = match arg.as_value() {
        Some(DynamicValue::List(list)) => list.len(),
        Some(DynamicValue::Map(map)) => map.len(),
        _ => arg.try_as_str()?.chars().count(),
    };

    Ok(l.into())
}

pub fn range(args: BoundArguments) -> FunctionResult {
    let (start, stop, step): (i64, i64, i64) = if args.len() == 3 {
        (
            args.get(0).unwrap().try_as_i64()?,
            args.get(1).unwrap().try_as_i64()?,
            args.get(2).unwrap().try_as_i64()?,
        )
    } else if args.len() == 2 {
        (
            args.get(0).unwrap().try_as_i64()?,
            args.get(1).unwrap().try_as_i64()?,
            1,
        )
    } else {
        (0, args.get(0).unwrap().try_as_i64()?, 1)
    };

    if step == 0 {
        return Err(EvaluationError::Custom("step cannot be 0".to_string()));
    }

    let len = if step > 0 {
        if start >= stop {
            0
        } else {
            ((stop - start - 1) / step + 1) as usize
        }
    } else if start <= stop {
        0
    } else {
        ((start - stop - 1) / (-step) + 1) as usize
    };

    let mut indices = Vec::with_capacity(len);

    let mut current = start;

    if step > 0 {
        while current < stop {
            indices.push(DynamicValue::from(current));
            current += step;
        }
    } else {
        while current > stop {
            indices.push(DynamicValue::from(current));
            current += step;
        }
    }

    Ok(DynamicValue::from(indices))
}

pub fn repeat(args: BoundArguments) -> FunctionResult {
    let (to_repeat_arg, times_arg) = args.get2();

    let times = times_arg.try_as_usize()?;

    if let Some(items) = to_repeat_arg.as_list() {
        let mut repeated = Vec::with_capacity(items.len() * times);

        for _ in 0..times {
            for item in items.iter() {
                repeated.push(item.clone());
            }
        }

        Ok(DynamicValue::from(repeated))
    } else {
        let to_repeat = to_repeat_arg.try_as_str()?;

        let mut repeated = String::with_capacity(to_repeat.len() * times);

        for _ in 0..times {
            repeated.push_str(&to_repeat);
        }

        Ok(DynamicValue::from(repeated))
    }
}

pub fn first(mut args: BoundArguments) -> FunctionResult {
    let arg = args.pop1();

    Ok(match arg.try_as_container()? {
        BoundContainer::String(string) => string.chars().next().into(),
        BoundContainer::Bytes(bytes) => std::str::from_utf8(bytes)?.chars().next().into(),
        BoundContainer::List(list) => list.first().cloned().into(),
        _ => {
            return Err(EvaluationError::Cast {
                from_value: arg.into_owned(),
                to_type: "sequence".to_string(),
            })
        }
    })
}

pub fn last(mut args: BoundArguments) -> FunctionResult {
    let arg = args.pop1();

    Ok(match arg.try_as_container()? {
        BoundContainer::String(string) => string.chars().next_back().into(),
        BoundContainer::Bytes(bytes) => std::str::from_utf8(bytes)?.chars().next_back().into(),
        BoundContainer::List(list) => list.last().cloned().into(),
        _ => {
            return Err(EvaluationError::Cast {
                from_value: arg.into_owned(),
                to_type: "sequence".to_string(),
            })
        }
    })
}

pub fn slice(args: BoundArguments) -> FunctionResult {
    let target = args.get(0).unwrap();

    if let Some(list) = target.as_list() {
        let mut lo = args.get(1).unwrap().try_as_i64()?;
        let opt_hi = args.get(2);

        let sublist: Vec<DynamicValue> = match opt_hi {
            None => {
                if lo < 0 {
                    let l = list.len();
                    lo = max(0, l as i64 + lo);

                    list[..lo as usize].to_vec()
                } else if lo >= list.len() as i64 {
                    Vec::new()
                } else {
                    list[..lo as usize].to_vec()
                }
            }
            Some(hi_value) => {
                let mut hi = hi_value.try_as_i64()?;

                if lo >= list.len() as i64 {
                    Vec::new()
                } else if lo < 0 {
                    let l = list.len();

                    lo = max(0, l as i64 + lo);

                    if hi < 0 {
                        hi = max(0, l as i64 + hi);
                    }

                    if hi <= lo {
                        Vec::new()
                    } else {
                        list[lo as usize..hi.min(list.len() as i64) as usize].to_vec()
                    }
                } else {
                    if hi < 0 {
                        let l = list.len();
                        hi = max(0, l as i64 + hi);
                    }

                    if hi <= lo {
                        Vec::new()
                    } else {
                        list[lo as usize..hi.min(list.len() as i64) as usize].to_vec()
                    }
                }
            }
        };

        return Ok(DynamicValue::from(sublist));
    }

    let string = target.try_as_str()?;

    let mut lo = args.get(1).unwrap().try_as_i64()?;
    let opt_hi = args.get(2);

    let chars = string.chars();

    let substring: String = match opt_hi {
        None => {
            if lo < 0 {
                let l = string.chars().count();
                lo = max(0, l as i64 + lo);

                chars.skip(lo as usize).collect()
            } else {
                chars.skip(lo as usize).collect()
            }
        }
        Some(hi_value) => {
            let mut hi = hi_value.try_as_i64()?;

            if lo < 0 {
                let l = string.chars().count();
                lo = max(0, l as i64 + lo);

                if hi < 0 {
                    hi = max(0, l as i64 + hi);
                }

                if hi <= lo {
                    "".to_string()
                } else {
                    chars.skip(lo as usize).take((hi - lo) as usize).collect()
                }
            } else {
                if hi < 0 {
                    let l = string.chars().count();
                    hi = max(0, l as i64 + hi);
                }

                chars.skip(lo as usize).take((hi - lo) as usize).collect()
            }
        }
    };

    Ok(DynamicValue::from(substring))
}

fn concat_str<'b>(first: &str, iter: impl Iterator<Item = BoundArgument<'b>>) -> FunctionResult {
    let mut output = String::from(first);

    for arg in iter {
        output.push_str(&arg.try_as_str()?);
    }

    Ok(output.into())
}

// TODO: deal with allocation reuse when dealing with raw Bytes
pub fn concat(args: BoundArguments) -> FunctionResult {
    let mut args_iter = args.into_iter();
    let first = args_iter.next().unwrap();

    if first.as_list().is_some() {
        match first {
            BoundArgument::Owned(DynamicValue::List(mut list)) => {
                if let Some(list_ref) = Arc::get_mut(&mut list) {
                    for arg in args_iter {
                        list_ref.push(arg.into_owned());
                    }

                    Ok(DynamicValue::List(list))
                } else {
                    let mut new_list = Vec::clone(&list);

                    for arg in args_iter {
                        new_list.push(arg.into_owned());
                    }

                    Ok(new_list.into())
                }
            }
            BoundArgument::Borrowed(DynamicValue::List(list)) => {
                let mut new_list = Vec::clone(list);

                for arg in args_iter {
                    new_list.push(arg.into_owned());
                }

                Ok(new_list.into())
            }
            _ => unreachable!(),
        }
    } else {
        match first {
            BoundArgument::Owned(DynamicValue::String(mut string)) => {
                if let Some(string_ref) = Arc::get_mut(&mut string) {
                    for arg in args_iter {
                        string_ref.push_str(&arg.try_as_str()?);
                    }

                    Ok(DynamicValue::String(string))
                } else {
                    concat_str(&string, args_iter)
                }
            }
            _ => concat_str(&first.try_as_str()?, args_iter),
        }
    }
}

pub fn compact(mut args: BoundArguments) -> FunctionResult {
    let arg = args.pop1();

    // TODO: this pattern, and the one of concat can probably be abstracted in a method
    Ok(match arg {
        BoundArgument::Borrowed(DynamicValue::List(list)) => list
            .iter()
            .filter(|value| value.is_truthy())
            .cloned()
            .collect::<Vec<_>>()
            .into(),
        BoundArgument::Owned(DynamicValue::List(mut list)) => match Arc::get_mut(&mut list) {
            Some(list_ref) => {
                list_ref.retain(|value| value.is_truthy());
                DynamicValue::List(list)
            }
            None => list
                .iter()
                .filter(|value| value.is_truthy())
                .cloned()
                .collect::<Vec<_>>()
                .into(),
        },
        _ => {
            return Err(EvaluationError::Cast {
                from_value: arg.into_owned(),
                to_type: "list".to_string(),
            })
        }
    })
}
