use std::borrow::Cow;
use std::num::NonZeroUsize;

use csv::StringRecord;
use serde_json::Value;

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
