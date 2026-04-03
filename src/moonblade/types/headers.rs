use std::collections::{btree_map::Entry, BTreeMap};
use std::ops::Index;

use crate::moonblade::parser::Expr;

use super::DynamicValue;

#[derive(Debug, PartialEq, Clone)]
pub enum ColumIndexationBy {
    Name(Vec<u8>),
    NameAndNth(Vec<u8>, isize),
    Pos(isize),
}

impl ColumIndexationBy {
    pub fn from_arguments(arguments: &[&Expr]) -> Option<Self> {
        if arguments.len() == 1 {
            let first_arg = arguments.first().unwrap();
            match first_arg {
                Expr::Str(column_name) => Some(Self::Name(column_name.as_bytes().to_vec())),
                Expr::Float(_) | Expr::Int(_) => first_arg.try_to_isize().map(Self::Pos),
                _ => None,
            }
        } else if arguments.len() == 2 {
            match arguments.first().unwrap() {
                Expr::Str(column_name) => {
                    let second_arg = arguments.get(1).unwrap();

                    second_arg.try_to_isize().map(|column_index| {
                        Self::NameAndNth(column_name.as_bytes().to_vec(), column_index)
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
            match pos_value.try_as_i64() {
                Err(_) => None,
                Ok(i) => match name_or_pos.try_as_str() {
                    Err(_) => None,
                    Ok(name) => Some(Self::NameAndNth(name.into_owned().into_bytes(), i as isize)),
                },
            }
        } else {
            match name_or_pos.try_as_i64() {
                Err(_) => match name_or_pos.try_as_str() {
                    Err(_) => None,
                    Ok(name) => Some(Self::Name(name.into_owned().into_bytes())),
                },
                Ok(i) => Some(Self::Pos(i as isize)),
            }
        }
    }

    pub fn has_name(&self) -> bool {
        matches!(self, Self::Name(_) | Self::NameAndNth(_, _))
    }
}

// NOTE: `mapping` could be a sorted (key, index) list we access using binary search
#[derive(Debug, Clone, Default)]
pub struct HeadersIndex {
    headers: Vec<Vec<u8>>,
    mapping: BTreeMap<Vec<u8>, Vec<usize>>,
    headless: bool,
}

impl HeadersIndex {
    pub fn empty(headless: bool) -> Self {
        Self {
            headless,
            ..Default::default()
        }
    }

    pub fn is_headless(&self) -> bool {
        self.headless
    }

    pub fn new<'a>(headers: impl IntoIterator<Item = &'a [u8]>, headless: bool) -> Self {
        let mut index = Self::empty(headless);

        for (i, header) in headers.into_iter().enumerate() {
            index.headers.push(header.to_vec());

            if headless {
                continue;
            }

            match index.mapping.entry(header.to_vec()) {
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

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.headers.len()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.headers.is_empty()
    }

    pub fn first_by_name(&self, name: impl AsRef<[u8]>) -> Option<usize> {
        self.mapping.get(name.as_ref()).map(|indices| indices[0])
    }

    pub fn get(&self, indexation: &ColumIndexationBy) -> Option<usize> {
        match indexation {
            ColumIndexationBy::Name(name) => self
                .mapping
                .get(name)
                .and_then(|positions| positions.first())
                .copied(),
            ColumIndexationBy::Pos(pos) => {
                let len = self.headers.len();

                if *pos < 0 {
                    // Negative indexing
                    let pos = pos.unsigned_abs();

                    if pos > len {
                        None
                    } else {
                        Some(len - pos)
                    }
                } else {
                    let pos = *pos as usize;

                    if pos >= len {
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

impl Index<usize> for HeadersIndex {
    type Output = [u8];

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        &self.headers[index]
    }
}
