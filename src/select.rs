use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;
use std::iter::repeat;
use std::ops;
use std::str::FromStr;

use serde::de::{Deserialize, Deserializer, Error};

#[derive(Clone)]
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

    pub fn selection(
        &self,
        first_record: &csv::ByteRecord,
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
            let set: HashSet<_> = map.into_iter().collect();
            let mut map = vec![];
            for i in 0..first_record.len() {
                if !set.contains(&i) {
                    map.push(i);
                }
            }
            return Ok(Selection(map));
        }
        Ok(Selection(map))
    }

    pub fn single_selection(
        &self,
        first_record: &csv::ByteRecord,
        use_names: bool,
    ) -> Result<usize, String> {
        let selection = self.selection(first_record, use_names)?;

        if selection.len() != 1 {
            return Err("target selection is not a single column".to_string());
        }

        Ok(selection[0])
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

impl<'de> Deserialize<'de> for SelectColumns {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(D::Error::custom)
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
                sels.push(Selector::All);

                self.bump();
                if self.is_end_of_selector() {
                    self.bump();
                }

                continue;
            }

            let f1: OneSelector = if self.cur() == Some('-') {
                OneSelector::Start
            } else {
                self.parse_one()?
            };

            let f2: Option<OneSelector> = if self.cur() == Some('-') {
                self.bump();
                Some(if self.is_end_of_selector() {
                    OneSelector::End
                } else {
                    self.parse_one()?
                })
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
            OneSelector::IndexedName(name, idx)
        } else {
            match FromStr::from_str(&name) {
                Err(_) => OneSelector::IndexedName(name, 0),
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

    fn parse_index(&mut self) -> Result<usize, String> {
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
        self.cur().map_or(true, |c| c == ',' || c == '-')
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
    All,
}

#[derive(Clone)]
enum OneSelector {
    Start,
    End,
    Index(usize),
    IndexedName(String, usize),
}

impl Selector {
    fn indices(
        &self,
        first_record: &csv::ByteRecord,
        use_names: bool,
    ) -> Result<Vec<usize>, String> {
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
        }
    }
}

impl OneSelector {
    fn index(&self, first_record: &csv::ByteRecord, use_names: bool) -> Result<usize, String> {
        match *self {
            OneSelector::Start => Ok(0),
            OneSelector::End => Ok(if first_record.is_empty() {
                0
            } else {
                first_record.len() - 1
            }),
            OneSelector::Index(i) => {
                if i >= first_record.len() {
                    Err(format!(
                        "Selector index {} is out of \
                                 bounds. Index must be >= 0 \
                                 and <= {}.",
                        i,
                        first_record.len()
                    ))
                } else {
                    Ok(i)
                }
            }
            OneSelector::IndexedName(ref s, sidx) => {
                if !use_names {
                    return Err(format!(
                        "Cannot use names ('{}') in selection \
                                        with --no-headers set.",
                        s
                    ));
                }
                let mut num_found = 0;
                for (i, field) in first_record.iter().enumerate() {
                    if field == s.as_bytes() {
                        if num_found == sidx {
                            return Ok(i);
                        }
                        num_found += 1;
                    }
                }
                if num_found == 0 {
                    Err(format!(
                        "Selector name '{}' does not exist \
                                 as a named header in the given CSV \
                                 data.",
                        s
                    ))
                } else {
                    Err(format!(
                        "Selector index '{}' for name '{}' is \
                                 out of bounds. Must be >= 0 and <= {}.",
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
        }
    }
}

impl fmt::Debug for OneSelector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            OneSelector::Start => write!(f, "Start"),
            OneSelector::End => write!(f, "End"),
            OneSelector::Index(idx) => write!(f, "Index({})", idx),
            OneSelector::IndexedName(ref s, idx) => write!(f, "IndexedName({}[{}])", s, idx),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Selection(Vec<usize>);

impl Selection {
    pub fn full(len: usize) -> Self {
        Self((0..len).collect())
    }

    pub fn offset_by(&mut self, by: usize) {
        for i in self.0.iter_mut() {
            *i += by;
        }
    }

    pub fn select<'a, 'b>(&'a self, row: &'b csv::ByteRecord) -> impl Iterator<Item = &'b [u8]>
    where
        'a: 'b,
    {
        self.iter().map(move |i| &row[*i])
    }

    pub fn select_string_record<'a, 'b>(
        &'a self,
        row: &'b csv::StringRecord,
    ) -> impl Iterator<Item = &'b str>
    where
        'a: 'b,
    {
        self.iter().map(move |i| &row[*i])
    }

    pub fn collect(&self, row: &csv::ByteRecord) -> Vec<Vec<u8>> {
        self.select(row).map(|f| f.to_vec()).collect()
    }

    pub fn has_duplicates(&self) -> bool {
        let mut indices = self.0.clone();
        indices.sort();
        indices.dedup();

        indices.len() != self.len()
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

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().copied()
    }

    pub fn contains(&self, i: usize) -> bool {
        self.0.iter().any(|j| i == *j)
    }

    pub fn subtract(&mut self, other: &Self) {
        self.0.retain(|i| !other.contains(*i));
    }
}

impl ops::Deref for Selection {
    type Target = [usize];

    fn deref(&self) -> &[usize] {
        &self.0
    }
}
