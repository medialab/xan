use std::borrow::ToOwned;
use std::cmp::Ordering;
use std::convert::TryFrom;
use std::fmt;
use std::iter::repeat;
use std::ops;
use std::str::FromStr;

use crate::record::Record;

#[derive(Clone, Deserialize)]
#[serde(try_from = "String")]
pub struct SelectColumns {
    selectors: Vec<Selector>,
    invert: bool,
}

impl SelectColumns {
    pub fn parse(mut s: &str) -> Result<Self, String> {
        let invert = if !s.is_empty() && s.as_bytes()[0] == b'!' {
            s = &s[1..];
            true
        } else {
            false
        };
        Ok(Self {
            selectors: SelectorParser::new(s).parse()?,
            invert,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.selectors.is_empty()
    }

    pub fn invert(&mut self) {
        self.invert = !self.invert;
    }

    pub fn selection<R: Record>(
        &self,
        first_record: &R,
        use_names: bool,
    ) -> Result<Selection, String> {
        if self.selectors.is_empty() {
            return Ok(Selection(if self.invert {
                // Inverting everything means we get nothing.
                vec![]
            } else {
                (0..first_record.len()).collect()
            }));
        }

        let mut map = vec![];
        for sel in &self.selectors {
            let idxs = sel.indices(first_record, use_names);
            map.extend(idxs?.into_iter());
        }
        if self.invert {
            let mut new_map = vec![];
            for i in 0..first_record.len() {
                if !map.contains(&i) {
                    new_map.push(i);
                }
            }
            return Ok(Selection(new_map));
        }
        Ok(Selection(map))
    }

    pub fn single_selection<R: Record>(
        &self,
        first_record: &R,
        use_names: bool,
    ) -> Result<usize, String> {
        let selection = self.selection(first_record, use_names)?;

        if selection.len() != 1 {
            return Err("target selection is not a single column".to_string());
        }

        Ok(selection[0])
    }

    pub fn retain_known<R: Record>(&mut self, headers: &R) -> Vec<usize> {
        let mut dropped: Vec<usize> = Vec::new();

        for (i, selector) in self.selectors.iter().enumerate() {
            match selector {
                Selector::One(sel) if sel.index(headers, true).is_err() => {
                    dropped.push(i);
                }
                Selector::Range(start, end)
                    if start.index(headers, true).is_err() && end.index(headers, true).is_err() =>
                {
                    dropped.push(i);
                }
                _ => continue,
            };
        }

        let mut i: usize = 0;

        self.selectors.retain(|_| {
            let drop = !dropped.contains(&i);

            i += 1;

            drop
        });

        dropped
    }
}

impl fmt::Debug for SelectColumns {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.selectors.is_empty() {
            write!(f, "<All>")
        } else {
            let strs: Vec<_> = self
                .selectors
                .iter()
                .map(|sel| format!("{:?}", sel))
                .collect();
            write!(f, "{}", strs.join(", "))
        }
    }
}

impl TryFrom<String> for SelectColumns {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl Default for SelectColumns {
    fn default() -> Self {
        Self::parse("").unwrap()
    }
}

struct SelectorParser {
    chars: Vec<char>,
    pos: usize,
}

impl SelectorParser {
    fn new(s: &str) -> SelectorParser {
        SelectorParser {
            chars: s.chars().collect(),
            pos: 0,
        }
    }

