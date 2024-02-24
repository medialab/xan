use csv;
use serde_json::Value;
use std::borrow::Cow;
use std::fs;
use std::io::{self, BufRead, BufReader};

use config::Config;
use util;
use CliResult;

static USAGE: &str = "
Converts a newline-delimited JSON file (.ndjson or .jsonl, typically) into
a CSV file.

The command tries to do its best but since it is not possible to
straightforwardly convert JSON lines to CSV, the process might lose some complex
fields from the input.

Also, it will fail if the JSON documents are not consistent with one another,
as the first JSON line will be use to infer the headers of the CSV output.

Usage:
    xan jsonl [options] [<input>]
    xan jsonl --help

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_output: Option<String>,
}

const SAMPLE_SIZE: usize = 64;

// TODO: option to consider lists as scalars or as paths
// TODO: configurable sample size
pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    let rdr: Box<dyn BufRead> = match args.arg_input {
        None => Box::new(BufReader::new(io::stdin())),
        Some(p) => Box::new(BufReader::new(fs::File::open(p)?)),
    };

    let mut sampled_records: Vec<Value> = Vec::new();
    let mut possible_stacks: Vec<TraversalStack> = Vec::new();
    let mut headers_emitted: bool = false;
    let mut output_record = csv::StringRecord::new();
    let mut best_stack = TraversalStack::new();

    for (i, line) in rdr.lines().enumerate() {
        let value: Value = serde_json::from_str(&line?).expect("Could not parse line as JSON!");

        if i < SAMPLE_SIZE {
            let mut stack = TraversalStack::new();
            traverse_to_build_stack(&value, &mut stack, 0);
            sampled_records.push(value);
            possible_stacks.push(stack);
        } else if !headers_emitted {
            best_stack = possible_stacks
                .iter()
                .max_by_key(|s| s.len())
                .unwrap()
                .to_vec();

            wtr.write_record(&headers_from_stack(&best_stack))?;

            for sample in sampled_records.iter() {
                fill_record(sample, &mut output_record, &best_stack);
                wtr.write_record(&output_record)?;
            }

            headers_emitted = true;
            sampled_records.clear();

            fill_record(&value, &mut output_record, &best_stack);
            wtr.write_record(&output_record)?;
        } else {
            fill_record(&value, &mut output_record, &best_stack);
            wtr.write_record(&output_record)?;
        }
    }

    // Sample was larger than the file
    if !sampled_records.is_empty() {
        best_stack = possible_stacks
            .iter()
            .max_by_key(|s| s.len())
            .unwrap()
            .to_vec();

        wtr.write_record(&headers_from_stack(&best_stack))?;

        for sample in sampled_records.iter() {
            fill_record(sample, &mut output_record, &best_stack);
            wtr.write_record(&output_record)?;
        }
    }

    Ok(wtr.flush()?)
}

fn fill_record(value: &Value, record: &mut csv::StringRecord, stack: &TraversalStack) {
    record.clear();

    traverse_with_stack(value, &stack, |v| {
        record.push_field(&serialize_value_to_csv(v));
    });
}

// NOTE: we keep a depth on `Delve` and `Pop` to be able to skip absent keys fast
#[derive(Debug, Clone)]
enum TraversalState {
    Delve(String, usize),
    Emit,
    Pop(usize),
}

type TraversalStack = Vec<TraversalState>;

fn traverse_to_build_stack(value: &Value, stack: &mut TraversalStack, depth: usize) {
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
                stack.push(TraversalState::Delve(k.to_string(), depth));

                traverse_to_build_stack(v, stack, depth + 1);

                stack.push(TraversalState::Pop(depth));
            }
        }
        _ => stack.push(TraversalState::Emit),
    };
}

fn headers_from_stack(stack: &TraversalStack) -> csv::StringRecord {
    let mut record = csv::StringRecord::new();

    // Single scalar early return
    if stack.len() == 1 {
        record.push_field("value");
        return record;
    }

    let mut path: Vec<&str> = Vec::new();

    for state in stack {
        match state {
            TraversalState::Delve(key, _) => {
                path.push(key.as_str());
            }
            TraversalState::Emit => {
                record.push_field(&path.join("."));
            }
            TraversalState::Pop(_) => {
                path.pop();
            }
        }
    }

    record
}

fn traverse_with_stack<F>(value: &Value, stack: &TraversalStack, mut callback: F)
where
    F: FnMut(&Value) -> (),
{
    let mut working_stack: Vec<&Value> = vec![];
    let mut current_value: &Value = value;

    let mut i: usize = 0;

    'outer: while i < stack.len() {
        let state = &stack[i];

        match state {
            TraversalState::Delve(key, depth) => {
                working_stack.push(current_value);

                match current_value.as_object().and_then(|o| o.get(key)) {
                    None => {
                        i += 1;

                        while i < stack.len() {
                            let next_state = &stack[i];

                            match next_state {
                                TraversalState::Emit => callback(&Value::Null),
                                TraversalState::Pop(target_depth) if target_depth == depth => {
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
            TraversalState::Pop(_) => {
                current_value = working_stack.pop().expect("cannot pop");
            }
            TraversalState::Emit => {
                callback(current_value);
            }
        }

        i += 1;
    }
}

fn serialize_value_to_csv(value: &Value) -> Cow<str> {
    match value {
        Value::Null => Cow::Borrowed(""),
        Value::Bool(b) => Cow::Borrowed(if *b { "true" } else { "false" }),
        Value::String(s) => Cow::Borrowed(s.as_str()),
        Value::Number(n) => Cow::Owned(n.to_string()),
        Value::Array(l) => Cow::Owned(serde_json::to_string(l).unwrap()),
        Value::Object(o) => Cow::Owned(serde_json::to_string(o).unwrap()),
    }
}
