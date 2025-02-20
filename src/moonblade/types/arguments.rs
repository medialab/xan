use std::ops::RangeInclusive;

use crate::moonblade::error::{ConcretizationError, InvalidArity};

#[derive(Debug, Clone, PartialEq)]
pub enum Argument {
    Positional,
    Optional,
    Named(String),
}

impl Argument {
    pub fn with_name(name: &str) -> Self {
        Self::Named(name.to_string())
    }

    pub fn is_optional(&self) -> bool {
        matches!(self, Self::Named(_) | Self::Optional)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionArguments {
    variadic: bool,
    arguments: Vec<Argument>,
}

impl FunctionArguments {
    pub fn nullary() -> Self {
        Self {
            variadic: false,
            arguments: Vec::new(),
        }
    }

    pub fn unary() -> Self {
        Self {
            variadic: false,
            arguments: vec![Argument::Positional],
        }
    }

    pub fn binary() -> Self {
        Self {
            variadic: false,
            arguments: vec![Argument::Positional; 2],
        }
    }

    pub fn nary(n: usize) -> Self {
        Self {
            variadic: false,
            arguments: vec![Argument::Positional; n],
        }
    }

    pub fn variadic(n: usize) -> Self {
        Self {
            variadic: true,
            arguments: vec![Argument::Positional; n],
        }
    }

    pub fn with_range(range: RangeInclusive<usize>) -> Self {
        let mut args = Vec::new();

        for _ in 0..*range.start() {
            args.push(Argument::Positional);
        }

        while args.len() < *range.end() {
            args.push(Argument::Optional);
        }

        Self {
            variadic: false,
            arguments: args,
        }
    }

    pub fn complex(arguments: Vec<Argument>) -> Self {
        Self {
            variadic: false,
            arguments,
        }
    }

    fn has_named(&self) -> bool {
        self.arguments
            .iter()
            .any(|arg| matches!(arg, Argument::Named(_)))
    }

    pub fn arity(&self) -> Arity {
        if self.variadic {
            Arity::Min(self.arguments.len())
        } else {
            let min = self
                .arguments
                .iter()
                .filter(|arg| !arg.is_optional())
                .count();

            if min == self.arguments.len() {
                Arity::Strict(min)
            } else {
                Arity::Range(min..=self.arguments.len())
            }
        }
    }

    pub fn validate_arity(&self, got: usize) -> Result<(), InvalidArity> {
        self.arity().validate(got)
    }

    fn find_named_arg_index(&self, name: &str) -> Option<usize> {
        self.arguments
            .iter()
            .position(|arg| matches!(arg, Argument::Named(n) if n == name))
    }

    // NOTE: arity will be validated beforehand
    pub fn reorder<T>(
        &self,
        actual_args: Vec<(Option<String>, T)>,
    ) -> Result<Vec<Option<T>>, ConcretizationError> {
        if self.variadic || !self.has_named() {
            return Ok(actual_args
                .into_iter()
                .map(|(_, value)| Some(value))
                .collect());
        }

        let mut reordered: Vec<Option<T>> = (0..self.arguments.len()).map(|_| None).collect();
        let mut i = 0;

        for (name_opt, value) in actual_args.into_iter() {
            match name_opt {
                Some(name) => {
                    match self.find_named_arg_index(&name) {
                        None => return Err(ConcretizationError::UnknownArgumentName(name)),
                        Some(j) => {
                            reordered[j] = Some(value);
                        }
                    };
                }
                None => {
                    reordered[i] = Some(value);
                    i += 1;
                }
            }
        }

        Ok(reordered)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Arity {
    Strict(usize),
    Min(usize),
    Range(RangeInclusive<usize>),
}

impl Arity {
    pub fn validate(self, got: usize) -> Result<(), InvalidArity> {
        match &self {
            Self::Strict(expected) => {
                if *expected != got {
                    Err(InvalidArity::from_arity(self, got))
                } else {
                    Ok(())
                }
            }
            Self::Min(expected_min) => {
                if got < *expected_min {
                    Err(InvalidArity::from_arity(self, got))
                } else {
                    Ok(())
                }
            }
            Self::Range(range) => {
                if !range.contains(&got) {
                    Err(InvalidArity::from_arity(self, got))
                } else {
                    Ok(())
                }
            }
        }
    }
}