    fn parse(&mut self) -> Result<Vec<Selector>, String> {
        let mut sels = vec![];
        loop {
            if self.cur().is_none() {
                break;
            }

            if self.cur() == Some('*') {
                self.bump();

                if self.is_end_of_selector() {
                    sels.push(Selector::All);
                } else {
                    let suffix = self.parse_name()?;

                    if self.cur() == Some(':') {
                        return Err("Prefix name selection cannot work with range.".to_string());
                    }

                    sels.push(Selector::GlobSuffix(suffix));
                }

                self.bump();

                continue;
            }

            let f1: OneSelector = if self.cur() == Some(':') {
                OneSelector::Start
            } else {
                self.parse_one()?
            };

            if let OneSelector::IndexedName(name, None) = &f1 {
                if name.ends_with('*') {
                    sels.push(Selector::GlobPrefix(name[..name.len() - 1].to_string()));
                    self.bump();
                    continue;
                }
            }

            let f2: Option<OneSelector> = if self.cur() == Some(':') {
                self.bump();

                let sel = if self.is_end_of_selector() {
                    OneSelector::End
                } else {
                    self.parse_one()?
                };

                Some(sel)
            } else {
                None
            };

            if !self.is_end_of_selector() {
                return Err(format!(
                    "Expected end of field but got '{}' instead.",
                    self.cur().unwrap()
                ));
            }

            sels.push(match f2 {
                Some(end) => Selector::Range(f1, end),
                None => Selector::One(f1),
            });

            self.bump();
        }
        Ok(sels)
    }

    fn parse_one(&mut self) -> Result<OneSelector, String> {
        let name = if self.cur() == Some('"') {
            self.bump();
            self.parse_quoted_name()?
        } else {
            self.parse_name()?
        };
        Ok(if self.cur() == Some('[') {
            let idx = self.parse_index()?;
            OneSelector::IndexedName(name, Some(idx))
        } else {
            match FromStr::from_str(&name) {
                Err(_) => OneSelector::IndexedName(name, None),
                Ok(idx) => OneSelector::Index(idx),
            }
        })
    }

    fn parse_name(&mut self) -> Result<String, String> {
        let mut name = String::new();
        loop {
            if self.is_end_of_field() || self.cur() == Some('[') {
                break;
            }
            name.push(self.cur().unwrap());
            self.bump();
        }
        Ok(name)
    }

    fn parse_quoted_name(&mut self) -> Result<String, String> {
        let mut name = String::new();
        loop {
            match self.cur() {
                None => {
                    return Err("Unclosed quote, missing closing \".".to_owned());
                }
                Some('"') => {
                    self.bump();
                    if self.cur() == Some('"') {
                        self.bump();
                        name.push('"');
                        name.push('"');
                        continue;
                    }
                    break;
                }
                Some(c) => {
                    name.push(c);
                    self.bump();
                }
            }
        }
        Ok(name)
    }

    fn parse_index(&mut self) -> Result<isize, String> {
        assert_eq!(self.cur().unwrap(), '[');
        self.bump();

        let mut idx = String::new();
        loop {
            match self.cur() {
                None => {
                    return Err("Unclosed index bracket, missing closing ].".to_owned());
                }
                Some(']') => {
                    self.bump();
                    break;
                }
                Some(c) => {
                    idx.push(c);
                    self.bump();
                }
            }
        }
        FromStr::from_str(&idx)
            .map_err(|err| format!("Could not convert '{}' to an integer: {}", idx, err))
    }

    fn cur(&self) -> Option<char> {
        self.chars.get(self.pos).cloned()
    }

    fn is_end_of_field(&self) -> bool {
        self.cur().map_or(true, |c| c == ',' || c == ':')
    }

    fn is_end_of_selector(&self) -> bool {
        self.cur().map_or(true, |c| c == ',')
    }

