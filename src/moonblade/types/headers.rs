use std::collections::{btree_map::Entry, BTreeMap};

use crate::moonblade::parser::Expr;

use super::DynamicValue;

#[derive(Debug, PartialEq, Clone)]
pub enum ColumIndexationBy {
    Name(String),
    NameAndNth(String, isize),
    Pos(isize),
}

impl ColumIndexationBy {
    pub fn from_arguments(arguments: &[&Expr]) -> Option<Self> {
        if arguments.len() == 1 {
            let first_arg = arguments.first().unwrap();
            match first_arg {
                Expr::Str(column_name) => Some(Self::Name(column_name.clone())),
                Expr::Float(_) | Expr::Int(_) => first_arg.try_to_isize().map(Self::Pos),
                _ => None,
            }
        } else if arguments.len() == 2 {
            match arguments.first().unwrap() {
                Expr::Str(column_name) => {
                    let second_arg = arguments.get(1).unwrap();

                    second_arg
                        .try_to_isize()
                        .map(|column_index| Self::NameAndNth(column_name.to_string(), column_index))
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
            match pos_value.try_as_i64() {
                Err(_) => None,
                Ok(i) => match name_or_pos.try_as_str() {
                    Err(_) => None,
                    Ok(name) => Some(Self::NameAndNth(name.into_owned(), i as isize)),
                },
            }
        } else {
            match name_or_pos.try_as_i64() {
                Err(_) => match name_or_pos.try_as_str() {
                    Err(_) => None,
                    Ok(name) => Some(Self::Name(name.into_owned())),
                },
                Ok(i) => Some(Self::Pos(i as isize)),
            }
        }
    }

    pub fn find_column_index(&self, headers: &csv::ByteRecord) -> Option<usize> {
        let len = headers.len();

        match self {
            Self::Pos(i) => {
                if *i < 0 {
                    // Negative indexing
                    let i = i.unsigned_abs();

                    if i > len {
                        None
                    } else {
                        Some(len - i)
                    }
                } else {
                    let i = *i as usize;

                    if i >= len {
                        None
                    } else {
                        Some(i)
                    }
                }
            }
            Self::Name(name) => {
                let name_bytes = name.as_bytes();

                for (i, cell) in headers.iter().enumerate() {
                    if cell == name_bytes {
                        return Some(i);
                    }
                }

                None
            }
            Self::NameAndNth(name, pos) => {
                let name_bytes = name.as_bytes();

                if *pos < 0 {
                    let mut c = pos.unsigned_abs() - 1;

                    for (i, cell) in headers.iter().rev().enumerate() {
                        if cell == name_bytes {
                            if c == 0 {
                                return Some(len - i - 1);
                            }
                            c -= 1;
                        }
                    }
                } else {
                    let mut c = *pos as usize;

                    for (i, cell) in headers.iter().enumerate() {
                        if cell == name_bytes {
                            if c == 0 {
                                return Some(i);
                            }
                            c -= 1;
                        }
                    }
                }

                None
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct HeadersIndex {
    headers: Vec<Vec<u8>>,
    mapping: BTreeMap<String, Vec<usize>>,
}

impl HeadersIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.headers.len()
    }

    pub fn get_first_by_name(&self, name: &str) -> Option<usize> {
        self.mapping.get(name).map(|indices| indices[0])
    }

    pub fn from_headers<'a>(headers: impl IntoIterator<Item = &'a [u8]>) -> Self {
        let mut index = Self::new();

        for (i, header) in headers.into_iter().enumerate() {
            index.headers.push(header.to_vec());

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

    pub fn get_at(&self, index: usize) -> &[u8] {
        &self.headers[index]
    }

    pub fn get(&self, indexation: &ColumIndexationBy) -> Option<usize> {
        match indexation {
            ColumIndexationBy::Name(name) => self
                .mapping
                .get(name)
                .and_then(|positions| positions.first())
                .copied(),
            ColumIndexationBy::Pos(pos) => {
                if *pos < 0 {
                    // Negative indexing
                    let pos = pos.unsigned_abs();

                    if pos > self.mapping.len() {
                        None
                    } else {
                        Some(self.mapping.len() - pos)
                    }
                } else {
                    let pos = *pos as usize;

                    if pos >= self.mapping.len() {
                        None
                    } else {
                        Some(pos)
                    }
                }
            }
            ColumIndexationBy::NameAndNth(name, pos) => self
                .mapping
                .get(name)
                .and_then(|positions| {
                    if *pos < 0 {
                        let pos = pos.unsigned_abs();
                        let len = positions.len();

                        if pos > len {
                            None
                        } else {
                            positions.get(len - pos)
                        }
                    } else {
                        positions.get(*pos as usize)
                    }
                })
                .copied(),
        }
    }
}
