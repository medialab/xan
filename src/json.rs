use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{btree_map::Entry as BTreeMapEntry, BTreeMap};
use std::io::{Read, Write};
use std::num::NonZeroUsize;
use std::rc::Rc;

use csv::StringRecord;
use serde::ser::{Serialize, SerializeMap, Serializer};
use serde_json::{json, Value};
use simd_csv::ByteRecord;

use crate::select::Selection;

#[derive(Default)]
pub struct AttributeNameInterner {
    strings: Vec<Rc<String>>,
    map: BTreeMap<Rc<String>, usize>,
}

impl AttributeNameInterner {
    pub fn register(&mut self, name: String) -> usize {
        use BTreeMapEntry::*;

        let name = Rc::new(name);
        let next_id = self.strings.len();

        match self.map.entry(name.clone()) {
            Occupied(entry) => *entry.get(),
            Vacant(entry) => {
                self.strings.push(name.clone());
                entry.insert(next_id);
                next_id
            }
        }
    }

    fn get(&self, id: usize) -> &str {
        &self.strings[id]
    }
}

thread_local! {
    pub static INTERNER: RefCell<AttributeNameInterner> = RefCell::new(AttributeNameInterner::default());
}

#[derive(Debug, Default)]
pub struct Attributes {
    entries: Vec<(usize, Value)>,
}

impl Attributes {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
        }
    }

    pub fn insert(&mut self, key: &str, value: Value) {
        let attr_name_id = INTERNER.with_borrow_mut(|interner| interner.register(key.to_string()));
        self.entries.push((attr_name_id, value));
    }

    pub fn iter(&self) -> impl Iterator<Item = &(usize, Value)> {
        self.entries.iter()
    }

    pub fn get(&self, id: usize) -> Option<&Value> {
        self.entries
            .iter()
            .find_map(|entry| if entry.0 == id { Some(&entry.1) } else { None })
    }
}

impl Serialize for Attributes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.entries.len()))?;

        for (k, v) in self.entries.iter() {
            INTERNER.with_borrow(|interner| map.serialize_entry(interner.get(*k), v))?;
        }

        map.end()
    }
}

#[derive(Debug, Default, Clone)]
pub struct OmittableAttributes {
    entries: Vec<(usize, Option<Value>)>,
}

impl OmittableAttributes {
    pub fn from_headers<'a>(headers: impl Iterator<Item = &'a str>) -> Self {
        let mut entries = Vec::new();

        for h in headers {
            INTERNER
                .with_borrow_mut(|interner| entries.push((interner.register(h.to_string()), None)));
        }

        Self { entries }
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut Option<Value>> {
        self.entries.iter_mut().map(|(_, v)| v)
    }
}

impl Serialize for OmittableAttributes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.entries.len()))?;

        for (k, opt) in self.entries.iter() {
            if let Some(v) = opt {
                INTERNER.with_borrow(|interner| map.serialize_entry(interner.get(*k), v))?;
            }
        }

        map.end()
    }
}

// NOTE: we keep a depth on `Delve` and `Pop` to be able to skip absent keys efficiently
#[derive(Debug, Clone)]
enum JSONTraversalState {
    Delve(String, usize),
    Emit,
    Pop(usize),
}

type JSONTraversalStack = Vec<JSONTraversalState>;

fn traverse_to_build_stack(
    value: &Value,
    stack: &mut JSONTraversalStack,
    reorder_keys: bool,
    depth: usize,
) {
    match value {
        Value::Object(map) => {
            let mut items = map.iter().collect::<Vec<_>>();

            // NOTE: we put scalar values first, then nested ones and we also sort by key
            if reorder_keys {
                items.sort_by_key(|i| {
                    (
                        if matches!(i.1, Value::Object(_)) {
                            1
                        } else {
                            0
                        },
                        i.0,
                    )
                });
            }

            for (k, v) in items {
                stack.push(JSONTraversalState::Delve(k.to_string(), depth));

                traverse_to_build_stack(v, stack, reorder_keys, depth + 1);

                stack.push(JSONTraversalState::Pop(depth));
            }
        }
        _ => stack.push(JSONTraversalState::Emit),
    };
}

fn headers_from_stack(stack: &JSONTraversalStack) -> ByteRecord {
    let mut record = ByteRecord::new();

    // Single scalar early return
    if stack.len() == 1 {
        record.push_field(b"value");
        return record;
    }

    let mut path: Vec<&str> = Vec::new();

    for state in stack {
        match state {
            JSONTraversalState::Delve(key, _) => {
                path.push(key.as_str());
            }
            JSONTraversalState::Emit => {
                record.push_field(path.join(".").as_bytes());
            }
            JSONTraversalState::Pop(_) => {
                path.pop();
            }
        }
    }

    record
}