    fn bump(&mut self) {
        if self.pos < self.chars.len() {
            self.pos += 1;
        }
    }
}

#[derive(Clone)]
enum Selector {
    One(OneSelector),
    Range(OneSelector, OneSelector),
    GlobPrefix(String),
    GlobSuffix(String),
    All,
}

#[derive(Clone)]
enum OneSelector {
    Start,
    End,
    Index(isize),
    IndexedName(String, Option<isize>),
}

impl Selector {
    fn indices<R: Record>(&self, first_record: &R, use_names: bool) -> Result<Vec<usize>, String> {
        match *self {
            Selector::All => Ok((0..first_record.len()).collect()),
            Selector::One(ref sel) => sel.index(first_record, use_names).map(|i| vec![i]),
            Selector::Range(ref sel1, ref sel2) => {
                let i1 = sel1.index(first_record, use_names)?;
                let i2 = sel2.index(first_record, use_names)?;
                Ok(match i1.cmp(&i2) {
                    Ordering::Equal => vec![i1],
                    Ordering::Less => (i1..(i2 + 1)).collect(),
                    Ordering::Greater => {
                        let mut inds = vec![];
                        let mut i = i1 + 1;
                        while i > i2 {
                            i -= 1;
                            inds.push(i);
                        }
                        inds
                    }
                })
            }
            Selector::GlobPrefix(ref prefix) => {
                if !use_names {
                    return Err(format!(
                        "Cannot use prefix ('{}') in selection \
                                        with --no-headers set.",
                        prefix
                    ));
                }

                let inds: Vec<usize> = first_record
                    .iter()
                    .enumerate()
                    .filter_map(|(i, h)| {
                        if h.starts_with(prefix.as_bytes()) {
                            Some(i)
                        } else {
                            None
                        }
                    })
                    .collect();

                if inds.is_empty() {
                    return Err(format!("Prefix '{}' selected nothing.", prefix));
                }

                Ok(inds)
            }
            Selector::GlobSuffix(ref suffix) => {
                if !use_names {
                    return Err(format!(
                        "Cannot use suffix ('{}') in selection \
                                        with --no-headers set.",
                        suffix
                    ));
                }

                let inds: Vec<usize> = first_record
                    .iter()
                    .enumerate()
                    .filter_map(|(i, h)| {
                        if h.ends_with(suffix.as_bytes()) {
                            Some(i)
                        } else {
                            None
                        }
                    })
                    .collect();

                if inds.is_empty() {
                    return Err(format!("Suffix '{}' selected nothing.", suffix));
                }

                Ok(inds)
            }
        }
    }
}

impl OneSelector {
    fn index<R: Record>(&self, first_record: &R, use_names: bool) -> Result<usize, String> {
        match *self {
            OneSelector::Start => Ok(0),
            OneSelector::End => Ok(if first_record.is_empty() {
                0
            } else {
                first_record.len() - 1
            }),
            OneSelector::Index(i) => {
                if i < 0 {
                    if i.unsigned_abs() > first_record.len() {
                        Err(format!(
                            "Selector index {} is out of \
                                 bounds. Index must be between -1 \
                                 and -{}.",
                            i,
                            first_record.len()
                        ))
                    } else {
                        Ok(first_record.len() - i.unsigned_abs())
                    }
                } else {
                    let i = i as usize;
                    if i >= first_record.len() {
                        Err(format!(
                            "Selector index {} is out of \
                                 bounds. Index must be between 0 \
                                 and {}.",
                            i,
                            first_record.len()
                        ))
                    } else {
                        Ok(i)
                    }
                }
            }
            OneSelector::IndexedName(ref s, sidx) => {
                let sidx = sidx.unwrap_or(0);

                if !use_names {
                    return Err(format!(
                        "Cannot use names ('{}') in selection \
                                        with --no-headers set.",
                        s
                    ));
                }
                let mut num_found = 0;

                if sidx < 0 {
                    for (i, field) in first_record.iter().enumerate().rev() {
                        if field == s.as_bytes() {
                            if num_found == sidx.abs() - 1 {
                                return Ok(i);
                            }
                            num_found += 1;
                        }
                    }
                } else {
                    for (i, field) in first_record.iter().enumerate() {
                        if field == s.as_bytes() {
                            if num_found == sidx {
                                return Ok(i);
                            }
                            num_found += 1;
                        }
                    }
                }

                if num_found == 0 {
                    Err(format!(
                        "Selector name '{}' does not exist \
                                 as a named header in the given CSV \
                                 data.",
                        s
                    ))
                } else if sidx < 0 {
                    Err(format!(
                        "Selector index '{}' for name '{}' is \
                                     out of bounds. Must be between -{} and -1.",
                        sidx, s, num_found
                    ))
                } else {
                    Err(format!(
                        "Selector index '{}' for name '{}' is \
                                 out of bounds. Must be between 0 and {}.",
                        sidx,
                        s,
                        num_found - 1
                    ))
                }
            }
        }
    }
}

impl fmt::Debug for Selector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Selector::All => write!(f, "All"),
            Selector::One(ref sel) => sel.fmt(f),
            Selector::Range(ref s, ref e) => write!(f, "Range({:?}, {:?})", s, e),
            Selector::GlobPrefix(ref prefix) => write!(f, "Prefix({:?})", prefix),
            Selector::GlobSuffix(ref suffix) => write!(f, "Suffix({:?})", suffix),
        }
    }
}

