use std::borrow::Cow;

use crate::collections::HashMap;

use crate::moonblade::error::EvaluationError;
use crate::moonblade::types::{BoundArgument, BoundArguments, BoundContainer, DynamicValue};

use super::FunctionResult;

fn get_subroutine<'v>(
    target: &'v BoundContainer,
    key: &BoundArgument,
) -> Result<Option<Cow<'v, DynamicValue>>, EvaluationError> {
    Ok(match target {
        BoundContainer::String(value) => {
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
        BoundContainer::Bytes(value) => {
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
        BoundContainer::List(list) => {
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
        BoundContainer::Map(map) => {
            let key = key.try_as_str()?;

            map.get(key.as_ref()).map(Cow::Borrowed)
        }
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

    if let Some(path) = key.as_list() {
        // let mut owned_value = Some(target);

        // let mut current = owned_value.unwrap();

        // for step in path.iter() {
        //     let container = current.try_as_container()?;

        //     match get_subroutine(&container, &BoundArgument::Borrowed(step))? {
        //         None => return Ok(default.map(|d| d.into_owned()).unwrap_or_default()),
        //         Some(next) => match next {
        //             Cow::Owned(owned) => {
        //                 owned_value = Some(BoundArgument::Owned(owned));
        //                 current = owned_value.unwrap();
        //             }
        //             Cow::Borrowed(borrowed) => {
        //                 current = BoundArgument::Borrowed(borrowed);
        //             }
        //         },
        //     }
        // }

        return Ok(DynamicValue::None);

        // return Ok(match owned_value {
        //     Some(owned) if std::ptr::eq(&owned, current) => owned,
        //     _ => current.clone(),
        // });
    }

    let container = target.try_as_container()?;

    let out = get_subroutine(&container, &key)?;

    Ok(match out {
        Some(v) => v.into_owned(),
        None => default.map(|b| b.into_owned()).unwrap_or_default(),
    })
}

pub fn contains(args: BoundArguments) -> FunctionResult {
    let (arg1, arg2) = args.get2();

    let container = arg1.try_as_container()?;

    match container {
        BoundContainer::Map(map) => {
            let needle = arg2.try_as_str()?;

            Ok(map.contains_key(needle.as_ref()).into())
        }
        BoundContainer::List(list) => Ok(list.iter().any(|item| arg2.eq_value(item)).into()),
        BoundContainer::String(text) => match arg2.as_regex() {
            Some(pattern) => Ok(pattern.is_match(text).into()),
            None => Ok(text.contains(arg2.try_as_str()?.as_ref()).into()),
        },
        BoundContainer::Bytes(bytes) => {
            let text =
                std::str::from_utf8(bytes).map_err(|_| EvaluationError::UnicodeDecodeError)?;

            match arg2.as_regex() {
                Some(pattern) => Ok(pattern.is_match(text).into()),
                None => Ok(text.contains(arg2.try_as_str()?.as_ref()).into()),
            }
        }
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