fn traverse_with_stack<F>(value: &Value, stack: &JSONTraversalStack, mut callback: F)
where
    F: FnMut(&Value),
{
    let mut working_stack: Vec<&Value> = vec![];
    let mut current_value: &Value = value;

    let mut i: usize = 0;

    'outer: while i < stack.len() {
        let state = &stack[i];

        match state {
            JSONTraversalState::Delve(key, depth) => {
                working_stack.push(current_value);

                match current_value.as_object().and_then(|o| o.get(key)) {
                    None => {
                        i += 1;

                        while i < stack.len() {
                            let next_state = &stack[i];

                            match next_state {
                                JSONTraversalState::Emit => callback(&Value::Null),
                                JSONTraversalState::Pop(target_depth) if target_depth == depth => {
                                    continue 'outer;
                                }
                                _ => (),
                            };

                            i += 1;
                        }
                    }
                    Some(next_value) => {
                        current_value = next_value;
                    }
                }
            }
            JSONTraversalState::Pop(_) => {
                current_value = working_stack.pop().expect("cannot pop");
            }
            JSONTraversalState::Emit => {
                callback(current_value);
            }
        }

        i += 1;
    }
}

#[inline]
fn serialize_json_value_to_csv_field(value: &Value) -> Cow<[u8]> {
    match value {
        Value::Null => Cow::Borrowed(b""),
        Value::Bool(b) => Cow::Borrowed(if *b { b"true" } else { b"false" }),
        Value::String(s) => Cow::Borrowed(s.as_bytes()),
        Value::Number(n) => Cow::Owned(n.to_string().into_bytes()),
        Value::Array(l) => Cow::Owned(serde_json::to_vec(l).unwrap()),
        Value::Object(o) => Cow::Owned(serde_json::to_vec(o).unwrap()),
    }
}

#[inline]
fn fill_record(value: &Value, record: &mut ByteRecord, stack: &JSONTraversalStack) {
    record.clear();

    traverse_with_stack(value, stack, |v| {
        record.push_field(&serialize_json_value_to_csv_field(v));
    });
}

fn merge(a: &mut Value, b: &Value) {
    if let Value::Object(a) = a {
        if let Value::Object(b) = b {
            for (k, v) in b {
                merge(a.entry(k).or_insert(Value::Null), v);
            }

            return;
        }
    }

    *a = b.clone();
}

pub struct JSONTabularizer<W: Write> {
    writer: simd_csv::Writer<W>,
    harmonized_value: Value,
    sample_size: Option<usize>,
    sample: Vec<Value>,
    flushed: bool,
    output_record: ByteRecord,
    stack: JSONTraversalStack,
    reorder_keys: bool,
}

impl<W: Write> JSONTabularizer<W> {
    pub fn from_writer(writer: simd_csv::Writer<W>, sample_size: Option<NonZeroUsize>) -> Self {
        Self {
            writer,
            harmonized_value: Value::Null,
            sample_size: sample_size.map(|limit| limit.get()),
            sample: match sample_size {
                Some(limit) => Vec::with_capacity(limit.get()),
                None => Vec::new(),
            },
            flushed: false,
            output_record: ByteRecord::new(),
            stack: JSONTraversalStack::new(),
            reorder_keys: false,
        }
    }

    pub fn reorder_keys(&mut self) {
        self.reorder_keys = true;
    }

    pub fn flush(&mut self) -> simd_csv::Result<()> {
        if self.flushed {
            return Ok(());
        }

        traverse_to_build_stack(
            &self.harmonized_value,
            &mut self.stack,
            self.reorder_keys,
            0,
        );
        self.writer
            .write_byte_record(&headers_from_stack(&self.stack))?;

        for value in self.sample.iter() {
            fill_record(value, &mut self.output_record, &self.stack);
            self.writer.write_byte_record(&self.output_record)?;
        }

        self.writer.flush()?;

        self.flushed = true;

        Ok(())
    }

    pub fn process(&mut self, value: Value) -> simd_csv::Result<()> {
        // Sampling
        if self.sample_size.is_none() || self.sample.len() < self.sample_size.unwrap() {
            merge(&mut self.harmonized_value, &value);
            self.sample.push(value);
            return Ok(());
        }

        self.flush()?;

        fill_record(&value, &mut self.output_record, &self.stack);
        self.writer.write_byte_record(&self.output_record)?;

        Ok(())
    }
}

const JSON_MAX_SAFE_INTEGER: i64 = 9007199254740991;

#[derive(Debug, Clone, Copy)]
pub enum JSONEmptyMode {
    Null,
    Empty,
    Omit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JSONType {
    Null,
    String,
    Integer,
    Float,
}

impl JSONType {
    fn merge(self, other: Self) -> Self {
        match self {
            Self::Null => other,
            Self::String => self,
            Self::Integer => match other {
                Self::Float | Self::String => other,
                Self::Integer | Self::Null => self,
            },
            Self::Float => match other {
                Self::Float | Self::Integer | Self::Null => self,
                Self::String => other,
            },
        }
    }
}

#[derive(Debug)]
struct JSONTypeInferrence {
    json_types: Vec<JSONType>,
    empty_mode: JSONEmptyMode,
}

impl JSONTypeInferrence {
    pub fn new(columns: usize, empty_mode: JSONEmptyMode) -> Self {
        let mut json_types = Vec::with_capacity(columns);

        for _ in 0..columns {
            json_types.push(JSONType::Null);
        }

        Self {
            json_types,
            empty_mode,
        }
    }

