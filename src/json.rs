use std::borrow::Cow;
use std::io::Read;
use std::num::NonZeroUsize;

use csv::StringRecord;
use serde_json::{json, Map, Value};

use crate::select::Selection;

// NOTE: we keep a depth on `Delve` and `Pop` to be able to skip absent keys efficiently
#[derive(Debug, Clone)]
enum JSONTraversalState {
    Delve(String, usize),
    Emit,
    Pop(usize),
}

type JSONTraversalStack = Vec<JSONTraversalState>;

fn traverse_to_build_stack(value: &Value, stack: &mut JSONTraversalStack, depth: usize) {
    match value {
        Value::Object(map) => {
            let mut items = map.iter().collect::<Vec<_>>();

            // NOTE: we put scalar values first, then nested ones and we also sort by key
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

            for (k, v) in items {
                stack.push(JSONTraversalState::Delve(k.to_string(), depth));

                traverse_to_build_stack(v, stack, depth + 1);

                stack.push(JSONTraversalState::Pop(depth));
            }
        }
        _ => stack.push(JSONTraversalState::Emit),
    };
}

fn headers_from_stack(stack: &JSONTraversalStack) -> csv::StringRecord {
    let mut record = csv::StringRecord::new();

    // Single scalar early return
    if stack.len() == 1 {
        record.push_field("value");
        return record;
    }

    let mut path: Vec<&str> = Vec::new();

    for state in stack {
        match state {
            JSONTraversalState::Delve(key, _) => {
                path.push(key.as_str());
            }
            JSONTraversalState::Emit => {
                record.push_field(&path.join("."));
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

fn serialize_json_value_to_csv_field(value: &Value) -> Cow<str> {
    match value {
        Value::Null => Cow::Borrowed(""),
        Value::Bool(b) => Cow::Borrowed(if *b { "true" } else { "false" }),
        Value::String(s) => Cow::Borrowed(s.as_str()),
        Value::Number(n) => Cow::Owned(n.to_string()),
        Value::Array(l) => Cow::Owned(serde_json::to_string(l).unwrap()),
        Value::Object(o) => Cow::Owned(serde_json::to_string(o).unwrap()),
    }
}

fn fill_record(value: &Value, record: &mut StringRecord, stack: &JSONTraversalStack) {
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

pub fn for_each_json_value_as_csv_record<I, F, E>(
    values: I,
    sample_size: NonZeroUsize,
    mut callback: F,
) -> Result<(), E>
where
    I: Iterator<Item = Result<Value, E>>,
    F: FnMut(&StringRecord) -> Result<(), E>,
{
    let mut merged_value_from_sample = Value::Null;
    let mut sampled_records: Vec<Value> = Vec::new();
    let mut headers_emitted: bool = false;
    let mut output_record = StringRecord::new();
    let mut stack = JSONTraversalStack::new();

    let sample_size: usize = sample_size.into();

    for (i, result) in values.enumerate() {
        let value = result?;

        // Reading sample
        if i < sample_size {
            merge(&mut merged_value_from_sample, &value);
            sampled_records.push(value);
            continue;
        }

        // Emitting headers
        if !headers_emitted {
            traverse_to_build_stack(&merged_value_from_sample, &mut stack, 0);
            callback(&headers_from_stack(&stack))?;

            for sample in sampled_records.iter() {
                fill_record(sample, &mut output_record, &stack);
                callback(&output_record)?;
            }

            headers_emitted = true;
            sampled_records.clear();
        }

        fill_record(&value, &mut output_record, &stack);
        callback(&output_record)?;
    }

    // Sample was larger than the file
    if !sampled_records.is_empty() {
        traverse_to_build_stack(&merged_value_from_sample, &mut stack, 0);
        callback(&headers_from_stack(&stack))?;

        for sample in sampled_records.iter() {
            fill_record(sample, &mut output_record, &stack);
            callback(&output_record)?;
        }
    }

    Ok(())
}

const JSON_MAX_SAFE_INTEGER: i64 = 9007199254740991;

#[derive(Debug, Clone, Copy)]
pub enum JSONEmptyMode {
    Null,
    Empty,
    Omit,
}

impl JSONEmptyMode {
    fn is_omit(&self) -> bool {
        matches!(self, Self::Omit)
    }
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
    capacity: usize,
    selection: Selection,
}

impl JSONTypeInferrenceBuffer {
    pub fn new(selection: Selection, buffer_size: usize, empty_mode: JSONEmptyMode) -> Self {
        Self {
            inferrence: JSONTypeInferrence::new(selection.len(), empty_mode),
            buffer: Vec::with_capacity(buffer_size),
            capacity: buffer_size,
            selection,
        }
    }

    pub fn with_columns(columns: usize, buffer_size: usize, empty_mode: JSONEmptyMode) -> Self {
        Self::new(Selection::full(columns), buffer_size, empty_mode)
    }

    pub fn read<R: Read>(&mut self, reader: &mut csv::Reader<R>) -> Result<(), csv::Error> {
        for result in reader.records().take(self.capacity) {
            self.process(result?);
        }

        Ok(())
    }

    pub fn process(&mut self, record: StringRecord) {
        self.inferrence
            .process(self.selection.select_string_record(&record));
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
            .select_string_record(headers)
            .zip(self.selection.select_string_record(record))
            .enumerate()
            .map(|(i, (header, value))| {
                self.inferrence
                    .cast(value, self.inferrence.json_types[i])
                    .map(|json_value| (header, json_value))
            })
    }

    pub fn mutate_json_map(
        &self,
        map: &mut Map<String, Value>,
        headers: &StringRecord,
        record: &StringRecord,
    ) {
        if self.inferrence.empty_mode.is_omit() {
            map.clear();
        }

        for (i, (header, value)) in headers
            .iter()
            .zip(self.selection.select_string_record(record))
            .enumerate()
        {
            if let Some(json_value) = self.inferrence.cast(value, self.inferrence.json_types[i]) {
                map.insert(header.to_string(), json_value);
            }
        }
    }

    pub fn records(&self) -> impl Iterator<Item = &StringRecord> {
        self.buffer.iter()
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
