use std::borrow::Cow;

use arrayvec::ArrayVec;

use crate::moonblade::error::EvaluationError;

use super::{DynamicNumber, DynamicValue};

pub struct BoundArguments {
    stack: ArrayVec<DynamicValue, BOUND_ARGUMENTS_CAPACITY>,
}

impl BoundArguments {
    pub fn new() -> Self {
        Self {
            stack: ArrayVec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn push(&mut self, arg: DynamicValue) {
        self.stack.push(arg);
    }

    pub fn get(&self, i: usize) -> Option<&DynamicValue> {
        self.stack.get(i)
    }

    pub fn get_not_none(&self, i: usize) -> Option<&DynamicValue> {
        let value = self.stack.get(i)?;

        match value {
            DynamicValue::None => None,
            _ => Some(value),
        }
    }

    pub fn get1(&self) -> &DynamicValue {
        &self.stack[0]
    }

    pub fn pop1(&mut self) -> DynamicValue {
        self.stack.pop().unwrap()
    }

    pub fn pop2(&mut self) -> (DynamicValue, DynamicValue) {
        let second = self.stack.pop().unwrap();
        let first = self.stack.pop().unwrap();

        (first, second)
    }

    pub fn pop3(&mut self) -> (DynamicValue, DynamicValue, DynamicValue) {
        let third = self.stack.pop().unwrap();
        let second = self.stack.pop().unwrap();
        let first = self.stack.pop().unwrap();

        (first, second, third)
    }

    pub fn get2(&self) -> (&DynamicValue, &DynamicValue) {
        (&self.stack[0], &self.stack[1])
    }

    pub fn get3(&self) -> (&DynamicValue, &DynamicValue, &DynamicValue) {
        (&self.stack[0], &self.stack[1], &self.stack[2])
    }

    pub fn get1_str(&self) -> Result<Cow<str>, EvaluationError> {
        self.get1().try_as_str()
    }

    pub fn pop1_bool(&mut self) -> bool {
        self.pop1().is_truthy()
    }

    pub fn pop1_number(&mut self) -> Result<DynamicNumber, EvaluationError> {
        self.pop1().try_as_number()
    }

    pub fn get2_str(&self) -> Result<(Cow<str>, Cow<str>), EvaluationError> {
        let (a, b) = self.get2();

        Ok((a.try_as_str()?, b.try_as_str()?))
    }

    pub fn get2_number(&self) -> Result<(DynamicNumber, DynamicNumber), EvaluationError> {
        let (a, b) = self.get2();

        Ok((a.try_as_number()?, b.try_as_number()?))
    }
}

pub struct BoundArgumentsIntoIterator(arrayvec::IntoIter<DynamicValue, BOUND_ARGUMENTS_CAPACITY>);

impl BoundArgumentsIntoIterator {
    pub fn next_not_none(&mut self) -> Option<DynamicValue> {
        self.next().and_then(|value| match value {
            DynamicValue::None => None,
            _ => Some(value),
        })
    }
}

impl Iterator for BoundArgumentsIntoIterator {
    type Item = DynamicValue;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl IntoIterator for BoundArguments {
    type Item = DynamicValue;
    type IntoIter = BoundArgumentsIntoIterator;

    fn into_iter(self) -> Self::IntoIter {
        BoundArgumentsIntoIterator(self.stack.into_iter())
    }
}

pub const BOUND_ARGUMENTS_CAPACITY: usize = 8;
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