impl fmt::Debug for OneSelector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            OneSelector::Start => write!(f, "Start"),
            OneSelector::End => write!(f, "End"),
            OneSelector::Index(idx) => write!(f, "Index({})", idx),
            OneSelector::IndexedName(ref s, idx) => match idx {
                None => write!(f, "IndexedName({})", s),
                Some(i) => write!(f, "IndexedName({}[{}])", s, i),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct Selection(Vec<usize>);

impl Selection {
    pub fn empty() -> Self {
        Self(vec![])
    }

    pub fn full(len: usize) -> Self {
        Self((0..len).collect())
    }

    pub fn into_first(self) -> Option<usize> {
        self.0.first().copied()
    }

    pub fn into_rest(self) -> Self {
        Self(self.0.into_iter().skip(1).collect())
    }

    pub fn without_indices(len: usize, indices: &[usize]) -> Self {
        let mut sel = Self::full(len);

        sel.0.retain(|i| !indices.contains(i));

        sel
    }

    pub fn offset_by(&mut self, by: usize) {
        for i in self.0.iter_mut() {
            *i += by;
        }
    }

    pub fn insert(&mut self, index: usize, element: usize) {
        self.0.insert(index, element);
    }

    pub fn select<'a, 'b, T: 'b + ?Sized>(
        &'a self,
        row: &'b impl ops::Index<usize, Output = T>,
    ) -> impl Iterator<Item = &'b T>
    where
        'a: 'b,
    {
        self.iter().map(|i| &row[*i])
    }

    pub fn collect<'a, 'b, T, O>(&'a self, row: &'b impl ops::Index<usize, Output = T>) -> Vec<O>
    where
        'a: 'b,
        T: 'b + ?Sized + ToOwned<Owned = O>,
    {
        self.select(row).map(|f| f.to_owned()).collect()
    }

    pub fn dedup(&mut self) {
        let mut new = Vec::new();

        for i in self.0.iter().copied() {
            if !new.contains(&i) {
                new.push(i);
            }
        }

        self.0 = new;
    }

    pub fn sort_and_dedup(&mut self) {
        self.0.sort();
        self.0.dedup();
    }

    pub fn has_duplicates(&self) -> bool {
        let mut deduplicated = self.clone();
        deduplicated.sort_and_dedup();

        deduplicated.len() != self.len()
    }

    pub fn indexed_mask(&self, alignment: usize) -> Vec<Option<usize>> {
        let mut m = repeat(None).take(alignment).collect::<Vec<Option<usize>>>();

        for (j, i) in self.iter().enumerate() {
            if *i < alignment {
                m[*i] = Some(j)
            }
        }

        m
    }

    pub fn mask(&self, alignment: usize) -> Vec<bool> {
        self.indexed_mask(alignment)
            .into_iter()
            .map(|o| o.is_some())
            .collect()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn contains(&self, i: usize) -> bool {
        self.0.iter().any(|j| i == *j)
    }

    pub fn subtract(&mut self, other: &Self) {
        self.0.retain(|i| !other.contains(*i));
    }

    pub fn inverse(&self, alignment: usize) -> Self {
        let mask = self.mask(alignment);

        let mut indices = Vec::new();

        for (i, is_selected) in mask.into_iter().enumerate() {
            if !is_selected {
                indices.push(i);
            }
        }

        Self(indices)
    }
}

impl ops::Deref for Selection {
    type Target = [usize];

    fn deref(&self) -> &[usize] {
        &self.0
    }
}
