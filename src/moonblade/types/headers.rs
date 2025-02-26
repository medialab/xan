use std::collections::{btree_map::Entry, BTreeMap};

use crate::moonblade::parser::Expr;

use super::DynamicValue;

#[derive(Debug, PartialEq, Clone)]
pub enum ColumIndexationBy {
    Name(String),
    NameAndNth((String, usize)),
    Pos(usize),
    ReversePos(usize),
}

impl ColumIndexationBy {
    pub fn from_arguments(arguments: &[&Expr]) -> Option<Self> {
        if arguments.len() == 1 {
            let first_arg = arguments.first().unwrap();
            match first_arg {
                Expr::Str(column_name) => Some(Self::Name(column_name.clone())),
                Expr::Float(_) | Expr::Int(_) => first_arg.try_to_isize().map(|i| {
                    if i >= 0 {
                        Self::Pos(i as usize)
                    } else {
                        Self::ReversePos(i.unsigned_abs())
                    }
                }),
                _ => None,
            }
        } else if arguments.len() == 2 {
            match arguments.first().unwrap() {
                Expr::Str(column_name) => {
                    let second_arg = arguments.get(1).unwrap();

                    second_arg.try_to_usize().map(|column_index| {
                        Self::NameAndNth((column_name.to_string(), column_index))
                    })
                }
                _ => None,
            }
        } else {
            None
        }
    }

    pub fn from_argument(argument: &Expr) -> Option<Self> {
        Self::from_arguments(&[argument])
    }

    pub fn from_bound_arguments(
        name_or_pos: DynamicValue,
        pos: Option<DynamicValue>,
    ) -> Option<Self> {
        if let Some(pos_value) = pos {
            match pos_value.try_as_usize() {
                Err(_) => None,
                Ok(i) => match name_or_pos.try_as_str() {
                    Err(_) => None,
                    Ok(name) => Some(Self::NameAndNth((name.into_owned(), i))),
                },
            }
        } else {
            match name_or_pos.try_as_i64() {
                Err(_) => match name_or_pos.try_as_str() {
                    Err(_) => None,
                    Ok(name) => Some(Self::Name(name.into_owned())),
                },
                Ok(i) => Some(if i < 0 {
                    Self::ReversePos(i.unsigned_abs() as usize)
                } else {
                    Self::Pos(i as usize)
                }),
            }
        }
    }

    pub fn find_column_index<'a>(
        &self,
        headers: impl IntoIterator<Item = &'a [u8]>,
        len: usize,
    ) -> Option<usize> {
        match self {
            Self::Pos(i) => {
                if i >= &len {
                    None
                } else {
                    Some(*i)
                }
            }
            Self::ReversePos(i) => {
                if *i > len {
                    None
                } else {
                    Some(len - i)
                }
            }
            Self::Name(name) => {
                let name_bytes = name.as_bytes();

                for (i, cell) in headers.into_iter().enumerate() {
                    if cell == name_bytes {
                        return Some(i);
                    }
                }

                None
            }
            Self::NameAndNth((name, pos)) => {
                let mut c = *pos;

                let name_bytes = name.as_bytes();

                for (i, cell) in headers.into_iter().enumerate() {
                    if cell == name_bytes {
                        if c == 0 {
                            return Some(i);
                        }
                        c -= 1;
                    }
                }

                None
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct HeadersIndex {
    mapping: BTreeMap<String, Vec<usize>>,
}

impl HeadersIndex {
    pub fn new() -> Self {
        HeadersIndex {
            mapping: BTreeMap::new(),
        }
    }

    pub fn from_headers<'a>(headers: impl IntoIterator<Item = &'a [u8]>) -> Self {
        let mut index = Self::new();

        for (i, header) in headers.into_iter().enumerate() {
            let key = std::str::from_utf8(header).unwrap().to_string();

            match index.mapping.entry(key) {
                Entry::Vacant(entry) => {
                    let positions: Vec<usize> = vec![i];
                    entry.insert(positions);
                }
                Entry::Occupied(mut entry) => {
                    entry.get_mut().push(i);
                }
            }
        }

        index
    }

    pub fn get(&self, indexation: &ColumIndexationBy) -> Option<usize> {
        match indexation {
            ColumIndexationBy::Name(name) => self
                .mapping
                .get(name)
                .and_then(|positions| positions.first())
                .copied(),
            ColumIndexationBy::Pos(pos) => {
                if *pos >= self.mapping.len() {
                    None
                } else {
                    Some(*pos)
                }
            }
            ColumIndexationBy::ReversePos(pos) => {
                if *pos > self.mapping.len() {
                    None
                } else {
                    Some(self.mapping.len() - pos)
                }
            }
            ColumIndexationBy::NameAndNth((name, pos)) => self
                .mapping
                .get(name)
                .and_then(|positions| positions.get(*pos))
                .copied(),
        }
    }
}