    fn infer(value: &str) -> JSONType {
        if value.is_empty() {
            return JSONType::Null;
        }

        if let Ok(integer) = value.parse::<i64>() {
            if integer.abs() <= JSON_MAX_SAFE_INTEGER {
                return JSONType::Integer;
            } else {
                return JSONType::String;
            }
        }

        if value.parse::<f64>().is_ok() {
            return JSONType::Float;
        }

        JSONType::String
    }

    fn cast(&self, value: &str, json_type: JSONType) -> Option<Value> {
        if value.is_empty() {
            return match self.empty_mode {
                JSONEmptyMode::Omit => None,
                JSONEmptyMode::Null => Some(Value::Null),
                JSONEmptyMode::Empty => Some(json!("")),
            };
        }

        match json_type {
            JSONType::String | JSONType::Null => Some(json!(value)),
            JSONType::Float => {
                if let Ok(float) = value.parse::<f64>() {
                    Some(json!(float))
                } else {
                    Some(json!(value))
                }
            }
            JSONType::Integer => {
                if let Ok(integer) = value.parse::<i64>() {
                    if integer.abs() <= JSON_MAX_SAFE_INTEGER {
                        return Some(json!(integer));
                    }
                }

                Some(json!(value))
            }
        }
    }

    fn process<'a>(&mut self, values: impl Iterator<Item = &'a str>) {
        for (json_type, value) in self.json_types.iter_mut().zip(values) {
            let new_json_type = Self::infer(value);
            *json_type = json_type.merge(new_json_type);
        }
    }
}

#[derive(Debug)]
pub struct JSONTypeInferrenceBuffer {
    inferrence: JSONTypeInferrence,
    buffer: Vec<StringRecord>,
    capacity: Option<usize>,
    selection: Selection,
}

impl JSONTypeInferrenceBuffer {
    pub fn new(
        selection: Selection,
        buffer_size: Option<usize>,
        empty_mode: JSONEmptyMode,
    ) -> Self {
        Self {
            inferrence: JSONTypeInferrence::new(selection.len(), empty_mode),
            buffer: match buffer_size {
                Some(capacity) => Vec::with_capacity(capacity),
                None => Vec::new(),
            },
            capacity: buffer_size,
            selection,
        }
    }

    pub fn with_columns(
        columns: usize,
        buffer_size: Option<usize>,
        empty_mode: JSONEmptyMode,
    ) -> Self {
        Self::new(Selection::full(columns), buffer_size, empty_mode)
    }

    pub fn read<R: Read>(&mut self, reader: &mut csv::Reader<R>) -> Result<(), csv::Error> {
        match self.capacity {
            Some(capacity) => {
                for result in reader.records().take(capacity) {
                    self.process(result?);
                }
            }
            None => {
                for result in reader.records() {
                    self.process(result?);
                }
            }
        }

        Ok(())
    }

    pub fn process(&mut self, record: StringRecord) {
        self.inferrence.process(self.selection.select(&record));
        self.buffer.push(record);
    }

    pub fn cast<'a, 'b>(
        &'a self,
        headers: &'b StringRecord,
        record: &'b StringRecord,
    ) -> impl Iterator<Item = Option<(&'a str, Value)>> + 'a
    where
        'b: 'a,
    {
        self.selection
            .select(headers)
            .zip(self.selection.select(record))
            .enumerate()
            .map(|(i, (header, value))| {
                self.inferrence
                    .cast(value, self.inferrence.json_types[i])
                    .map(|json_value| (header, json_value))
            })
    }

    pub fn mutate_attributes(&self, attributes: &mut OmittableAttributes, record: &StringRecord) {
        for ((cell, current_value), json_type) in self
            .selection
            .select(record)
            .zip(attributes.values_mut())
            .zip(self.inferrence.json_types.iter())
        {
            *current_value = self.inferrence.cast(cell, *json_type);
        }
    }

    pub fn records(&self) -> impl Iterator<Item = &StringRecord> {
        self.buffer.iter()
    }

    pub fn types(&self) -> impl Iterator<Item = JSONType> + '_ {
        self.inferrence.json_types.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inferrence() {
        let mut inferrence = JSONTypeInferrence::new(4, JSONEmptyMode::Null);

        inferrence.process(["1", "1", "", "5"].into_iter());
        inferrence.process(["2", "george", "", "6"].into_iter());
        inferrence.process(["3", "", "", "3.8"].into_iter());

        assert_eq!(
            inferrence.json_types,
            vec![
                JSONType::Integer,
                JSONType::String,
                JSONType::Null,
                JSONType::Float
            ]
        );
    }
}
