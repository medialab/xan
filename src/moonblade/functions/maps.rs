use std::borrow::Cow;

use crate::collections::HashMap;

use crate::moonblade::error::EvaluationError;
use crate::moonblade::types::{BoundArguments, DynamicValue};

use super::FunctionResult;

fn get_subroutine<'v>(
    target: &'v DynamicValue,
    key: &DynamicValue,
) -> Result<Option<Cow<'v, DynamicValue>>, EvaluationError> {
    Ok(match target {
        DynamicValue::String(value) => {
            let mut index = key.try_as_i64()?;

            if index < 0 {
                index += value.len() as i64;
            }

            if index < 0 {
                None
            } else {
                value
                    .chars()
                    .nth(index as usize)
                    .map(DynamicValue::from)
                    .map(Cow::Owned)
            }
        }
        DynamicValue::Bytes(value) => {
            let mut index = key.try_as_i64()?;

            if index < 0 {
                index += value.len() as i64;
            }

            if index < 0 {
                None
            } else {
                let value =
                    std::str::from_utf8(value).map_err(|_| EvaluationError::UnicodeDecodeError)?;

                value
                    .chars()
                    .nth(index as usize)
                    .map(DynamicValue::from)
                    .map(Cow::Owned)
            }
        }
        DynamicValue::List(list) => {
            let mut index = key.try_as_i64()?;

            if index < 0 {
                index += list.len() as i64;
            }

            if index < 0 {
                None
            } else {
                list.get(index as usize).map(Cow::Borrowed)
            }
        }
        DynamicValue::Map(map) => {
            let key = key.try_as_str()?;

            map.get(key.as_ref()).map(Cow::Borrowed)
        }
        value => return Err(EvaluationError::from_cast(value, "sequence")),
    })
}

pub fn get(mut args: BoundArguments) -> FunctionResult {
    let (target, key, default) = if args.len() == 3 {
        let (target, key, default) = args.pop3();

        (target, key, Some(default))
    } else {
        let (target, key) = args.pop2();

        (target, key, None)
    };

    let mut owned_value = Some(target);

    match key {
        DynamicValue::List(path) => {
            let mut current = owned_value.as_ref().unwrap();

            for step in path.iter() {
                match get_subroutine(current, step)? {
                    None => return Ok(default.unwrap_or_default()),
                    Some(next) => match next {
                        Cow::Owned(owned) => {
                            owned_value = Some(owned);
                            current = owned_value.as_ref().unwrap();
                        }
                        Cow::Borrowed(borrowed) => {
                            current = borrowed;
                        }
                    },
                }
            }

            Ok(match owned_value {
                Some(owned) if std::ptr::eq(&owned, current) => owned,
                _ => current.clone(),
            })
        }
        _ => Ok(get_subroutine(owned_value.as_ref().unwrap(), &key)?
            .map(|v| v.into_owned())
            .unwrap_or_else(|| default.unwrap_or_default())),
    }
}

pub fn contains(args: BoundArguments) -> FunctionResult {
    let (arg1, arg2) = args.get2();

    match arg1 {
        DynamicValue::String(text) => match arg2 {
            DynamicValue::Regex(pattern) => Ok(DynamicValue::from(pattern.is_match(text))),
            _ => {
                let pattern = arg2.try_as_str()?;
                Ok(DynamicValue::from(text.contains(pattern.as_ref())))
            }
        },
        DynamicValue::Bytes(bytes) => {
            let text =
                std::str::from_utf8(bytes).map_err(|_| EvaluationError::UnicodeDecodeError)?;

            match arg2 {
                DynamicValue::Regex(pattern) => Ok(DynamicValue::from(pattern.is_match(text))),
                _ => {
                    let pattern = arg2.try_as_str()?;
                    Ok(DynamicValue::from(text.contains(pattern.as_ref())))
                }
            }
        }
        DynamicValue::List(list) => {
            let needle = arg2.try_as_str()?;

            for item in list.iter() {
                if needle == item.try_as_str()? {
                    return Ok(DynamicValue::from(true));
                }
            }

            Ok(DynamicValue::from(false))
        }
        DynamicValue::Map(map) => {
            let needle = arg2.try_as_str()?;

            Ok(DynamicValue::from(map.contains_key(needle.as_ref())))
        }
        value => Err(EvaluationError::from_cast(value, "sequence")),
    }
}

pub fn keys(args: BoundArguments) -> FunctionResult {
    let map = args.get1().try_as_map()?;

    Ok(DynamicValue::from(
        map.keys()
            .map(|k| DynamicValue::from(k.as_str()))
            .collect::<Vec<_>>(),
    ))
}

pub fn values(args: BoundArguments) -> FunctionResult {
    let map = args.get1().try_as_map()?;

    Ok(DynamicValue::from(
        map.values().cloned().collect::<Vec<_>>(),
    ))
}

pub fn index_by(args: BoundArguments) -> FunctionResult {
    let list = args.get1().try_as_list()?;
    let key = args.get(1).unwrap().try_as_str()?;

    let mut map: HashMap<String, DynamicValue> = HashMap::new();

    for item in list {
        let record = item.try_as_map()?;

        if let Some(value) = record.get(key.as_ref()) {
            map.insert(value.try_as_str()?.into_owned(), item.clone());
        }
    }

    Ok(DynamicValue::from(map))
}
