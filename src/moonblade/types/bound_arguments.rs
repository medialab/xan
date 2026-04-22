use std::borrow::Cow;

use arrayvec::ArrayVec;

use crate::collections::HashMap;
use crate::moonblade::error::EvaluationError;

use super::{DynamicNumber, DynamicValue};

pub enum BoundContainer<'a> {
    String(&'a str),
    Bytes(&'a [u8]),
    List(&'a Vec<DynamicValue>),
    Map(&'a HashMap<String, DynamicValue>),
}

pub enum BoundArgument<'a> {
    Owned(DynamicValue),
    Borrowed(&'a DynamicValue),
    Cell(&'a [u8]),
}

impl BoundArgument<'_> {
    #[inline]
    pub fn type_of(&self) -> &str {
        match self {
            Self::Owned(owned) => owned.type_of(),
            Self::Borrowed(borrowed) => borrowed.type_of(),
            Self::Cell(_) => "bytes",
        }
    }

    pub fn into_owned(self) -> DynamicValue {
        match self {
            Self::Owned(owned) => owned,
            Self::Borrowed(borrowed) => borrowed.clone(),
            Self::Cell(cell) => DynamicValue::from(cell),
        }
    }

    pub fn to_value(self) -> DynamicValue {
        match self {
            Self::Owned(owned) => owned,
            Self::Borrowed(borrowed) => borrowed.clone(),
            Self::Cell(cell) => cell.into(),
        }
    }

    #[inline]
    fn map<D, F, T>(&self, over_dynamic_value: D, over_cell: F) -> T
    where
        D: FnOnce(&DynamicValue) -> T,
        F: FnOnce(&[u8]) -> T,
    {
        match self {
            Self::Owned(owned) => over_dynamic_value(&owned),
            Self::Borrowed(borrowed) => over_dynamic_value(*borrowed),
            Self::Cell(cell) => over_cell(cell),
        }
    }

    #[inline]
    pub fn try_as_f64(&self) -> Result<f64, EvaluationError> {
        self.map(DynamicValue::try_as_f64, |cell| {
            if let Ok(f) = fast_float::parse::<f64, &[u8]>(cell) {
                Ok(f)
            } else {
                Err(EvaluationError::from_cell_cast(cell, "float"))
            }
        })
    }

    #[inline]
    pub fn try_as_i64(&self) -> Result<i64, EvaluationError> {
        self.map(DynamicValue::try_as_i64, |cell| {
            if let Ok(i) = btoi::btoi::<i64>(cell) {
                Ok(i)
            } else {
                Err(EvaluationError::from_cell_cast(cell, "integer"))
            }
        })
    }

    #[inline]
    pub fn try_as_number(&self) -> Result<DynamicNumber, EvaluationError> {
        self.map(DynamicValue::try_as_number, |cell| {
            if let Ok(n) = DynamicNumber::try_from(cell) {
                Ok(n)
            } else {
                Err(EvaluationError::from_cell_cast(cell, "number"))
            }
        })
    }

    #[inline]
    pub fn try_as_list(&self) -> Result<&Vec<DynamicValue>, EvaluationError> {
        match self {
            Self::Owned(owned) => owned.try_as_list(),
            Self::Borrowed(borrowed) => borrowed.try_as_list(),
            Self::Cell(cell) => Err(EvaluationError::from_cell_cast(cell, "list")),
        }
    }

    #[inline]
    pub fn try_as_map(&self) -> Result<&HashMap<String, DynamicValue>, EvaluationError> {
        match self {
            Self::Owned(owned) => owned.try_as_map(),
            Self::Borrowed(borrowed) => borrowed.try_as_map(),
            Self::Cell(cell) => Err(EvaluationError::from_cell_cast(cell, "map")),
        }
    }

    #[inline]
    pub fn try_as_bytes(&self) -> Result<&[u8], EvaluationError> {
        match self {
            Self::Owned(owned) => owned.try_as_bytes(),
            Self::Borrowed(borrowed) => borrowed.try_as_bytes(),
            Self::Cell(cell) => Ok(cell),
        }
    }

    #[inline]
    pub fn try_as_str(&self) -> Result<Cow<str>, EvaluationError> {
        match self {
            Self::Owned(owned) => owned.try_as_str(),
            Self::Borrowed(borrowed) => borrowed.try_as_str(),
            Self::Cell(cell) => match std::str::from_utf8(cell) {
                Ok(string) => Ok(Cow::Borrowed(string)),
                Err(_) => Err(EvaluationError::UnicodeDecodeError),
            },
        }
    }

    #[inline]
    pub fn try_as_container(&self) -> Result<BoundContainer, EvaluationError> {
        match self {
            Self::Owned(owned) => match owned {
                DynamicValue::String(string) => Ok(BoundContainer::String(string)),
                DynamicValue::Bytes(bytes) => Ok(BoundContainer::Bytes(bytes)),
                DynamicValue::List(list) => Ok(BoundContainer::List(list)),
                DynamicValue::Map(map) => Ok(BoundContainer::Map(map)),
                _ => Err(EvaluationError::from_cast(owned, "container")),
            },
            Self::Borrowed(borrowed) => match borrowed {
                DynamicValue::String(string) => Ok(BoundContainer::String(string)),
                DynamicValue::Bytes(bytes) => Ok(BoundContainer::Bytes(bytes)),
                DynamicValue::List(list) => Ok(BoundContainer::List(list)),
                DynamicValue::Map(map) => Ok(BoundContainer::Map(map)),
                _ => Err(EvaluationError::from_cast(*borrowed, "container")),
            },
            Self::Cell(cell) => Ok(BoundContainer::Bytes(cell)),
        }
    }

    pub fn as_regex(&self) -> Option<&regex::Regex> {
        match self {
            Self::Owned(DynamicValue::Regex(regex)) => Some(regex),
            Self::Borrowed(DynamicValue::Regex(regex)) => Some(regex),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&Vec<DynamicValue>> {
        match self {
            Self::Owned(DynamicValue::List(list)) => Some(list),
            Self::Borrowed(DynamicValue::List(list)) => Some(list),
            _ => None,
        }
    }

    pub fn eq_value(&self, value: &DynamicValue) -> bool {
        match self {
            Self::Owned(owned) => owned.eq(value),
            Self::Borrowed(borrowed) => (*borrowed).eq(value),
            Self::Cell(cell) => match value {
                DynamicValue::Bytes(other_cell) => cell == other_cell.as_ref(),
                _ => false,
            },
        }
    }
}

pub const BOUND_ARGUMENTS_CAPACITY: usize = 8;

pub struct BoundArguments<'a> {
    stack: ArrayVec<BoundArgument<'a>, BOUND_ARGUMENTS_CAPACITY>,
}

impl<'a> BoundArguments<'a> {
    pub fn new() -> Self {
        Self {
            stack: ArrayVec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn push(&mut self, arg: BoundArgument<'a>) {
        self.stack.push(arg);
    }

    pub fn get(&self, i: usize) -> Option<&BoundArgument> {
        self.stack.get(i)
    }

    // pub fn get_not_none(&self, i: usize) -> Option<&DynamicValue> {
    //     let value = self.stack.get(i)?;

    //     match value {
    //         DynamicValue::None => None,
    //         _ => Some(value),
    //     }
    // }

    pub fn get1(&self) -> &BoundArgument {
        &self.stack[0]
    }

    pub fn pop1(&mut self) -> BoundArgument {
        self.stack.pop().unwrap()
    }

    pub fn pop2(&mut self) -> (BoundArgument, BoundArgument) {
        let second = self.stack.pop().unwrap();
        let first = self.stack.pop().unwrap();

        (first, second)
    }

    pub fn pop3(&mut self) -> (BoundArgument, BoundArgument, BoundArgument) {
        let third = self.stack.pop().unwrap();
        let second = self.stack.pop().unwrap();
        let first = self.stack.pop().unwrap();

        (first, second, third)
    }

    pub fn get2(&self) -> (&BoundArgument, &BoundArgument) {
        (&self.stack[0], &self.stack[1])
    }

    pub fn get3(&self) -> (&BoundArgument, &BoundArgument, &BoundArgument) {
        (&self.stack[0], &self.stack[1], &self.stack[2])
    }

    pub fn get1_str(&self) -> Result<Cow<'_, str>, EvaluationError> {
        self.get1().try_as_str()
    }

    // pub fn pop1_bool(&mut self) -> bool {
    //     self.pop1().is_truthy()
    // }

    pub fn pop1_number(&mut self) -> Result<DynamicNumber, EvaluationError> {
        self.pop1().try_as_number()
    }

    // pub fn get2_str(&self) -> Result<(Cow<'_, str>, Cow<'_, str>), EvaluationError> {
    //     let (a, b) = self.get2();

    //     Ok((a.try_as_str()?, b.try_as_str()?))
    // }

    pub fn get2_number(&self) -> Result<(DynamicNumber, DynamicNumber), EvaluationError> {
        let (a, b) = self.get2();

        Ok((a.try_as_number()?, b.try_as_number()?))
    }
}

pub struct BoundArgumentsIntoIterator<'a>(
    arrayvec::IntoIter<BoundArgument<'a>, BOUND_ARGUMENTS_CAPACITY>,
);

// impl BoundArgumentsIntoIterator {
//     pub fn next_not_none(&mut self) -> Option<DynamicValue> {
//         self.next().and_then(|value| match value {
//             DynamicValue::None => None,
//             _ => Some(value),
//         })
//     }
// }

impl<'a> Iterator for BoundArgumentsIntoIterator<'a> {
    type Item = BoundArgument<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a> IntoIterator for BoundArguments<'a> {
    type Item = BoundArgument<'a>;
    type IntoIter = BoundArgumentsIntoIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        BoundArgumentsIntoIterator(self.stack.into_iter())
    }
}

const LAMBDA_ARGUMENTS_CAPACITY: usize = 4;

#[derive(Clone, Debug)]
pub struct LambdaArguments {
    stack: ArrayVec<(String, DynamicValue), LAMBDA_ARGUMENTS_CAPACITY>,
}

impl LambdaArguments {
    pub fn new() -> Self {
        Self {
            stack: ArrayVec::new(),
        }
    }

    pub fn get(&self, name: &str) -> &DynamicValue {
        self.stack
            .iter()
            .find_map(|(n, v)| if n == name { Some(v) } else { None })
            .expect("lambda variables cannot be out-of-bounds")
    }

    pub fn register(&mut self, name: &str) -> usize {
        for (i, (n, _)) in self.stack.iter().enumerate() {
            if n == name {
                return i;
            }
        }

        let i = self.stack.len();

        self.stack.push((name.to_string(), DynamicValue::None));
        i
    }

    pub fn set(&mut self, index: usize, value: DynamicValue) {
        self.stack[index].1 = value;
    }

    // pub fn upsert(&mut self, name: &str, value: DynamicValue) {
    //     for (n, v) in self.stack.iter_mut() {
    //         if n == name {
    //             *v = value;
    //             return;
    //         }
    //     }

    //     self.stack.push((name.to_string(), value));
    // }
}
